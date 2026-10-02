use std::{
    collections::HashMap,
    sync::{Arc, RwLock, Weak},
};

use anyhow::{Context, anyhow, bail};
use enumflags2::BitFlags;
use libbinder::{
    Runtime,
    object::{B, Flag, ObjectTrait},
    packet::{self, Packet},
    proxy::Proxy,
};
use nix::unistd::Pid;

use crate::{
    implementation::object::ImplObject,
    interface::{
        REPLY_ERROR, REPLY_SUCCESS,
        object::IObject,
        service::IService,
        service_manager::{self, IServiceManager},
    },
    once_event::OnceEvent,
    proxy::service::IServiceProxy,
};

struct State {
    services: HashMap<String, (Arc<B<dyn IService>>, Pid)>,
    is_shutting_down: bool,
}

pub struct ImplManager {
    base: ImplObject,
    derived: Weak<B<dyn IServiceManager>>,
    shutdown_event: OnceEvent,
    state: RwLock<State>,
}

impl ImplManager {
    pub fn new(runtime: Weak<Runtime>, derived: Weak<B<dyn IServiceManager>>) -> Self {
        Self {
            base: ImplObject::new(runtime, derived.clone() as Weak<B<dyn IObject>>),
            shutdown_event: OnceEvent::new(),
            state: RwLock::new(State {
                is_shutting_down: false,
                services: HashMap::new(),
            }),
            derived,
        }
    }

    pub fn wait_shutdown(&self) {
        self.shutdown_event.wait();
    }
}

impl ObjectTrait for ImplManager {
    fn get_remote<'a>(&'a self) -> Option<&'a Proxy> {
        self.base.get_remote()
    }

    fn get_runtime<'a>(&'a self) -> &'a Weak<Runtime> {
        self.base.get_runtime()
    }

    fn on_transaction(
        &self,
        code: u32,
        flags: BitFlags<Flag>,
        message: &mut Packet,
    ) -> Option<(u32, Packet)> {
        let mut reader = message.reader();
        let response = match code {
            service_manager::REGISTER_CODE => match reader.read_reference() {
                Ok(proxy) => match IServiceProxy::from_proxy(proxy) {
                    Ok(service) => match str::from_utf8(reader.get_rest_of_data()) {
                        Ok(name) => {
                            let ret = self
                                .derived
                                .upgrade()
                                .unwrap()
                                .register(Arc::new(B::new(service)), name);

                            ret.map(|_| {
                                Some(
                                    packet::Writer::new(self.get_runtime().upgrade().unwrap())
                                        .finish(),
                                )
                            })
                        }
                        Err(e) => Err(anyhow!("Malform service name: {e}")),
                    },
                    Err(e) => Err(anyhow!(
                        "Cannot check if service supports IService interface: {e}"
                    )),
                },
                Err(e) => Err(anyhow!("Cannot read service reference: {e}")),
            },
            service_manager::UNREGISTER_CODE => match str::from_utf8(reader.get_rest_of_data()) {
                Ok(name) => {
                    let ret = self.derived.upgrade().unwrap().unregister(name);

                    ret.map(|_| {
                        Some(packet::Writer::new(self.get_runtime().upgrade().unwrap()).finish())
                    })
                }
                Err(x) => Err(anyhow!("Malformed interface name: {x}")),
            },
            service_manager::SHUTDOWN_CODE => {
                self.derived.upgrade().unwrap().shutdown();
                Ok(None)
            }
            service_manager::GET_SERVICE_CODE => match str::from_utf8(reader.get_rest_of_data()) {
                Ok(name) => {
                    let ret = self.derived.upgrade().unwrap().get_service(name);

                    ret.map(|x| {
                        let mut writer = packet::Writer::new(self.get_runtime().upgrade().unwrap());
                        writer.write_reference(x);
                        Some(writer.finish())
                    })
                }
                Err(x) => Err(anyhow!("Malformed interface name: {x}")),
            },
            service_manager::HEALTH_CHECK_CODE => {
                let ret = self.derived.upgrade().unwrap().health_check();

                ret.map(|_| {
                    Some(packet::Writer::new(self.get_runtime().upgrade().unwrap()).finish())
                })
            }
            _ => return self.base.on_transaction(code, flags, message),
        };

        match response {
            Ok(Some(response)) => Some((REPLY_SUCCESS, response)),
            Ok(None) => {
                assert!(
                    flags.contains(Flag::OneWay),
                    "Expecting reply, but got none"
                );
                None
            }
            Err(e) => {
                if flags.contains(Flag::OneWay) {
                    return None;
                }

                let mut writer = packet::Writer::new(self.get_runtime().upgrade().unwrap());
                writer.write_bytes(format!("{e:#}"));
                Some((REPLY_ERROR, writer.finish()))
            }
        }
    }
}

impl IObject for ImplManager {
    fn has_interface(&self, interface: &str) -> anyhow::Result<bool> {
        match interface {
            service_manager::ID => Ok(true),
            _ => self.base.has_interface(interface),
        }
    }
}

impl IServiceManager for ImplManager {
    fn health_check(&self) -> anyhow::Result<()> {
        let state = self.state.read().unwrap();
        for service in &state.services {
            service.1.0.say_hello().with_context(|| {
                format!("Cannot check service's health (Service: {})", service.0)
            })?;
        }
        Ok(())
    }

    fn shutdown(&self) {
        let mut state = self.state.write().unwrap();
        state.is_shutting_down = true;
        drop(state);

        let state = self.state.read().unwrap();
        // Trigger shutdown on all services
        state.services.values().for_each(|x| {
            x.0.stop();
        });
        self.shutdown_event.trigger();
    }

    fn register(&self, service: Arc<B<dyn IService>>, name: &str) -> anyhow::Result<()> {
        let mut state = self.state.write().unwrap();
        if state.is_shutting_down {
            bail!("Service manager is shutting down");
        }

        if state.services.get(name).is_some() {
            bail!("Service '{name}' already registered")
        }

        state.services.insert(
            name.to_string(),
            (
                service,
                self.get_runtime()
                    .upgrade()
                    .unwrap()
                    .get_caller_identity()
                    .sender_pid,
            ),
        );
        Ok(())
    }

    fn unregister(&self, name: &str) -> anyhow::Result<()> {
        let mut state = self.state.write().unwrap();
        let Some((_, owner)) = state.services.get(name) else {
            bail!("Service '{name}' is unknown");
        };
        let caller_pid: Pid = self
            .get_runtime()
            .upgrade()
            .unwrap()
            .get_caller_identity()
            .sender_pid;

        if owner != &caller_pid {
            bail!("You do not own service '{name}'");
        }

        state.services.remove(name);
        Ok(())
    }

    fn get_service(&self, name: &str) -> anyhow::Result<Arc<B<dyn IService>>> {
        match self.state.read().unwrap().services.get(name) {
            Some(x) => Ok(x.0.clone()),
            None => bail!("Cannot find service: {name}"),
        }
    }
}
