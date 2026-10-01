use std::sync::{Arc, Weak};

use either::Either;
use libbinder_sys::types::reference::{ObjectRef, ObjectRefRemote};

use crate::{
    Runtime,
    object::{B, CallerIdentity, ObjectTrait},
};

pub struct Proxy {
    pub(crate) rt: Weak<Runtime>,
    pub(crate) reference: Either<Arc<B<dyn ObjectTrait>>, ObjectRefRemote>,
}

impl Proxy {
    pub fn from_local(local: Arc<B<dyn ObjectTrait>>) -> Self {
        Self {
            rt: local.get_runtime().clone(),
            reference: Either::Left(local),
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
        // When sending out this is ignored
        caller_identity: Option<CallerIdentity>,
    ) -> Option<(u32, crate::packet::Packet)> {
        assert!(
            caller_identity.is_none(),
            "Sending out to remote do not support setting caller identity"
        );

        match &self.reference {
            Either::Left(local) => local.on_transaction(code, flags, message, caller_identity),
            Either::Right(x) => {
                let rt: Arc<Runtime> = self
                    .rt
                    .upgrade()
                    .expect("Runtime is not alive anymore for Binder proxy");
                rt.send_packet(code, flags, message, ObjectRef::Remote(*x))
                    .expect("Cannot send transaction")
            }
        }
    }
}
