use std::sync::Weak;

use anyhow::bail;
use enumflags2::BitFlags;
use libbinder::{object::ObjectTrait, packet, proxy::Proxy};

use crate::{
    interface::{
        REPLY_SUCCESS,
        object::{self, IObject},
    },
    proxy,
};

pub struct ObjectProxy(Proxy);

impl ObjectProxy {
    pub fn from_proxy(proxy: Proxy) -> anyhow::Result<Self> {
        Ok(Self(proxy))
    }
}

impl ObjectTrait for ObjectProxy {
    fn get_remote<'a>(&'a self) -> Option<&'a Proxy> {
        Some(&self.0)
    }

    fn get_runtime(&self) -> &Weak<libbinder::Runtime> {
        self.0.get_runtime()
    }

    fn on_transaction(
        &self,
        code: u32,
        flags: enumflags2::BitFlags<libbinder::object::Flag>,
        message: &mut libbinder::packet::Packet,
    ) -> Option<(u32, libbinder::packet::Packet)> {
        self.0.on_transaction(code, flags, message)
    }
}

impl IObject for ObjectProxy {
    fn has_interface(&self, interface: &str) -> anyhow::Result<bool> {
        let mut writer = packet::Writer::new(self.0.get_runtime().upgrade().unwrap());
        writer.write_bytes(interface.as_bytes());
        let (code, packet) = self
            .on_transaction(
                object::HAS_INTERFACE_CODE,
                BitFlags::default(),
                &mut writer.finish(),
            )
            .expect("This suppose be non oneway transaction");
        if code != REPLY_SUCCESS {
            return Err(proxy::decode_error(&packet));
        }

        let response = packet.get_data();
        if response.len() < 1 {
            bail!("Remote returned short response")
        }

        Ok(response[0] != 0)
    }
}
