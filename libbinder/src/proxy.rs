// This proxies calls to the remote task

use std::sync::{Arc, Weak};

use enumflags2::BitFlags;
use libbinder_sys::{
    transaction::TransactionFlag,
    types::reference::{ObjectRef, ObjectRefRemote},
};

use crate::{
    Runtime,
    object::{self, ObjectTrait},
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
        reply: Option<(
            &mut u32,
            &mut BitFlags<object::Flag>,
            &mut crate::packet::Writer,
        )>,
    ) -> anyhow::Result<()> {
        let rt: Arc<Runtime> = self
            .rt
            .upgrade()
            .expect("Runtime is not alive anymore for Binder proxy");
        let reply_ret = rt.send_packet(code, flags, message, ObjectRef::Remote(self.remote_ref))?;
        if let Some((code, flags, reply)) = reply {
            let (ret_code, ret_flags, ret_reply) = reply_ret.unwrap();
            *code = ret_code;
            *flags = BitFlags::default();
            if ret_flags.contains(TransactionFlag::OneWay) {
                *flags |= object::Flag::OneWay;
            }

            reply.copy_from(&ret_reply);
        }

        Ok(())
    }
}
