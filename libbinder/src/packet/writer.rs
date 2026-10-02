use std::{mem, sync::Arc};

use either::Either;
use libbinder_sys::types::{buffer::Buffer, reference::ObjectRefLocal};

use crate::{
    Runtime,
    object::{B, ObjectTrait},
    packet::{Owned, Packet},
};

pub struct Writer {
    runtime: Arc<Runtime>,
    data: Vec<u8>,
    offsets: Vec<usize>,
    byte_bufs: Vec<Box<dyn AsRef<[u8]>>>,
    buffers_size: usize,
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
            byte_bufs: Vec::new(),
            buffers_size: 0,
        }
    }

    pub fn new_recycled(
        runtime: Arc<Runtime>,
        mut data: Vec<u8>,
        mut offsets: Vec<usize>,
        mut byte_bufs: Vec<Box<dyn AsRef<[u8]>>>,
    ) -> Self {
        data.clear();
        offsets.clear();
        byte_bufs.clear();
        Self {
            runtime,
            data,
            offsets,
            byte_bufs,
            buffers_size: 0,
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
        let mut buffers_size = mem::take(&mut self.buffers_size);
        // PRETTY random but required in https://github.com/torvalds/linux/blob/ce1e0223d8ad4211275c82a17ed6d43ab81e13d9/drivers/android/binder.c#L3458
        // its checked
        if !buffers_size.is_multiple_of(size_of::<u64>()) {
            buffers_size = buffers_size.next_multiple_of(size_of::<u64>())
        }

        Packet {
            runtime: self.runtime.clone(),
            is_sent: false,
            inner: Either::Left(Owned {
                data: mem::take(&mut self.data),
                offsets: mem::take(&mut self.offsets),
                byte_bufs: mem::take(&mut self.byte_bufs),
            }),
            buffers_size,
        }
    }

    pub fn write_bytes<T>(&mut self, bytes: T)
    where
        T: AsRef<[u8]>,
    {
        self.data.extend_from_slice(bytes.as_ref());
    }

    pub fn write_buf<T: AsRef<[u8]> + 'static>(&mut self, bytes: T) {
        assert!(
            self.data.len().is_multiple_of(4),
            "Binder objects must be at offset of multiple of four"
        );
        if !bytes.as_ref().len().is_multiple_of(size_of::<u64>()) {
            todo!(
                "Maybe bounce buffers and a warning or smth. kernel requires it aligned to 8 bytes"
            )
        }

        let raw = Buffer {
            buffer: bytes.as_ref(),
            parent: None,
        };

        raw.with_raw_bytes(|bytes| self.data.extend_from_slice(bytes));
        self.byte_bufs.push(Box::new(bytes));
    }

    pub fn write_reference(&mut self, reference: Arc<B<dyn ObjectTrait>>) {
        assert!(
            self.data.len().is_multiple_of(4),
            "Binder objects must be at offset of multiple of four"
        );

        self.offsets.push(self.data.len());

        // Special handling if its remote
        if let Some(remote) = reference.get_remote() {
            // We also 'clone' ownership of the remote reference
            self.runtime.inc_remote_ref(
                &remote
                    .reference
                    .as_ref()
                    .right()
                    .expect("get_remote returned local proxy instead of remote"),
            );
            remote
                .reference
                .as_ref()
                .right()
                .expect("get_remote impls, return non remote proxy!")
                .with_raw_bytes(|bytes| self.data.extend_from_slice(bytes))
        } else {
            let flags = reference.flags.into_flags();
            let raw = ObjectRefLocal {
                data: self.runtime.add_object(reference),
                extra_data: 0,
            };

            raw.with_raw_bytes_and_flag(flags, |bytes| self.data.extend_from_slice(bytes))
        }
    }
}
