use libbinder_sys::{
    BinderUsize,
    transaction::{Transaction, TransactionKernelManaged},
    types::reference::ObjectRefLocal,
};

pub struct RetIterator<'a> {
    buf: &'a [u8],
}

impl<'buf> RetIterator<'buf> {
    // # Safety
    // caller must make sure that 'buf' is valid buffer written by kernel
    pub unsafe fn new(buf: &'buf [u8]) -> Self {
        Self { buf }
    }
}

pub enum RetVal<'buf> {
    Err(i32),
    Ok,
    FailedTransaction,
    Transaction(Transaction<'buf, 'buf>),
    TransactionComplete,
    Reply(Transaction<'buf, 'buf>),
    DeadBinder(#[expect(unused)] usize),
    DeadReply,
    SpawnLooper,
    AcquireStrong(ObjectRefLocal),
    ReleaseStrong(ObjectRefLocal),
    AcquireWeak(ObjectRefLocal),
    ReleaseWeak(ObjectRefLocal),
}

impl<'buf> Iterator for RetIterator<'buf> {
    type Item = RetVal<'buf>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.buf.is_empty() {
            return None;
        }

        let ret_val = libbinder_sys::commands::ReturnVal::try_from_bytes(
            <[u8; 4]>::try_from(&self.buf[..4]).expect("Cannot read code"),
        )
        .expect("Cannot parse binder return code");

        let advance_bytes;
        let payload = &self.buf[4..];
        let ret = Some(match ret_val {
            libbinder_sys::commands::ReturnVal::Error => {
                advance_bytes = 4;
                RetVal::Err(i32::from_ne_bytes(
                    <[u8; 4]>::try_from(&payload[..4]).expect("Cannot error code"),
                ))
            }
            libbinder_sys::commands::ReturnVal::Ok => {
                advance_bytes = 0;
                RetVal::Ok
            }
            libbinder_sys::commands::ReturnVal::Failed => {
                advance_bytes = 0;
                RetVal::FailedTransaction
            }
            libbinder_sys::commands::ReturnVal::Transaction => {
                // SAFETY: By having RetIterator instance, caller must make sure that 'buf' is valid
                // buffer coming from kernel
                advance_bytes = TransactionKernelManaged::bytes_needed();
                RetVal::Transaction(Transaction::KernelManaged(unsafe {
                    TransactionKernelManaged::from_bytes(
                        &payload[..TransactionKernelManaged::bytes_needed()],
                        false,
                    )
                }))
            }
            libbinder_sys::commands::ReturnVal::TransactionSecctx => {
                // SAFETY: By having RetIterator instance, caller must make sure that 'buf' is valid
                // buffer coming from kernel
                advance_bytes = TransactionKernelManaged::bytes_needed_for_secctx();
                RetVal::Transaction(Transaction::KernelManaged(unsafe {
                    TransactionKernelManaged::from_bytes_from_secctx(
                        &payload[..TransactionKernelManaged::bytes_needed_for_secctx()],
                    )
                }))
            }
            libbinder_sys::commands::ReturnVal::Reply => {
                // SAFETY: By having RetIterator instance, caller must make sure that 'buf' is valid
                // buffer coming from kernel
                advance_bytes = TransactionKernelManaged::bytes_needed();
                RetVal::Reply(Transaction::KernelManaged(unsafe {
                    TransactionKernelManaged::from_bytes(
                        &payload[..TransactionKernelManaged::bytes_needed()],
                        true,
                    )
                }))
            }
            libbinder_sys::commands::ReturnVal::TransactionComplete => {
                advance_bytes = 0;
                RetVal::TransactionComplete
            }
            libbinder_sys::commands::ReturnVal::DeadBinder => {
                advance_bytes = size_of::<BinderUsize>();
                let ret = BinderUsize::from_ne_bytes(
                    <[u8; size_of::<BinderUsize>()]>::try_from(
                        &payload[..size_of::<BinderUsize>()],
                    )
                    .expect("Cannot read cookie for dead binder"),
                );
                RetVal::DeadBinder(usize::try_from(ret).unwrap())
            }
            libbinder_sys::commands::ReturnVal::DeadReply => {
                advance_bytes = 0;
                RetVal::DeadReply
            }
            libbinder_sys::commands::ReturnVal::SpawnLooper => {
                advance_bytes = 0;
                RetVal::SpawnLooper
            }
            // Reached no-op, continue to next
            libbinder_sys::commands::ReturnVal::Noop => {
                self.buf = &self.buf[4..];
                return self.next();
            }
            libbinder_sys::commands::ReturnVal::Acquire => {
                advance_bytes = size_of::<usize>() * 2;
                let data = usize::from_ne_bytes(
                    <[u8; size_of::<usize>()]>::try_from(&payload[..size_of::<usize>()])
                        .expect("Cannot read 'data' portion of BR_ACQUIRE'"),
                );
                let extra_data = usize::from_ne_bytes(
                    <[u8; size_of::<usize>()]>::try_from(
                        &payload[size_of::<usize>()..size_of::<usize>() * 2],
                    )
                    .expect("Cannot read 'extra_data' portion of BR_ACQUIRE'"),
                );
                RetVal::AcquireStrong(ObjectRefLocal { data, extra_data })
            }
            libbinder_sys::commands::ReturnVal::Release => {
                advance_bytes = size_of::<usize>() * 2;
                let data = usize::from_ne_bytes(
                    <[u8; size_of::<usize>()]>::try_from(&payload[..size_of::<usize>()])
                        .expect("Cannot read 'data' portion of BR_RELEASE'"),
                );
                let extra_data = usize::from_ne_bytes(
                    <[u8; size_of::<usize>()]>::try_from(
                        &payload[size_of::<usize>()..size_of::<usize>() * 2],
                    )
                    .expect("Cannot read 'extra_data' portion of BR_RELEASE'"),
                );
                RetVal::ReleaseStrong(ObjectRefLocal { data, extra_data })
            }
            libbinder_sys::commands::ReturnVal::AcquireWeak => {
                advance_bytes = size_of::<usize>() * 2;
                let data = usize::from_ne_bytes(
                    <[u8; size_of::<usize>()]>::try_from(&payload[..size_of::<usize>()])
                        .expect("Cannot read 'data' portion of BR_INCREFS'"),
                );
                let extra_data = usize::from_ne_bytes(
                    <[u8; size_of::<usize>()]>::try_from(
                        &payload[size_of::<usize>()..size_of::<usize>() * 2],
                    )
                    .expect("Cannot read 'extra_data' portion of BR_INCREFS'"),
                );
                RetVal::AcquireWeak(ObjectRefLocal { data, extra_data })
            }
            libbinder_sys::commands::ReturnVal::ReleaseWeak => {
                advance_bytes = size_of::<usize>() * 2;
                let data = usize::from_ne_bytes(
                    <[u8; size_of::<usize>()]>::try_from(&payload[..size_of::<usize>()])
                        .expect("Cannot read 'data' portion of BR_DECREFS'"),
                );
                let extra_data = usize::from_ne_bytes(
                    <[u8; size_of::<usize>()]>::try_from(
                        &payload[size_of::<usize>()..size_of::<usize>() * 2],
                    )
                    .expect("Cannot read 'extra_data' portion of BR_DECREFS'"),
                );
                RetVal::ReleaseWeak(ObjectRefLocal { data, extra_data })
            }
            libbinder_sys::commands::ReturnVal::ClearDeathNotificationDone => todo!(),
            libbinder_sys::commands::ReturnVal::FrozenReply => todo!(),
            libbinder_sys::commands::ReturnVal::OneWaySpamSuspect => todo!(),
            libbinder_sys::commands::ReturnVal::TransactionPendingFrozen => todo!(),
            libbinder_sys::commands::ReturnVal::FrozenBinder => todo!(),
        });

        self.buf = &self.buf[advance_bytes + 4..];
        ret
    }
}
