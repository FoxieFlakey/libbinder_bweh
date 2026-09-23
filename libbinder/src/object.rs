use std::any::Any;

use crate::packet::{self, Packet};
use enumflags2::{BitFlags, bitflags};

#[bitflags]
#[repr(u16)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Flag {
    OneWay,
}

pub trait Object: Sync + Send + Any + 'static {
    fn on_transaction(
        &self,
        code: u32,
        flags: BitFlags<Flag>,
        message: &Packet,
        reply: Option<(&mut u32, &mut BitFlags<Flag>, &mut packet::Writer)>,
    ) -> anyhow::Result<()>;
}
