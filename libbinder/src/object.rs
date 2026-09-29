use std::any::Any;

use crate::packet::{self, Packet};
use enumflags2::{BitFlags, bitflags};
use libbinder_sys::transaction::TransactionFlag;

#[bitflags]
#[repr(u16)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Flag {
    OneWay,
}

impl Flag {
    pub(crate) fn into_raw(raw: BitFlags<Self>) -> BitFlags<TransactionFlag> {
        let mut ret = Default::default();
        if raw.contains(Flag::OneWay) {
            ret |= TransactionFlag::OneWay;
        }
        ret
    }

    pub(crate) fn from_raw(raw: BitFlags<TransactionFlag>) -> BitFlags<Self> {
        let mut ret = Default::default();
        if raw.contains(TransactionFlag::OneWay) {
            ret |= Flag::OneWay;
        }
        ret
    }
}

pub trait ObjectTrait: Sync + Send + Any + 'static {
    fn on_transaction(
        &self,
        code: u32,
        flags: BitFlags<Flag>,
        message: &Packet,
        reply: Option<(&mut u32, &mut BitFlags<Flag>, &mut packet::Writer)>,
    ) -> anyhow::Result<()>;
}
