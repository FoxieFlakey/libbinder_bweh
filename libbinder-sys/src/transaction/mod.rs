use std::ffi::CStr;

use bytemuck::{Pod, Zeroable};
use enumflags2::{BitFlags, bitflags};

use crate::{BinderUsize, object::reference::ObjectRef};

mod kernel_managed;
mod not_kernel_managed;
pub use kernel_managed::TransactionKernelManaged;
pub use not_kernel_managed::{TransactionNotKernelMananged, TransactionNotKernelManangedSg};

pub enum Transaction<'buffer, 'buffer_offsets> {
    NotKernelManaged(TransactionNotKernelMananged<'buffer, 'buffer_offsets>),
    NotKernelManagedSg(TransactionNotKernelManangedSg<'buffer, 'buffer_offsets>),
    KernelManaged(TransactionKernelManaged),
}

impl<'buffer, 'buffer_offsets> Transaction<'buffer, 'buffer_offsets> {
    pub fn with_bytes<F: FnOnce(&[u8]) -> R, R>(&self, func: F) -> R {
        match self {
            Self::NotKernelManaged(x) => x.with_bytes(func),
            Self::NotKernelManagedSg(x) => x.with_bytes(func),
            Self::KernelManaged(x) => x.with_bytes(func),
        }
    }

    pub fn with_common_mut<F, R>(&mut self, func: F) -> R
    where
        F: FnOnce(&mut TransactionDataCommon) -> R,
    {
        match self {
            Self::KernelManaged(x) => x.with_data_mut(func),
            Self::NotKernelManaged(x) => func(&mut x.data),
            Self::NotKernelManagedSg(x) => func(&mut x.data.data),
        }
    }

    pub fn get_common<'a: 'buffer + 'buffer_offsets>(
        &'a self,
    ) -> &'a TransactionDataCommon<'buffer, 'buffer_offsets> {
        match self {
            Self::KernelManaged(x) => x.get_data(),
            Self::NotKernelManaged(x) => &x.data,
            Self::NotKernelManagedSg(x) => &x.data.data,
        }
    }
}

#[derive(Clone)]
pub struct TransactionDataCommon<'buf, 'buf_offsets> {
    pub target: ObjectRef,
    pub flags: BitFlags<TransactionFlag>,
    pub code: u32,
    pub data_slice: &'buf [u8],
    pub offsets: &'buf_offsets [BinderUsize],
    // Security context of the sender, if its received from
    // BC_TRANSACTION, or None, outside other cases
    pub secctx: Option<&'buf CStr>,
    // NOTE: When sending transaction, kernel ignore these
    // and overwrites when receiving
    pub sender_euid: u32,
    pub sender_pid: i32,
}

#[bitflags]
#[repr(u32)]
#[derive(Clone, Copy)]
pub enum TransactionFlag {
    OneWay = 0x01,
    RootObject = 0x04,
    StatusCode = 0x08,
    AcceptFds = 0x10,
    ClearBuffer = 0x20,
    UpdateTransaction = 0x40,
}

// Union in binder_transaction_data
#[repr(C)]
#[derive(Clone, Copy, Zeroable)]
union BinderOrHandleUnion {
    binder: BinderUsize,
    handle: u32,
}

unsafe impl Pod for BinderOrHandleUnion {}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct BufferStruct {
    buffer: BinderUsize,
    offsets: BinderUsize,
}

#[repr(C)]
#[derive(Clone, Copy, Zeroable)]
union DataUnion {
    ptr: BufferStruct,
    _unused: [u8; 8],
}

unsafe impl Pod for DataUnion {}

// Equivalent to struct binder_transaction_data
#[derive(Clone, Copy, Zeroable, Pod)]
#[repr(C)]
pub(crate) struct TransactionDataRaw {
    target: BinderOrHandleUnion,
    extra_data: usize,
    code: u32,
    flags: u32,
    sender_pid: nix::libc::pid_t,
    sender_uid: nix::libc::uid_t,
    data_size: BinderUsize,
    offsets_size: BinderUsize,
    data: DataUnion,
}

// Equivalent to struct binder_transaction_data_sg
#[derive(Clone, Copy, Zeroable, Pod)]
#[repr(C)]
pub(crate) struct TransactionDataSgRaw {
    transaction_data: TransactionDataRaw,
    buffers_size: BinderUsize,
}

// Equivalent to struct binder_transaction_data_secctx
#[derive(Clone, Copy, Zeroable, Pod)]
#[repr(C)]
pub(crate) struct TransactionDataSecctxRaw {
    transaction_data: TransactionDataRaw,
    secctx: BinderUsize,
}
