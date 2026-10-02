use std::sync::Arc;

use anyhow::{Context, bail};
use enumflags2::BitFlags;
use libbinder::{
    Runtime,
    object::{Flag, ObjectTrait, TransactionError},
    packet::{self, Packet},
    proxy::Proxy,
};

use crate::{
    interface::{
        REPLY_SUCCESS,
        object::IObject,
        service::{self, IService},
    },
    proxy::{self, object::ObjectProxy},
};

pub struct IServiceProxy(ObjectProxy);

impl IServiceProxy {
    pub fn from_proxy(proxy: Proxy) -> anyhow::Result<Self> {
        let super_proxy = ObjectProxy::from_proxy(proxy)?;
        if !super_proxy
            .has_interface(service::ID)
            .context("Cannot check if remote supports IService")?
        {
            bail!("Remote object doesnt support IService");
        }
        Ok(Self(super_proxy))
    }
}

impl ObjectTrait for IServiceProxy {
    fn get_remote<'a>(&'a self) -> Option<&'a Proxy> {
        self.0.get_remote()
    }

    fn get_runtime(&self) -> Arc<Runtime> {
        self.0.get_runtime()
    }

    fn on_transaction(
        &self,
        code: u32,
        flags: BitFlags<Flag>,
        message: &mut Packet,
    ) -> Result<Option<(u32, Packet)>, TransactionError> {
        self.0.on_transaction(code, flags, message)
    }
}

impl IObject for IServiceProxy {
    fn has_interface(&self, interface: &str) -> anyhow::Result<bool> {
        self.0.has_interface(interface)
    }
}

impl IService for IServiceProxy {
    fn stop(&self) -> anyhow::Result<()> {
        let (code, packet) = self
            .on_transaction(
                service::STOP_CODE,
                BitFlags::default(),
                &mut packet::Writer::new(self.0.get_runtime()).finish(),
            )
            .context("Cannot perform transaction")?
            .expect("This suppose be non oneway transaction");
        if code != REPLY_SUCCESS {
            return Err(proxy::decode_error(&packet));
        }
        Ok(())
    }

    fn say_hello(&self) -> anyhow::Result<()> {
        let (code, packet) = self
            .on_transaction(
                service::SAY_HELLO_CODE,
                BitFlags::default(),
                &mut packet::Writer::new(self.0.get_runtime()).finish(),
            )
            .context("Cannot perform transaction")?
            .expect("This suppose be non oneway transaction");
        if code != REPLY_SUCCESS {
            return Err(proxy::decode_error(&packet));
        }

        Ok(())
    }
}
