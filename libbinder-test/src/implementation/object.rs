use std::sync::{Arc, Weak};

use libbinder::{
    Runtime,
    object::{B, ObjectTrait},
};

use crate::interface::{IObject, iobject};

pub struct ImplObject {
    runtime: Weak<Runtime>,
    this: Weak<B<dyn IObject>>,
}

impl ImplObject {
    pub fn new(rt: Weak<Runtime>, this: Weak<B<dyn IObject>>) -> Self {
        Self { runtime: rt, this }
    }

    pub fn get_this(&self) -> Arc<B<dyn IObject>> {
        self.this.upgrade().unwrap()
    }
}

impl ObjectTrait for ImplObject {
    fn get_remote<'a>(&'a self) -> Option<&'a libbinder::proxy::Proxy> {
        None
    }

    fn get_runtime(&self) -> std::sync::Arc<Runtime> {
        self.runtime.upgrade().unwrap()
    }

    fn on_transaction(
        &self,
        code: u32,
        flags: enumflags2::BitFlags<libbinder::object::Flag>,
        message: &mut libbinder::packet::Packet,
    ) -> Result<Option<(u32, libbinder::packet::Packet)>, libbinder::object::TransactionError> {
        <dyn IObject>::decode_and_dispatch(self, code, flags, message)
    }
}

impl IObject for ImplObject {
    fn has_interface(&self, interface: &str) -> anyhow::Result<bool> {
        match interface {
            iobject::ID => Ok(true),
            _ => Ok(false),
        }
    }
}
