use std::os::fd::OwnedFd;

use anyhow::{Context, anyhow, bail};
use enumflags2::BitFlags;
use libbinder::{object::ObjectTrait, proxy::Proxy};

use crate::{
    interface::object::{REPLY_FAILURE, REPLY_SUCCESS},
    packetable::Packetable,
    reader::Reader,
    writer::Writer,
};

pub trait IObject: ObjectTrait {
    // this can be used to determine if derived proxy can be made
    // but lets put that out of scope, assume this is normal method
    fn has_interface(&self, name: &str) -> anyhow::Result<bool>;
    fn get_an_file(&self, path: &str) -> anyhow::Result<OwnedFd>;
}

pub const IOBJECT_HAS_INTERFACE_ID: u32 =
    /* would be base::NEXT_TRANSACTION_ID + 0, but there no parent */
    0;
pub const IOBJECT_GET_AN_FILE: u32 =
    /* would be base::NEXT_TRANSACTION_ID + 1, but there no parent */
    1;
pub const IOBJECT_NEXT_TRANSACTION_ID: u32 =
    /* would be base::NEXT_TRANSACTION_ID + 2, but there no parent */
    2;

impl dyn IObject {
    pub fn decode_and_dispatch(
        target: &dyn IObject,
        code: u32,
        flags: enumflags2::BitFlags<libbinder::object::Flag>,
        message: &mut libbinder::packet::Packet,
    ) -> Result<Option<(u32, libbinder::packet::Packet)>, libbinder::object::TransactionError> {
        let reply: Result<libbinder::packet::Packet, anyhow::Error> = (|| {
            let mut reply_writer = Writer::new(target.get_runtime());
            let mut msg_reader = Reader::new(message);
            match code {
                IOBJECT_HAS_INTERFACE_ID => {
                    let name = msg_reader.read_str().context("Reading 'name'")?;
                    target
                        .has_interface(name)?
                        .serialize(&mut reply_writer)
                        .context("Cannot serializing return value")?;
                    Ok(reply_writer.finish())
                }

                IOBJECT_GET_AN_FILE => {
                    let name = msg_reader.read_str().context("Reading 'name'")?;
                    target
                        .get_an_file(name)?
                        .serialize(&mut reply_writer)
                        .context("Cannot serializing return value")?;
                    Ok(reply_writer.finish())
                }

                // As there no 'base' to be called its returns error
                x => Err(anyhow!("unrecognized transaction code {x}")),
            }
        })();

        if flags.contains(libbinder::object::Flag::OneWay) {
            // Similar to Android's behaviour, don't send reply
            Ok(None)
        } else {
            match reply {
                Ok(x) => Ok(Some((REPLY_SUCCESS, x))),
                Err(err) => {
                    let mut writer = Writer::new(target.get_runtime());
                    err.serialize(&mut writer).unwrap();
                    Ok(Some((REPLY_FAILURE, writer.finish())))
                }
            }
        }
    }
}

// Proxy object would routes method and sent it to
// remote.
pub struct ProxyIObject {
    base: Proxy,
}

impl ProxyIObject {
    // Caller will assume it support IObject. In real one there would be other verification method
    // mainly thats where has_interface useful, but lets skip for simplicity for now, and thats
    // why its fallible even its always Ok
    pub fn new(base: Proxy) -> anyhow::Result<Self> {
        Ok(Self { base })
    }
}

impl ObjectTrait for ProxyIObject {
    fn get_remote<'a>(&'a self) -> Option<&'a Proxy> {
        self.base.get_remote()
    }

    fn get_runtime(&self) -> std::sync::Arc<libbinder::Runtime> {
        self.base.get_runtime()
    }

    fn on_transaction(
        &self,
        code: u32,
        flags: enumflags2::BitFlags<libbinder::object::Flag>,
        message: &mut libbinder::packet::Packet,
    ) -> Result<Option<(u32, libbinder::packet::Packet)>, libbinder::object::TransactionError> {
        self.base.on_transaction(code, flags, message)
    }
}

impl IObject for ProxyIObject {
    fn has_interface(&self, name: &str) -> anyhow::Result<bool> {
        let mut writer = Writer::new(self.get_runtime());
        name.serialize(&mut writer)
            .context("Error serializing argument 'name'")?;
        let mut packet = writer.finish();
        let (code, reply) = self
            .on_transaction(IOBJECT_HAS_INTERFACE_ID, BitFlags::default(), &mut packet)
            .context("Cannot perform transaction")?
            .context("Expecting reply, but got none")?;

        let mut reader = Reader::new(&reply);
        if code == REPLY_FAILURE {
            // ABI for failure is just bare string
            bail!(
                "remote error: {}",
                str::deserialize(&mut reader).context("Cannot read remote error message")?
            );
        } else if code == REPLY_SUCCESS {
            // here you decode the resopne
            Ok(bool::deserialize(&mut reader).context("Cannot read remote's response")?)
        } else {
            bail!("Remote sent unknown reply code: {code}");
        }
    }

    fn get_an_file(&self, path: &str) -> anyhow::Result<OwnedFd> {
        let mut writer = Writer::new(self.get_runtime());
        path.serialize(&mut writer)
            .context("Error serializing argument 'name'")?;
        let mut packet = writer.finish();
        let (code, reply) = self
            .on_transaction(IOBJECT_GET_AN_FILE, BitFlags::default(), &mut packet)
            .context("Cannot perform transaction")?
            .context("Expecting reply, but got none")?;

        let mut reader = Reader::new(&reply);
        if code == REPLY_FAILURE {
            // ABI for failure is just bare string
            bail!(
                "remote error: {}",
                str::deserialize(&mut reader).context("Cannot read remote error message")?
            );
        } else if code == REPLY_SUCCESS {
            // here you decode the resopne
            Ok(OwnedFd::deserialize(&mut reader).context("Cannot read remote's response")?)
        } else {
            bail!("Remote sent unknown reply code: {code}");
        }
    }
}

macro_rules! iobject_forwarder {
    ($impl_name:ident, $target_field:ident) => {
        impl IObject for $impl_name {
            fn has_interface(&self, name: &str) -> anyhow::Result<bool> {
                self.$target_field.has_interface(name)
            }

            fn get_an_file(&self, path: &str) -> anyhow::Result<OwnedFd> {
                self.$target_field.get_an_file(path)
            }
        }
    };
}
