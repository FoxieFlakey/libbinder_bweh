use libbinder_sys::{
    BinderUsize,
    transaction::{Transaction, TransactionKernelManaged},
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
    Transaction(Transaction<'buf, 'buf>),
    TransactionComplete,
    Reply(Transaction<'buf, 'buf>),
    DeadBinder(usize),
    DeadReply,
    SpawnLooper,
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
            _ => todo!(),
        });

        self.buf = &self.buf[advance_bytes + 4..];
        ret
    }
}
