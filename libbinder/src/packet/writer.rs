use std::{mem, sync::Arc};

use either::Either;
use libbinder_sys::types::reference::{ObjectRef, ObjectRefLocal};

use crate::{
    Runtime,
    object::{B, ObjectTrait},
    packet::{Owned, Packet},
};

pub struct Writer {
    runtime: Arc<Runtime>,
    data: Vec<u8>,
    offsets: Vec<usize>,
}

impl Drop for Writer {
    fn drop(&mut self) {
        super::drop_objects(&self.runtime, &self.data, &self.offsets);
    }
}

impl Writer {
    pub fn new(runtime: Arc<Runtime>) -> Self {
        Self {
            runtime,
            data: Vec::new(),
            offsets: Vec::new(),
        }
    }

    pub fn new_recycled(runtime: Arc<Runtime>, mut data: Vec<u8>, mut offsets: Vec<usize>) -> Self {
        data.clear();
        offsets.clear();
        Self {
            runtime,
            data,
            offsets,
        }
    }

    pub fn get_data(&self) -> &[u8] {
        &self.data
    }

    pub fn get_offsets(&self) -> &[usize] {
        &self.offsets
    }

    pub fn clear(&mut self) {
        self.data.clear();
        self.offsets.clear();
    }

    pub fn finish(mut self) -> Packet {
        Packet {
            runtime: self.runtime.clone(),
            is_sent: false,
            inner: Either::Left(Owned {
                data: mem::take(&mut self.data),
                offsets: mem::take(&mut self.offsets),
            }),
        }
    }

    pub fn write_bytes<T>(&mut self, bytes: T)
    where
        T: AsRef<[u8]>,
    {
        self.data.extend_from_slice(bytes.as_ref());
    }

    pub fn write_reference(&mut self, reference: Arc<B<dyn ObjectTrait>>) {
        assert!(
            self.data.len().is_multiple_of(4),
            "Binder objects must be at offset of multiple of four"
        );

        self.offsets.push(self.data.len());
        let reference_raw = ObjectRef::Local(ObjectRefLocal {
            data: self.runtime.add_object(reference),
            extra_data: 0,
        });
        reference_raw.with_raw_bytes(|bytes| self.data.extend_from_slice(bytes));
    }
}
