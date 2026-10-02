use std::sync::{Arc, Weak};

use either::Either;
use libbinder_sys::types::reference::ObjectRefRemote;

use crate::{
    Runtime,
    object::{B, ObjectTrait},
};

pub struct Proxy {
    pub(crate) rt: Weak<Runtime>,
    pub(crate) reference: Either<Arc<B<dyn ObjectTrait>>, ObjectRefRemote>,
}

impl Drop for Proxy {
    fn drop(&mut self) {
        let Some(rt) = self.rt.upgrade() else {
            return;
        };

        if let Either::Right(x) = &self.reference {
            rt.dec_remote_ref(x);
        }
    }
}

impl Proxy {
    pub fn from_object(local: Arc<B<dyn ObjectTrait>>) -> Self {
        match local.get_remote() {
            Some(proxy) => Self {
                rt: local.get_runtime().clone(),
                reference: Either::Right(
                    *proxy
                        .reference
                        .as_ref()
                        .right()
                        .inspect(|remote_ref| {
                            // We need to notify kernel that the local reference is cloned
                            proxy.rt.upgrade().unwrap().inc_remote_ref(*remote_ref);
                        })
                        .expect(".get_remote returns non remote reference!"),
                ),
            },
            None => Self {
                rt: local.get_runtime().clone(),
                reference: Either::Left(local),
            },
        }
    }
}

impl ObjectTrait for Proxy {
    fn get_remote<'a>(&'a self) -> Option<&'a Proxy> {
        if self.reference.is_right() {
            None
        } else {
            Some(self)
        }
    }

    fn get_runtime<'a>(&'a self) -> &'a Weak<Runtime> {
        &self.rt
    }

    fn on_transaction(
        &self,
        code: u32,
        flags: enumflags2::BitFlags<crate::object::Flag>,
        message: &mut crate::packet::Packet,
    ) -> Result<Option<(u32, crate::packet::Packet)>, crate::object::TransactionError> {
        match &self.reference {
            Either::Left(local) => local.on_transaction(code, flags, message),
            Either::Right(x) => {
                let rt: Arc<Runtime> = self
                    .rt
                    .upgrade()
                    .expect("Runtime is not alive anymore for Binder proxy");
                rt.send_packet(code, flags, message, *x)
            }
        }
    }
}
