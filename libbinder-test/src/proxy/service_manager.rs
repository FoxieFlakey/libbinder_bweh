use std::sync::Arc;

use anyhow::{Context, bail};
use enumflags2::BitFlags;
use libbinder::{
    Runtime,
    object::{B, Flag, ObjectTrait, TransactionError},
    packet::{self, Packet},
    proxy::Proxy,
};

use crate::{
    interface::{
        REPLY_SUCCESS,
        object::IObject,
        service::IService,
        service_manager::{self, IServiceManager},
    },
    proxy::{self, object::ObjectProxy, service::IServiceProxy},
};

pub struct IServiceManagerProxy(ObjectProxy);

impl IServiceManagerProxy {
    pub fn from_proxy(proxy: Proxy) -> anyhow::Result<Self> {
        let super_proxy = ObjectProxy::from_proxy(proxy)?;
        if !super_proxy
            .has_interface(service_manager::ID)
            .context("Cannot check if remote supports IServiceManager")?
        {
            bail!("Remote object doesnt support IServiceManager");
        }
        Ok(Self(super_proxy))
    }
}

impl ObjectTrait for IServiceManagerProxy {
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

impl IObject for IServiceManagerProxy {
    fn has_interface(&self, interface: &str) -> anyhow::Result<bool> {
        self.0.has_interface(interface)
    }
}

impl IServiceManager for IServiceManagerProxy {
    fn shutdown(&self) -> anyhow::Result<()> {
        let (code, packet) = self
            .on_transaction(
                service_manager::SHUTDOWN_CODE,
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

    fn health_check(&self) -> anyhow::Result<()> {
        let (code, packet) = self
            .on_transaction(
                service_manager::HEALTH_CHECK_CODE,
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

    fn register(&self, service: Arc<B<dyn IService>>, name: &str) -> anyhow::Result<()> {
        let mut writer = packet::Writer::new(self.0.get_runtime());
        writer.write_reference(service);
        writer.write_bytes(name.as_bytes());

        let (code, packet) = self
            .on_transaction(
                service_manager::REGISTER_CODE,
                BitFlags::default(),
                &mut writer.finish(),
            )
            .context("Cannot perform transaction")?
            .expect("This suppose be non oneway transaction");
        if code != REPLY_SUCCESS {
            return Err(proxy::decode_error(&packet));
        }

        Ok(())
    }

    fn unregister(&self, name: &str) -> anyhow::Result<()> {
        let mut writer = packet::Writer::new(self.0.get_runtime());
        writer.write_bytes(name.as_bytes());

        let (code, packet) = self
            .on_transaction(
                service_manager::UNREGISTER_CODE,
                BitFlags::default(),
                &mut writer.finish(),
            )
            .context("Cannot perform transaction")?
            .expect("This suppose be non oneway transaction");
        if code != REPLY_SUCCESS {
            return Err(proxy::decode_error(&packet));
        }

        Ok(())
    }

    fn get_service(&self, name: &str) -> anyhow::Result<Arc<B<dyn IService>>> {
        let mut writer = packet::Writer::new(self.0.get_runtime());
        writer.write_bytes(name.as_bytes());

        let (code, packet) = self
            .on_transaction(
                service_manager::GET_SERVICE_CODE,
                BitFlags::default(),
                &mut writer.finish(),
            )
            .context("Cannot perform transaction")?
            .expect("This suppose be non oneway transaction");
        if code != REPLY_SUCCESS {
            return Err(proxy::decode_error(&packet));
        }

        let mut reader = packet.reader();
        let service = reader
            .read_reference()
            .context("Cannot read service reference")?;

        Ok(Arc::new(B::new(
            IServiceProxy::from_proxy(service).context("Checking if result implements IService")?,
        )))
    }
}
