use std::sync::Weak;

use enumflags2::BitFlags;
use libbinder::{
    Runtime,
    object::{B, Flag, ObjectTrait},
    packet::{self, Packet},
};

use crate::{
    implementation::object::ImplObject,
    interface::{
        REPLY_ERROR, REPLY_SUCCESS,
        object::IObject,
        service::{self, IService},
    },
    once_event::OnceEvent,
};

pub struct ImplService {
    derived: Weak<B<dyn IService>>,
    base: ImplObject,
    shutdown_triggered: OnceEvent,
}

impl ImplService {
    pub fn new(runtime: Weak<Runtime>, derived: Weak<B<dyn IService>>) -> Self {
        Self {
            base: ImplObject::new(runtime.clone(), derived.clone() as Weak<B<dyn IObject>>),
            derived,
            shutdown_triggered: OnceEvent::new(),
        }
    }

    pub fn wait_shutdown(&self) {
        self.shutdown_triggered.wait();
    }
}

impl ObjectTrait for ImplService {
    fn get_remote<'a>(&'a self) -> Option<&'a libbinder::proxy::Proxy> {
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
        let response =
            match code {
                service::SAY_HELLO_CODE => self.derived.upgrade().unwrap().say_hello().map(|_| {
                    Some(packet::Writer::new(self.get_runtime().upgrade().unwrap()).finish())
                }),
                service::STOP_CODE => {
                    self.derived.upgrade().unwrap().stop();
                    Ok(None)
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

impl IObject for ImplService {
    fn has_interface(&self, interface: &str) -> anyhow::Result<bool> {
        match interface {
            service::ID => Ok(true),
            _ => self.base.has_interface(interface),
        }
    }
}

impl IService for ImplService {
    fn stop(&self) {
        println!(
            "[Base service] Shutting down, triggered by {}",
            self.get_runtime()
                .upgrade()
                .unwrap()
                .get_caller_identity()
                .sender_pid
        );
        self.shutdown_triggered.trigger();
    }

    fn say_hello(&self) -> anyhow::Result<()> {
        println!(
            "[Base service] Hello!!!. Requested by {}",
            self.get_runtime()
                .upgrade()
                .unwrap()
                .get_caller_identity()
                .sender_pid
        );
        Ok(())
    }
}
