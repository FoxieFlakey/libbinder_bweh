use libbinder_sys::types::reference::ObjectRef;

use crate::packet::Packet;

// A very simple format, passing data as it is
// it is unportable because usize and platform
// dependant type gets encoded too.
//
// TODO: Turn this into trait
pub struct RawFormat {
    pub(super) data: Vec<u8>,
    pub(super) offsets: Vec<usize>,
}

impl RawFormat {
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

    pub fn write_u8(&mut self, data: u8) {
        self.data.extend_from_slice(&data.to_ne_bytes());
    }

    pub fn write_u16(&mut self, data: u16) {
        self.data.extend_from_slice(&data.to_ne_bytes());
    }

    pub fn write_u32(&mut self, data: u32) {
        self.data.extend_from_slice(&data.to_ne_bytes());
    }

    pub fn write_u64(&mut self, data: u64) {
        self.data.extend_from_slice(&data.to_ne_bytes());
    }

    pub fn write_usize(&mut self, data: usize) {
        self.data.extend_from_slice(&data.to_ne_bytes());
    }

    pub fn write_reference(&mut self, reference: &ObjectRef) {
        assert!(
            self.data.len().is_multiple_of(4),
            "Binder objects must be at offset of multiple of four"
        );

        self.offsets.push(self.data.len());
        reference.with_raw_bytes(|bytes| self.data.extend_from_slice(bytes))
    }
}
