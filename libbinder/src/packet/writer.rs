use std::{
    mem,
    os::fd::{AsFd, BorrowedFd},
    sync::Arc,
};

use aligned_vec::AVec;
use anyhow::{Context, anyhow};
use either::Either;
use libbinder_sys::types::{buffer::Buffer, fd, reference::ObjectRefLocal};
use nix::fcntl::FdFlag;

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

    pub fn get_current_offset(&self) -> usize {
        self.data.len()
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

    pub fn min_object_align() -> usize {
        4
    }

    pub fn write_fd(&mut self, fd: BorrowedFd<'_>) -> anyhow::Result<()> {
        assert!(
            self.data.len().is_multiple_of(Self::min_object_align()),
            "Binder objects must be at offset of multiple of four"
        );
        let offset = self.data.len();
        let new_fd = nix::unistd::dup(fd).context("Duping an FD")?;

        let mut flags = FdFlag::from_bits(
            nix::fcntl::fcntl(new_fd.as_fd(), nix::fcntl::F_GETFD).context("Getting FD flags")?,
        )
        .ok_or(anyhow!("Cannot create FdFlag"))?;
        flags |= FdFlag::FD_CLOEXEC;
        nix::fcntl::fcntl(new_fd.as_fd(), nix::fcntl::F_SETFD(flags))
            .context("Setting FD flags to have O_CLOEXEC")?;

        fd::with_raw_bytes(new_fd.as_fd(), |x| self.data.extend_from_slice(x));
        self.offsets.push(offset);

        // The fd will be dropped later in Drop code of Writer
        // when parses the offset for object need cleanups
        mem::forget(new_fd);
        Ok(())
    }

    pub fn write_buf<T: AsRef<[u8]> + 'static>(&mut self, bytes: T) {
        assert!(
            self.data.len().is_multiple_of(Self::min_object_align()),
            "Binder objects must be at offset of multiple of four"
        );
        let boxed;
        let addr = bytes.as_ref().as_ptr().addr();
        let raw = if !addr.is_multiple_of(size_of::<u64>()) {
            // Unaligned &[u8] was given, make it aligned
            let bytes = bytes.as_ref();
            let mut aligned: AVec<u8> = AVec::with_capacity(size_of::<u64>(), bytes.len());
            aligned.extend_from_slice(bytes);
            boxed = Box::new(aligned) as Box<dyn AsRef<[u8]>>;
            Buffer {
                buffer: boxed.as_ref().as_ref(),
                parent: None,
            }
        } else {
            boxed = Box::new(bytes) as Box<dyn AsRef<[u8]>>;
            assert_eq!(
                boxed.as_ref().as_ref().as_ptr().addr(),
                addr,
                "as_ref changes value between 2 calls?? should not happen"
            );
            Buffer {
                buffer: boxed.as_ref().as_ref(),
                parent: None,
            }
        };

        let offset = self.data.len();

        raw.with_raw_bytes(|bytes| self.data.extend_from_slice(bytes));
        self.buffers_size += raw.buffer.len();
        self.byte_bufs.push(boxed);
        self.offsets.push(offset);
    }

    pub fn write_reference(&mut self, reference: Arc<B<dyn ObjectTrait>>) {
        assert!(
            self.data.len().is_multiple_of(Self::min_object_align()),
            "Binder objects must be at offset of multiple of four"
        );
        let offset = self.data.len();

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

        self.offsets.push(offset);
    }
}
