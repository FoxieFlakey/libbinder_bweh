use std::sync::Weak;

use anyhow::anyhow;
use enumflags2::BitFlags;
use libbinder::{
    Runtime,
    object::{B, Flag, ObjectTrait},
    packet::{self, Packet},
    proxy::Proxy,
};

use crate::interface::{
    REPLY_ERROR, REPLY_SUCCESS,
    object::{self, IObject},
};

pub struct ImplObject(Weak<Runtime>, Weak<B<dyn IObject>>);

impl ImplObject {
    pub fn new(runtime: Weak<Runtime>, derived: Weak<B<dyn IObject>>) -> Self {
        Self(runtime, derived)
    }
}

impl ObjectTrait for ImplObject {
    fn get_remote<'a>(&'a self) -> Option<&'a Proxy> {
        None
    }

    fn get_runtime<'a>(&'a self) -> &'a Weak<Runtime> {
        &self.0
    }

    fn on_transaction(
        &self,
        code: u32,
        flags: BitFlags<Flag>,
        message: &mut Packet,
    ) -> Option<(u32, Packet)> {
        let reader = message.reader();
        let response = match code {
            object::HAS_INTERFACE_CODE => match str::from_utf8(reader.get_rest_of_data()) {
                Ok(name) => {
                    let ret = self.1.upgrade().unwrap().has_interface(name);

                    ret.map(|x| {
                        let mut writer = packet::Writer::new(self.get_runtime().upgrade().unwrap());
                        if x {
                            writer.write_bytes(&0x01u8.to_ne_bytes());
                        } else {
                            writer.write_bytes(&0x00u8.to_ne_bytes());
                        }
                        Some(writer.finish())
                    })
                }
                Err(x) => Err(anyhow!("Malformed interface name: {x}")),
            },
            x => Err(anyhow!("Unknown transaction code {x}")),
        };

        match response {
            Ok(Some(response)) => Some((REPLY_SUCCESS, response)),
            Ok(None) => {
                assert!(
                    flags.contains(Flag::OneWay),
                    "Expecting reply, but got none"
                );
                None
            }
            Err(e) => {
                if flags.contains(Flag::OneWay) {
                    return None;
                }

                let mut writer = packet::Writer::new(self.get_runtime().upgrade().unwrap());
                writer.write_bytes(format!("{e:#}"));
                Some((REPLY_ERROR, writer.finish()))
            }
        }
    }
}

impl IObject for ImplObject {
    fn has_interface(&self, interface: &str) -> anyhow::Result<bool> {
        if interface == object::ID {
            Ok(true)
        } else {
            Ok(false)
        }
    }
}
