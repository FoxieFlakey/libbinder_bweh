use std::{
    collections::HashMap,
    mem,
    sync::{Arc, RwLock, Weak},
};

use anyhow::{Context, anyhow, bail};
use enumflags2::BitFlags;
use libbinder::{
    DeathNotificationToken, Runtime,
    object::{B, Flag, ObjectTrait, TransactionError},
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

struct ServiceInfo {
    service: Arc<B<dyn IService>>,
    owning_pid: Pid,
    death_notification: DeathNotificationToken,
}

struct State {
    services: HashMap<String, ServiceInfo>,
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

    fn get_runtime(&self) -> Arc<Runtime> {
        self.base.get_runtime()
    }

    fn on_transaction(
        &self,
        code: u32,
        flags: BitFlags<Flag>,
        message: &mut Packet,
    ) -> Result<Option<(u32, Packet)>, TransactionError> {
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

                            ret.map(|_| Some(packet::Writer::new(self.get_runtime()).finish()))
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

                    ret.map(|_| Some(packet::Writer::new(self.get_runtime()).finish()))
                }
                Err(x) => Err(anyhow!("Malformed interface name: {x}")),
            },
            service_manager::SHUTDOWN_CODE => self
                .derived
                .upgrade()
                .unwrap()
                .shutdown()
                .map(|_| Some(packet::Writer::new(self.get_runtime()).finish())),
            service_manager::GET_SERVICE_CODE => match str::from_utf8(reader.get_rest_of_data()) {
                Ok(name) => {
                    let ret = self.derived.upgrade().unwrap().get_service(name);

                    ret.map(|x| {
                        let mut writer = packet::Writer::new(self.get_runtime());
                        writer.write_reference(x);
                        Some(writer.finish())
                    })
                }
                Err(x) => Err(anyhow!("Malformed interface name: {x}")),
            },
            service_manager::HEALTH_CHECK_CODE => {
                let ret = self.derived.upgrade().unwrap().health_check();

                ret.map(|_| Some(packet::Writer::new(self.get_runtime()).finish()))
            }
            _ => return self.base.on_transaction(code, flags, message),
        };

        match response {
            Ok(Some(response)) => Ok(Some((REPLY_SUCCESS, response))),
            Ok(None) => {
                assert!(
                    flags.contains(Flag::OneWay),
                    "Expecting reply, but got none"
                );
                Ok(None)
            }
            Err(e) => {
                if flags.contains(Flag::OneWay) {
                    return Ok(None);
                }

                let mut writer = packet::Writer::new(self.get_runtime());
                writer.write_bytes(format!("{e:#}"));
                Ok(Some((REPLY_ERROR, writer.finish())))
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
            service.1.service.say_hello().with_context(|| {
                format!("Cannot check service's health (Service: {})", service.0)
            })?;
        }
        Ok(())
    }

    fn shutdown(&self) -> anyhow::Result<()> {
        let mut state = self.state.write().unwrap();
        state.is_shutting_down = true;
        let mut registry = mem::take(&mut state.services);
        drop(state);

        // Trigger shutdown on all services
        let keys = registry.keys().cloned().collect::<Vec<_>>();
        for service_name in keys {
            let service = registry.get(&service_name).unwrap();
            if let Err(e) = service
                .service
                .stop()
                .with_context(|| format!("Cannot stop service '{}'", service_name))
            {
                let mut state = self.state.write().unwrap();
                state.is_shutting_down = false;
                state.services = registry;
                drop(state);
                println!("Cannot stop service '{service_name}': {e}");
                return Err(e);
            }
            registry.remove(&service_name);
            println!("Stopped service '{service_name}'");
        }

        self.shutdown_event.trigger();
        Ok(())
    }

    fn register(&self, service: Arc<B<dyn IService>>, name: &str) -> anyhow::Result<()> {
        let mut state = self.state.write().unwrap();
        if state.is_shutting_down {
            bail!("Service manager is shutting down");
        }

        if state.services.get(name).is_some() {
            bail!("Service '{name}' already registered")
        }

        let rt = self.get_runtime();
        let this = self.base.get_derived().clone();
        let name_cloned = name.to_string();
        let at_registration_proxy = service
            .get_remote()
            .expect("Service manager only handles removes, never register local services on itself")
            .clone();
        state.services.insert(
            name.to_string(),
            ServiceInfo {
                owning_pid: rt.get_caller_identity().sender_pid,
                death_notification: rt
                    .attach_death_callback(&service, move || {
                        if let Some(x) = this
                            .upgrade()
                            .map(|x| x as Arc<B<dyn ObjectTrait>>)
                        {
                            let x = x.downcast_ref::<ImplManager>().unwrap();
                            let mut state = x.state.write().unwrap();
                            if let Some(x) = state.services.get(&name_cloned) {
                                let remote = x.service.get_remote().expect("Service manager only handles removes, never register local services on itself");
                                if remote == &at_registration_proxy {
                                    println!("Service '{name_cloned}' died, unregistering");
                                    // it is same service, lets remove
                                    state.services.remove(&name_cloned);
                                }
                            }
                        }
                    })
                    .expect("Service manager only handles removes, never register local services on itself"),
                service,
            },
        );
        println!("Registered service '{name}'");
        Ok(())
    }

    fn unregister(&self, name: &str) -> anyhow::Result<()> {
        let mut state = self.state.write().unwrap();
        let Some(ServiceInfo { owning_pid, .. }) = state.services.get(name) else {
            bail!("Service '{name}' is unknown");
        };
        let caller_pid: Pid = self.get_runtime().get_caller_identity().sender_pid;

        if owning_pid != &caller_pid {
            bail!("You do not own service '{name}'");
        }

        let result = state.services.remove(name).unwrap();
        let _ = self
            .get_runtime()
            .detach_death_callback(result.death_notification);
        println!("Unregistering service '{name}'");
        Ok(())
    }

    fn get_service(&self, name: &str) -> anyhow::Result<Arc<B<dyn IService>>> {
        match self.state.read().unwrap().services.get(name) {
            Some(x) => Ok(x.service.clone()),
            None => bail!("Cannot find service: {name}"),
        }
    }
}
