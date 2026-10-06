use std::sync::Weak;

use libbinder::{
    Runtime,
    object::{B, ObjectTrait},
};

use crate::{
    implementation::object::ImplObject,
    interface::{IObject, IService, iservice},
    once_event::OnceEvent,
};

pub struct ImplService {
    base: ImplObject,
    stop_triggered: OnceEvent,
}

iservice::decode_and_dispatch!(ImplService, base);

impl ImplService {
    pub fn new(rt: Weak<Runtime>, this: Weak<B<dyn IService>>) -> Self {
        Self {
            base: ImplObject::new(rt, this),
            stop_triggered: OnceEvent::new(),
        }
    }

    pub fn wait_shutdown(&self) {
        self.stop_triggered.wait();
    }
}

impl IObject for ImplService {
    fn has_interface(&self, interface: &str) -> anyhow::Result<bool> {
        match interface {
            iservice::ID => Ok(true),
            _ => self.base.has_interface(interface),
        }
    }
}

impl IService for ImplService {
    fn stop(&self) -> anyhow::Result<()> {
        let caller_pid = self.get_runtime().get_caller_identity().sender_pid;
        let caller_uid = self.get_runtime().get_caller_identity().sender_euid;
        println!("[Service] Stop trigged by {caller_pid} who is {caller_uid}");
        self.stop_triggered.trigger();
        Ok(())
    }

    fn say_hello(&self) -> anyhow::Result<()> {
        let caller_pid = self.get_runtime().get_caller_identity().sender_pid;
        let caller_uid = self.get_runtime().get_caller_identity().sender_euid;
        println!("[Service] Say hello from {caller_pid} who is {caller_uid}");
        Ok(())
    }
}
