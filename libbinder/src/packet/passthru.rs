use std::sync::Arc;

use libbinder_sys::types::reference::{ObjectRef, ObjectRefLocal};

use crate::{object::Object, packet::Packet};

pub struct Writer {
    pub(super) data: Vec<u8>,
    pub(super) offsets: Vec<usize>,
}

impl Drop for Writer {
    fn drop(&mut self) {
        super::drop_objects(&self.data, &self.offsets);
    }
}

impl Writer {
    pub fn new() -> Self {
        Self {
            data: Vec::new(),
            offsets: Vec::new(),
        }
    }

    pub fn copy_from(&mut self, other: &Packet) {
        self.data.clear();
        self.offsets.clear();
        self.data.extend_from_slice(other.get_data());
        self.offsets.extend_from_slice(other.get_offsets());
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

    pub fn write_bytes<T>(&mut self, bytes: T)
    where
        T: AsRef<[u8]>,
    {
        self.data.extend_from_slice(bytes.as_ref());
    }

    pub fn write_reference(&mut self, reference: Arc<Box<dyn Object>>) {
        assert!(
            self.data.len().is_multiple_of(4),
            "Binder objects must be at offset of multiple of four"
        );

        self.offsets.push(self.data.len());
        let reference_raw = ObjectRef::Local(ObjectRefLocal {
            data: Arc::into_raw(reference).addr(),
            extra_data: 0,
        });
        reference_raw.with_raw_bytes(|bytes| self.data.extend_from_slice(bytes));
    }
}
