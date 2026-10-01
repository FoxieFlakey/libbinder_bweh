use std::{
    any::Any,
    ffi::CString,
    ops::Deref,
    sync::{RwLock, Weak},
};

use crate::{Runtime, packet::Packet};
use enumflags2::{BitFlags, bitflags};
use libbinder_sys::transaction::TransactionFlag;

#[derive(Default, Clone, Copy)]
pub struct ObjectFlags {
    pub accept_fds: bool,
    pub want_transaction_security_context: bool,
    pub priority: u8,
}

impl ObjectFlags {
    pub(crate) fn into_flags(&self) -> u32 {
        let mut ret = 0;
        if self.accept_fds {
            ret |= libbinder_sys::types::FLAT_BINDER_FLAG_ACCEPTS_FDS;
        }

        if self.want_transaction_security_context {
            ret |= libbinder_sys::types::FLAT_BINDER_FLAG_TXN_SECURITY_CTX;
        }

        ret |= u32::from(self.priority) & libbinder_sys::types::FLAT_BINDER_FLAG_PRIORITY_MASK;
        ret
    }
}

#[bitflags]
#[repr(u16)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Flag {
    OneWay,
    // Clears all data associated with this parcel on the receiving
    ClearBufs,
}

impl Flag {
    pub(crate) fn into_raw(raw: BitFlags<Self>) -> BitFlags<TransactionFlag> {
        let mut ret = Default::default();
        if raw.contains(Flag::OneWay) {
            ret |= TransactionFlag::OneWay;
        }

        if raw.contains(Flag::ClearBufs) {
            ret |= TransactionFlag::ClearBuffer;
        }
        ret
    }

    pub(crate) fn from_raw(raw: BitFlags<TransactionFlag>) -> BitFlags<Self> {
        let mut ret = Default::default();
        if raw.contains(TransactionFlag::OneWay) {
            ret |= Flag::OneWay;
        }

        if raw.contains(TransactionFlag::ClearBuffer) {
            ret |= Flag::ClearBufs;
        }
        ret
    }
}

pub trait ObjectTrait: Sync + Send + Any + 'static {
    fn on_transaction(
        &self,
        code: u32,
        flags: enumflags2::BitFlags<Flag>,
        message: &mut Packet,
        // If None it means local transaction (a.k.a
        // direct call)
        caller_identity: Option<CallerIdentity>,
    ) -> Option<(u32, Packet)>;
}

pub struct B<T: ObjectTrait + ?Sized> {
    pub(crate) control: RwLock<Refs>,
    pub(crate) flags: ObjectFlags,
    inner: T,
}

pub struct CallerIdentity {
    pub sender_euid: u32,
    pub sender_pid: i32,
    pub sender_security_ctx: Option<CString>,
}

impl<T: ObjectTrait> B<T> {
    pub fn new(data: T) -> Self {
        Self {
            control: RwLock::new(Refs {
                has_strong: false,
                has_weak: false,
                live_slot: None,
            }),
            flags: ObjectFlags::default(),
            inner: data,
        }
    }

    pub fn new_with_flags(data: T, flags: ObjectFlags) -> Self {
        Self {
            control: RwLock::new(Refs {
                has_strong: false,
                has_weak: false,
                live_slot: None,
            }),
            flags,
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
