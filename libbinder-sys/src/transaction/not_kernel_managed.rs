use crate::{
    object::reference::ObjectRef,
    transaction::{
        BinderOrHandleUnion, BufferStruct, DataUnion, TransactionDataCommon, TransactionDataRaw,
        TransactionDataSgRaw,
    },
};

#[derive(Clone)]
pub struct TransactionNotKernelManangedSg<'buffer, 'buffer_offsets> {
    pub data: TransactionNotKernelMananged<'buffer, 'buffer_offsets>,
    pub buffers_size: usize,
}

impl TransactionNotKernelManangedSg<'_, '_> {
    pub fn with_bytes<R, F: FnOnce(&[u8]) -> R>(&self, func: F) -> R {
        let raw = self.as_raw();
        let bytes = bytemuck::bytes_of(&raw);
        func(bytes)
    }

    fn as_raw(&self) -> TransactionDataSgRaw {
        TransactionDataSgRaw {
            transaction_data: self.data.as_raw(),
            buffers_size: self.buffers_size,
        }
    }
}

#[derive(Clone)]
pub struct TransactionNotKernelMananged<'buffer, 'buffer_offsets> {
    pub data: TransactionDataCommon<'buffer, 'buffer_offsets>,
}

impl TransactionNotKernelMananged<'_, '_> {
    pub fn with_bytes<R, F: FnOnce(&[u8]) -> R>(&self, func: F) -> R {
        let raw = self.as_raw();
        let bytes = bytemuck::bytes_of(&raw);
        func(bytes)
    }

    fn as_raw(&self) -> TransactionDataRaw {
        let (target, extra_data) = match &self.data.target {
            ObjectRef::Local(x) => (BinderOrHandleUnion { binder: x.data }, x.extra_data),
            ObjectRef::Remote(x) => (
                BinderOrHandleUnion {
                    handle: x.data_handle,
                },
                0,
            ),
        };

        TransactionDataRaw {
            data_size: self.data.data_slice.len(),
            offsets_size: self.data.offsets.len() * size_of::<usize>(),
            sender_pid: 0,
            sender_uid: 0,
            flags: self.data.flags.bits(),
            code: self.data.code,
            data: DataUnion {
                ptr: BufferStruct {
                    buffer: self.data.data_slice.as_ptr().addr(),
                    offsets: self.data.offsets.as_ptr().addr(),
                },
            },
            extra_data,
            target,
        }
    }
}
