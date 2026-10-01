use std::{
    any::Any,
    ops::Deref,
    sync::{RwLock, Weak},
};

use crate::{
    Runtime,
    packet::{self, Packet},
};
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
        message: &mut Packet,
        reply: Option<(&mut u32, &mut BitFlags<Flag>, &mut packet::Writer)>,
    ) -> anyhow::Result<()>;
}

pub struct B<T: ObjectTrait + ?Sized> {
    pub(crate) control: RwLock<Refs>,
    inner: T,
}

impl<T: ObjectTrait> B<T> {
    pub fn new(data: T) -> Self {
        Self {
            control: RwLock::new(Refs {
                has_strong: false,
                has_weak: false,
                live_slot: None,
            }),
            inner: data,
        }
    }
}

impl<T: ObjectTrait + ?Sized> Deref for B<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

pub(crate) struct Refs {
    pub(crate) live_slot: Option<(usize, Weak<Runtime>)>,
    pub(crate) has_strong: bool,
    pub(crate) has_weak: bool,
}
