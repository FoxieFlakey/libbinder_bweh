use std::{
    any::Any,
    ffi::CString,
    io,
    ops::Deref,
    sync::{RwLock, Weak},
};

use crate::{Runtime, packet::Packet, proxy::Proxy};
use enumflags2::{BitFlags, bitflags};
use libbinder_sys::transaction::TransactionFlag;
use nix::unistd::{Pid, Uid};

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

#[derive(thiserror::Error, Debug)]
pub enum TransactionError {
    #[error("Target of this transaction is gone")]
    TargetDied,
    #[error("Target of this transaction is frozen")]
    TargetFrozen,
    #[error("Kernel cannot send this transaction")]
    KernelCantSend,
    #[error("Kernel has an error")]
    KernelError(io::Error),
}

pub trait ObjectTrait: Sync + Send + Any + 'static {
    // only return if current object just proxy
    // to a remote. this is mainly so the remote
    // handle can be immediately sent
    fn get_remote<'a>(&'a self) -> Option<&'a Proxy>;
    fn get_runtime<'a>(&'a self) -> &'a Weak<Runtime>;
    fn on_transaction(
        &self,
        code: u32,
        flags: enumflags2::BitFlags<Flag>,
        message: &mut Packet,
    ) -> Result<Option<(u32, Packet)>, TransactionError>;
}

pub struct B<T: ObjectTrait + ?Sized> {
    pub(crate) control: RwLock<Refs>,
    pub(crate) flags: ObjectFlags,
    inner: T,
}

#[derive(Clone)]
pub struct CallerIdentity {
    pub sender_euid: Uid,
    pub sender_pid: Pid,
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

impl B<dyn ObjectTrait> {
    pub fn downcast_ref<T: Any + ObjectTrait>(&self) -> Option<&T> {
        let a: &dyn Any = &self.inner;
        a.downcast_ref()
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
