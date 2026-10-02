use std::{os::fd::OwnedFd, sync::Arc};

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
        file_server::{self, IFileServer},
        object::IObject,
        service::IService,
    },
    proxy::{self, service::IServiceProxy},
};

pub struct IFileServerProxy(IServiceProxy);

impl IFileServerProxy {
    pub fn from_proxy(proxy: Proxy) -> anyhow::Result<Self> {
        let super_proxy = IServiceProxy::from_proxy(proxy)?;
        if !super_proxy
            .has_interface(file_server::ID)
            .context("Cannot check if remote supports IFileServer")?
        {
            bail!("Remote object doesnt support IFileServer");
        }
        Ok(Self(super_proxy))
    }
}

impl ObjectTrait for IFileServerProxy {
    fn get_remote<'a>(&'a self) -> Option<&'a libbinder::proxy::Proxy> {
        self.0.get_remote()
    }

    fn get_runtime(&self) -> Arc<Runtime> {
        self.0.get_runtime()
    }

    fn on_transaction(
        &self,
        code: u32,
        flags: BitFlags<libbinder::object::Flag>,
        message: &mut Packet,
    ) -> Result<Option<(u32, Packet)>, TransactionError> {
        self.0.on_transaction(code, flags, message)
    }
}

impl IObject for IFileServerProxy {
    fn has_interface(&self, interface: &str) -> anyhow::Result<bool> {
        self.0.has_interface(interface)
    }
}

impl IService for IFileServerProxy {
    fn stop(&self) -> anyhow::Result<()> {
        self.0.stop()
    }

    fn say_hello(&self) -> anyhow::Result<()> {
        self.0.say_hello()
    }
}

impl IFileServer for IFileServerProxy {
    fn open_file(&self, path: &str) -> anyhow::Result<OwnedFd> {
        let mut writer = packet::Writer::new(self.0.get_runtime());
        writer.write_bytes(path.as_bytes());

        let (code, packet) = self
            .on_transaction(
                file_server::OPEN_FILE_CODE,
                Flag::AcceptFd.into(),
                &mut writer.finish(),
            )
            .context("Cannot perform transaction")?
            .expect("This suppose be non oneway transaction");
        if code != REPLY_SUCCESS {
            return Err(proxy::decode_error(&packet));
        }

        let mut reader = packet.reader();
        reader.read_fd().context("Cannot read resulting FD")
    }

    fn read_file(&self, path: &str) -> anyhow::Result<Vec<u8>> {
        let mut writer = packet::Writer::new(self.0.get_runtime());
        writer.write_bytes(path.as_bytes());

        let (code, packet) = self
            .on_transaction(
                file_server::READ_FILE_CODE,
                BitFlags::default(),
                &mut writer.finish(),
            )
            .context("Cannot perform transaction")?
            .expect("This suppose be non oneway transaction");
        if code != REPLY_SUCCESS {
            return Err(proxy::decode_error(&packet));
        }

        let mut reader = packet.reader();
        Ok(reader
            .read_buf()
            .context("Cannot read bytes buffer result")?
            .to_vec())
    }

    fn write_file(&self, path: &str, buf: &[u8]) -> anyhow::Result<()> {
        let mut writer = packet::Writer::new(self.0.get_runtime());
        writer.write_bytes(path.len().to_ne_bytes());
        writer.write_bytes(path.as_bytes());
        writer.write_bytes(buf.len().to_ne_bytes());
        writer.write_bytes(buf);

        let (code, packet) = self
            .on_transaction(
                file_server::WRITE_FILE_CODE,
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
}
