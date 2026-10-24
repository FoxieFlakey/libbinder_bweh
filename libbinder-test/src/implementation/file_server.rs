use std::{
    fs::{self, File},
    io::{Read, Write},
    os::fd::OwnedFd,
    sync::Weak,
};

use anyhow::Context;
use libbinder::{
    Runtime,
    object::{B, TransactionError},
};

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
    fn has_interface(&self, interface: &str) -> Result<bool, TransactionError> {
        match interface {
            ifileserver::ID => Ok(true),
            _ => self.base.has_interface(interface),
        }
    }
}

impl IService for ImplFileServer {
    fn stop(&self) -> Result<(), TransactionError> {
        self.base.stop()
    }

    fn say_hello(&self) -> Result<(), TransactionError> {
        self.base.say_hello()
    }
}

impl IFileServer for ImplFileServer {
    fn open_file(&self, path: &str) -> Result<OwnedFd, TransactionError> {
        Ok(fs::OpenOptions::new()
            .write(true)
            .read(true)
            .create(true)
            .open(path)
            .context("Cannot open file")
            .map_err(|x| TransactionError::Miscellanous(format!("{x:#}")))?
            .into())
    }

    fn read_file(&self, path: &str) -> Result<Vec<u8>, TransactionError> {
        let mut buffer = Vec::new();
        let mut file = File::from(
            self.derived
                .upgrade()
                .unwrap()
                .open_file(path)
                .context("Cannot open file for reading")
                .map_err(|x| TransactionError::Miscellanous(format!("{x:#}")))?,
        );
        file.read_to_end(&mut buffer)
            .context("Cannot read the content")
            .map_err(|x| TransactionError::Miscellanous(format!("{x:#}")))?;
        Ok(buffer)
    }

    fn write_file(&self, path: &str, buf: &[u8]) -> Result<(), TransactionError> {
        let mut file = File::from(
            self.derived
                .upgrade()
                .unwrap()
                .open_file(path)
                .context("Cannot open file for writing")
                .map_err(|x| TransactionError::Miscellanous(format!("{x:#}")))?,
        );
        file.write_all(buf)
            .context("Cannot write to file")
            .map_err(|x| TransactionError::Miscellanous(format!("{x:#}")))?;
        Ok(())
    }
}
