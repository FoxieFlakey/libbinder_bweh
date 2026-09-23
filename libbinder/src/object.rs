use crate::packet::{self, Packet};
use enumflags2::{BitFlags, bitflags};

#[bitflags]
#[repr(u16)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Flag {
    OneWay,
}

pub trait Object {
    // Similar to one in Android, returns true if 'code'
    // is known else 'false' if not known
    //
    // On Err, runtime will try serializes the error to reply
    // and returns it
    fn on_transaction(
        &self,
        code: u32,
        flags: BitFlags<Flag>,
        message: &Packet,
        reply: Option<&mut packet::Writer>,
    ) -> anyhow::Result<bool>;
}
