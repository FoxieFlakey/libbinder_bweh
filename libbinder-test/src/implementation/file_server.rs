use std::{
    fs::{self, File},
    io::{Read, Write},
    os::fd::{AsFd, OwnedFd},
    sync::{Arc, Weak},
};

use anyhow::{Context, anyhow};
use enumflags2::BitFlags;
use libbinder::{
    Runtime,
    object::{B, Flag, ObjectTrait, TransactionError},
    packet::{self, Packet},
    proxy::Proxy,
};

use crate::{
    implementation::service::ImplService,
    interface::{
        REPLY_ERROR, REPLY_SUCCESS,
        file_server::{self, IFileServer},
        object::IObject,
        service::IService,
    },
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

    pub fn base(&self) -> &ImplService {
        &self.base
    }
}

impl ObjectTrait for ImplFileServer {
    fn get_remote<'a>(&'a self) -> Option<&'a Proxy> {
        self.base.get_remote()
    }

    fn get_runtime(&self) -> Arc<Runtime> {
        self.base.get_runtime()
    }

    fn on_transaction(
        &self,
        code: u32,
        flags: BitFlags<Flag>,
        message: &mut Packet,
    ) -> Result<Option<(u32, Packet)>, TransactionError> {
        let mut reader = message.reader();
        let response = match code {
            file_server::OPEN_FILE_CODE => {
                if flags.contains(Flag::AcceptFd) {
                    match str::from_utf8(reader.get_rest_of_data()) {
                        Ok(path) => match self.derived.upgrade().unwrap().open_file(path) {
                            Ok(x) => {
                                let mut writer = packet::Writer::new(self.get_runtime());
                                match writer.write_fd(x.as_fd()) {
                                    Ok(_) => Ok(Some(writer.finish())),
                                    Err(e) => Err(anyhow!("Error writing file FD: {e}")),
                                }
                            }
                            Err(e) => Err(e),
                        },
                        Err(e) => Err(anyhow!("Invalid 'path': {e}")),
                    }
                } else {
                    Err(anyhow!("AcceptFd flag required for this transaction"))
                }
            }
            file_server::WRITE_FILE_CODE => {
                let mut path_len_raw = [0; size_of::<usize>()];
                match reader.read_bytes(&mut path_len_raw) {
                    Ok(()) => {
                        let len = usize::from_ne_bytes(path_len_raw);
                        if reader.get_rest_of_data().len() >= len {
                            match str::from_utf8(&reader.get_rest_of_data()[..len]) {
                                Ok(path) => {
                                    reader.skip_bytes(len).unwrap();
                                    let mut buffer_len_raw = [0; size_of::<usize>()];
                                    match reader.read_bytes(&mut buffer_len_raw) {
                                        Ok(()) => {
                                            let buffer_len = usize::from_ne_bytes(buffer_len_raw);
                                            if reader.get_rest_of_data().len() >= buffer_len {
                                                let buffer =
                                                    &reader.get_rest_of_data()[..buffer_len];
                                                reader.skip_bytes(buffer_len).unwrap();
                                                self.derived
                                                    .upgrade()
                                                    .unwrap()
                                                    .write_file(path, buffer)
                                                    .map(|_| {
                                                        Some(
                                                            packet::Writer::new(self.get_runtime())
                                                                .finish(),
                                                        )
                                                    })
                                            } else {
                                                Err(anyhow!(
                                                    "Buffer given is shorter than the length"
                                                ))
                                            }
                                        }
                                        Err(e) => Err(anyhow!("Cannot read buffer's length: {e}")),
                                    }
                                }
                                Err(e) => Err(anyhow!("Invalid 'path': {e}")),
                            }
                        } else {
                            Err(anyhow!("Path given is shorter than the length"))
                        }
                    }
                    Err(e) => Err(anyhow!("Cannot read path's length: {e}")),
                }
            }
            file_server::READ_FILE_CODE => match str::from_utf8(reader.get_rest_of_data()) {
                Ok(path) => self.derived.upgrade().unwrap().read_file(path).map(|x| {
                    let mut writer = packet::Writer::new(self.get_runtime());
                    writer.write_buf(x);
                    Some(writer.finish())
                }),
                Err(e) => Err(anyhow!("Invalid 'path': {e}")),
            },
            _ => return self.base.on_transaction(code, flags, message),
        };

        match response {
            Ok(Some(response)) => Ok(Some((REPLY_SUCCESS, response))),
            Ok(None) => {
                assert!(
                    flags.contains(Flag::OneWay),
                    "Expecting reply, but got none"
                );
                Ok(None)
            }
            Err(e) => {
                if flags.contains(Flag::OneWay) {
                    return Ok(None);
                }

                let mut writer = packet::Writer::new(self.get_runtime());
                writer.write_bytes(format!("{e:#}"));
                Ok(Some((REPLY_ERROR, writer.finish())))
            }
        }
    }
}

impl IObject for ImplFileServer {
    fn has_interface(&self, interface: &str) -> anyhow::Result<bool> {
        match interface {
            file_server::ID => Ok(true),
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
