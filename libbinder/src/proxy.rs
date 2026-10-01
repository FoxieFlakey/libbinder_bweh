// This proxies calls to the remote task

use std::sync::{Arc, Weak};

use libbinder_sys::types::reference::{ObjectRef, ObjectRefRemote};

use crate::{Runtime, object::ObjectTrait};

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
    ) -> anyhow::Result<Option<(u32, crate::packet::Packet)>> {
        let rt: Arc<Runtime> = self
            .rt
            .upgrade()
            .expect("Runtime is not alive anymore for Binder proxy");
        let (ret_code, ret_reply) = rt
            .send_packet(code, flags, message, ObjectRef::Remote(self.remote_ref))?
            .unwrap();
        Ok(Some((ret_code, ret_reply)))
    }
}
