// This proxies calls to the remote task

use std::sync::{Arc, Weak};

use libbinder_sys::types::reference::{ObjectRef, ObjectRefRemote};

use crate::{
    Runtime,
    object::{CallerIdentity, ObjectTrait},
};

pub struct Proxy {
    pub(crate) rt: Weak<Runtime>,
    pub(crate) remote_ref: ObjectRefRemote,
}

impl ObjectTrait for Proxy {
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

        let rt: Arc<Runtime> = self
            .rt
            .upgrade()
            .expect("Runtime is not alive anymore for Binder proxy");
        rt.send_packet(code, flags, message, ObjectRef::Remote(self.remote_ref))
            .expect("Cannot send transaction")
    }
}
