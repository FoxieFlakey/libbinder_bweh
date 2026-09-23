use std::slice;

use bytemuck_utils::PodData;
use enumflags2::BitFlags;

use crate::{
    BinderUsize, ObjectRefLocal,
    object::reference::{ObjectRef, ObjectRefRemote},
    transaction::{
        BinderOrHandleUnion, BufferStruct, DataUnion, TransactionDataCommon, TransactionDataRaw,
    },
};

#[derive(Clone)]
pub struct TransactionKernelManaged {
    // Cannot specifically make 'static is placeholder mean
    // as long as this struct alive. The getter method turn
    // it into proper borrow to ensure that by time when. Drop
    // runs this is dropped first and safe
    data: TransactionDataCommon<'static, 'static>,
    kernel_buf: BinderUsize,
}

impl TransactionKernelManaged {
    // Note: We placed fake empty slices, which will be restored
    // don't change the slices. That is to ensure references to
    // buffer don't escape as 'static well just a placeholder
    //
    // To meaning "dynamic lifetime" bound to kernel's buffer that
    // will be free'd later. I could use 'yoke' but it ended up
    // becoming more complicated than necessary
    pub fn with_data_mut<F: FnOnce(&mut TransactionDataCommon<'static, 'static>) -> R, R>(
        &mut self,
        func: F,
    ) -> R {
        let (buffer_slice_saved, offsets_slice_saved) = (self.data.data_slice, self.data.offsets);
        self.data.data_slice = &[];
        self.data.offsets = &[];

        let ret = func(&mut self.data);

        self.data.data_slice = buffer_slice_saved;
        self.data.offsets = offsets_slice_saved;
        return ret;
    }

    pub fn get_kernel_buf(&self) -> BinderUsize {
        self.kernel_buf
    }

    pub fn with_bytes<F: FnOnce(&[u8]) -> R, R>(&self, func: F) -> R {
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

    pub fn get_data<'a>(&'a self) -> &'a TransactionDataCommon<'a, 'a> {
        &self.data
    }

    pub fn bytes_needed() -> usize {
        size_of::<TransactionDataRaw>()
    }

    // SAFETY: The 'bytes' has to be from kernel from the correct binder_dev
    // and the bytes assumed to be from BR_TRANSACTION/BR_REPLY
    //
    // The 'bytes' alignment can be unaligned, and its fine
    pub unsafe fn from_bytes(bytes: &[u8], is_reply: bool) -> Self {
        let raw = PodData::<TransactionDataRaw>::try_from_bytes(bytes)
            .expect("Cannot convert bytes to transaction data raw");

        // SAFETY: The buffers data as far as 'static concerned lives longer
        // before the 'static reference gone
        let data_slice: &'static [u8] =
            unsafe { slice::from_raw_parts(raw.data.ptr.buffer as *mut _, raw.data_size) };
        let offsets: &'static [usize] = unsafe {
            slice::from_raw_parts(
                raw.data.ptr.offsets as *mut _,
                raw.offsets_size / size_of::<usize>(),
            )
        };

        Self {
            kernel_buf: unsafe { raw.data.ptr.buffer },
            data: TransactionDataCommon {
                code: raw.code,
                target: if is_reply {
                    ObjectRef::Remote(ObjectRefRemote {
                        data_handle: unsafe { raw.target.handle },
                        extra_local_data: raw.extra_data,
                    })
                } else {
                    ObjectRef::Local(ObjectRefLocal {
                        data: unsafe { raw.target.binder },
                        extra_data: raw.extra_data,
                    })
                },
                flags: BitFlags::from_bits(raw.flags).ok().unwrap(),
                data_slice,
                offsets,
            },
        }
    }
}
