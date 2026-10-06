use std::{
    fs::{self, File},
    io::{Read, Write},
    os::fd::OwnedFd,
    sync::Weak,
};

use anyhow::Context;
use libbinder::{Runtime, object::B};

use crate::{
    implementation::service::ImplService,
    interface::{IFileServer, IObject, IService, ifileserver},
};

pub struct ImplFileServer {
    base: ImplService,
    derived: Weak<B<dyn IFileServer>>,
}

impl ImplFileServer {
    pub fn new(runtime: Weak<Runtime>, derived: Weak<B<dyn IFileServer>>) -> Self {
        Self {
            base: ImplService::new(runtime, derived.clone() as Weak<B<dyn IService>>),
            derived,
        }
    }

    pub fn wait_shutdown(&self) {
        self.base.wait_shutdown();
    }
}

ifileserver::decode_and_dispatch!(ImplFileServer, base);

impl IObject for ImplFileServer {
    fn has_interface(&self, interface: &str) -> anyhow::Result<bool> {
        match interface {
            ifileserver::ID => Ok(true),
            _ => self.base.has_interface(interface),
        }
    }
}

impl IService for ImplFileServer {
    fn stop(&self) -> anyhow::Result<()> {
        self.base.stop()
    }

    fn say_hello(&self) -> anyhow::Result<()> {
        self.base.say_hello()
    }
}

impl IFileServer for ImplFileServer {
    fn open_file(&self, path: &str) -> anyhow::Result<OwnedFd> {
        Ok(fs::OpenOptions::new()
            .write(true)
            .read(true)
            .create(true)
            .open(path)
            .context("Cannot open file")?
            .into())
    }

    fn read_file(&self, path: &str) -> anyhow::Result<Vec<u8>> {
        let mut buffer = Vec::new();
        let mut file = File::from(
            self.derived
                .upgrade()
                .unwrap()
                .open_file(path)
                .context("Cannot open file for reading")?,
        );
        file.read_to_end(&mut buffer)
            .context("Cannot read the content")?;
        Ok(buffer)
    }

    fn write_file(&self, path: &str, buf: &[u8]) -> anyhow::Result<()> {
        let mut file = File::from(
            self.derived
                .upgrade()
                .unwrap()
                .open_file(path)
                .context("Cannot open file for writing")?,
        );
        file.write_all(buf).context("Cannot write to file")?;
        Ok(())
    }
}
