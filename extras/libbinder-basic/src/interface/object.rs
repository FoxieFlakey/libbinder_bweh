use std::os::fd::OwnedFd;

use anyhow::{Context, anyhow, bail};
use enumflags2::BitFlags;
use libbinder::{object::ObjectTrait, proxy::Proxy};

use crate::{packetable::Packetable, reader::Reader, writer::Writer};

// 'ObjectTrait' is core interface where "on_transaction"
// method resides, which will marshals and unmarshals data
// dispatch to appropriate methods. You must not assume any
// trait name at all other than the methods

// While IOBject is downstream interface
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

// here lets say derive the IObject into IDerived
pub trait IDerived: IObject {
    fn derive_say_hi(&self) -> anyhow::Result<String>;
}

pub const IDERIVED_SAY_HI: u32 = IOBJECT_NEXT_TRANSACTION_ID + 0;
pub const IDERIVED_NEXT_TRANSACTION_ID: u32 = IOBJECT_NEXT_TRANSACTION_ID + 1;

// here lets say derive the IObject into IOther IObject has multiple subtraits
pub trait IOther: IObject {
    fn other_method(&self) -> anyhow::Result<u32>;
}

pub const IOTHER_METHOD: u32 = IOBJECT_NEXT_TRANSACTION_ID + 0;
pub const IOTHER_NEXT_TRANSACTION_ID: u32 = IOBJECT_NEXT_TRANSACTION_ID + 1;

/* framework defined, assume this exists dont hard code */
pub const REPLY_SUCCESS: u32 = 0;

// Return this reply code, ONLY if you dont recognize the transaction code
// or other error that is now allowed remote to read request/perform transaction/
// THIS IS NOT set for if the remote method itself returns Result::Err, it remains
// encoded as REPLY_SUCCESS because the transaction si sucesfully calls to method
pub const REPLY_FAILURE: u32 = 1;
/**/

// Decoder/server side method that decode and dispatch
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
                    writer.write_str(&err.to_string());
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

// Here for the derived proxy
// Proxy object would routes method and sent it to
// remote.
pub struct ProxyDerived {
    base: ProxyIObject,
}

impl ProxyDerived {
    // Caller will assume it support IObject. In real one there would be other verification method
    // mainly thats where has_interface useful, but lets skip for simplicity for now, and thats
    // why its fallible even its always Ok
    pub fn new(base: ProxyIObject) -> anyhow::Result<Self> {
        Ok(Self { base })
    }
}

impl ObjectTrait for ProxyDerived {
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

iobject_forwarder!(ProxyDerived, base);

impl IDerived for ProxyDerived {
    fn derive_say_hi(&self) -> anyhow::Result<String> {
        let writer = Writer::new(self.get_runtime());
        // Leave this empty, because no arguments to be written

        let mut packet = writer.finish();
        let (code, reply) = self
            .on_transaction(IDERIVED_SAY_HI, BitFlags::default(), &mut packet)
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
            Ok(str::deserialize(&mut reader).context("Cannot read remote's response")?)
        } else {
            bail!("Remote sent unknown reply code: {code}");
        }
    }
}

// Here for the other proxy
// Proxy object would routes method and sent it to
// remote.
pub struct ProxyOther {
    base: ProxyIObject,
}

impl ProxyOther {
    // Caller will assume it support IObject. In real one there would be other verification method
    // mainly thats where has_interface useful, but lets skip for simplicity for now, and thats
    // why its fallible even its always Ok
    pub fn new(base: ProxyIObject) -> anyhow::Result<Self> {
        Ok(Self { base })
    }
}

impl ObjectTrait for ProxyOther {
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

iobject_forwarder!(ProxyOther, base);

impl IOther for ProxyOther {
    fn other_method(&self) -> anyhow::Result<u32> {
        let writer = Writer::new(self.get_runtime());
        // Leave this empty, because no arguments to be written

        let mut packet = writer.finish();
        let (code, reply) = self
            .on_transaction(IDERIVED_SAY_HI, BitFlags::default(), &mut packet)
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
            Ok(u32::deserialize(&mut reader).context("Cannot read remote's response")?)
        } else {
            bail!("Remote sent unknown reply code: {code}");
        }
    }
}
