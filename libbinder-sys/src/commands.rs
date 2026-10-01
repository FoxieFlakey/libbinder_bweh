use anyhow::Context;
use bytemuck::{Pod, Zeroable};
use bytemuck_utils::PodData;
use nix::{request_code_none, request_code_read, request_code_write};
use num_enum::{TryFromPrimitive, TryFromPrimitiveError};

use crate::{
    BinderUsize,
    transaction::{TransactionDataRaw, TransactionDataSecctxRaw, TransactionDataSgRaw},
};

const BINDER_CMD_MAGIC: u8 = b'c';

#[repr(i32)]
#[derive(Debug, Clone, Copy, TryFromPrimitive)]
pub enum Command {
    Acquire = request_code_write!(BINDER_CMD_MAGIC, 5, size_of::<u32>()),
    Release = request_code_write!(BINDER_CMD_MAGIC, 6, size_of::<u32>()),
    AcquireWeak = request_code_write!(BINDER_CMD_MAGIC, 4, size_of::<u32>()),
    ReleaseWeak = request_code_write!(BINDER_CMD_MAGIC, 7, size_of::<u32>()),
    AcquireWeakDone = request_code_write!(BINDER_CMD_MAGIC, 8, size_of::<PtrCookieRaw>()),
    AcquireDone = request_code_write!(BINDER_CMD_MAGIC, 9, size_of::<PtrCookieRaw>()),
    SendTransaction = request_code_write!(BINDER_CMD_MAGIC, 0, size_of::<TransactionDataRaw>()),
    SendReply = request_code_write!(BINDER_CMD_MAGIC, 1, size_of::<TransactionDataRaw>()),
    FreeBuffer = request_code_write!(BINDER_CMD_MAGIC, 3, size_of::<BinderUsize>()),
    RegisterLooper = request_code_none!(BINDER_CMD_MAGIC, 11),
    EnterLooper = request_code_none!(BINDER_CMD_MAGIC, 12),
    ExitLooper = request_code_none!(BINDER_CMD_MAGIC, 13),

    // death notification
    RequestDeathNotification = request_code_write!(BINDER_RET_MAGIC, 14, size_of::<PtrCookieRaw>()),
    ClearDeathNotification = request_code_write!(BINDER_RET_MAGIC, 15, size_of::<PtrCookieRaw>()),
    DeathNotificationDone = request_code_write!(BINDER_RET_MAGIC, 16, size_of::<PtrCookieRaw>()),

    SendTransactionSG =
        request_code_write!(BINDER_RET_MAGIC, 17, size_of::<TransactionDataSgRaw>()),
    SendReplySG = request_code_write!(BINDER_RET_MAGIC, 18, size_of::<TransactionDataSgRaw>()),

    // Freeze notification
    RequestFreezeNotification =
        request_code_write!(BINDER_RET_MAGIC, 19, size_of::<PtrCookieRaw>()),
    ClearFreezeNotification = request_code_write!(BINDER_RET_MAGIC, 20, size_of::<PtrCookieRaw>()),
    FreezeNotificationDone = request_code_write!(BINDER_RET_MAGIC, 21, size_of::<PtrCookieRaw>()),
}

impl Command {
    pub fn as_bytes(self) -> [u8; 4] {
        (self as u32).to_ne_bytes()
    }
}

const BINDER_RET_MAGIC: u8 = b'r';

#[derive(Pod, Zeroable, Clone, Copy)]
#[repr(C)]
pub struct PtrCookieRaw {
    pub ptr: BinderUsize,
    pub cookie: BinderUsize,
}

impl PtrCookieRaw {
    // Unaligned read does not matter
    // any bit pattern is correct
    pub fn try_from_raw_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        Ok(PodData::unwrap(PodData::to_owned(
            PodData::<PtrCookieRaw>::try_from_bytes(bytes).context("")?,
        )))
    }
}

#[repr(i32)]
#[derive(Debug, Clone, Copy, TryFromPrimitive)]
pub enum ReturnVal {
    Error = request_code_read!(BINDER_RET_MAGIC, 0, size_of::<i32>()),
    Ok = request_code_none!(BINDER_RET_MAGIC, 1),
    Transaction = request_code_read!(BINDER_RET_MAGIC, 2, size_of::<TransactionDataRaw>()),
    TransactionSecctx =
        request_code_read!(BINDER_RET_MAGIC, 2, size_of::<TransactionDataSecctxRaw>()),
    Reply = request_code_read!(BINDER_RET_MAGIC, 3, size_of::<TransactionDataRaw>()),
    DeadReply = request_code_none!(BINDER_RET_MAGIC, 5),
    TransactionComplete = request_code_none!(BINDER_RET_MAGIC, 6),
    Noop = request_code_none!(BINDER_RET_MAGIC, 12),
    SpawnLooper = request_code_none!(BINDER_RET_MAGIC, 13),
    DeadBinder = request_code_read!(BINDER_RET_MAGIC, 15, size_of::<BinderUsize>()),
    Failed = request_code_none!(BINDER_RET_MAGIC, 17),
    Acquire = request_code_read!(BINDER_RET_MAGIC, 8, size_of::<PtrCookieRaw>()),
    AcquireWeak = request_code_read!(BINDER_RET_MAGIC, 7, size_of::<PtrCookieRaw>()),
    Release = request_code_read!(BINDER_RET_MAGIC, 9, size_of::<PtrCookieRaw>()),
    ReleaseWeak = request_code_read!(BINDER_RET_MAGIC, 10, size_of::<PtrCookieRaw>()),
    ClearDeathNotificationDone = request_code_read!(BINDER_RET_MAGIC, 16, size_of::<BinderUsize>()),
    FrozenReply = request_code_none!(BINDER_RET_MAGIC, 18),
    OneWaySpamSuspect = request_code_none!(BINDER_RET_MAGIC, 19),
    TransactionPendingFrozen = request_code_none!(BINDER_RET_MAGIC, 20),
    FrozenBinder = request_code_read!(
        BINDER_RET_MAGIC,
        21,
        size_of::<crate::BinderFrozenStateInfo>()
    ),
}

impl ReturnVal {
    pub fn try_from_bytes(bytes: [u8; 4]) -> Result<Self, TryFromPrimitiveError<Self>> {
        Self::try_from_primitive(i32::from_ne_bytes(bytes))
    }
}
