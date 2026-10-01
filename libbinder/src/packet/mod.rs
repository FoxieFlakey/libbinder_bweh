// The data for packet is de/serialized with serde
// For binder type to work serializer has to specially
// handle to keep track offset of each types. Binder's
// transaction need to know where those are

mod reader;
mod writer;

use std::{mem::ManuallyDrop, os::fd::AsFd, ptr, sync::Arc};

use anyhow::anyhow;
use either::Either;
// Right now the writer is raw format
// later make this more flexible OR
// make it the only writer
pub use libbinder_sys::transaction::TransactionFlag;
use libbinder_sys::{
    commands::Command,
    transaction::TransactionKernelManaged,
    types::{ObjectParsed, reference::ObjectRefLocal},
    write_read::binder_read_write,
};
use nix::errno::Errno;
pub use writer::Writer;

use crate::{Runtime, packet::reader::Reader};

struct Owned {
    data: Vec<u8>,
    offsets: Vec<usize>,
}

pub struct Packet {
    runtime: Arc<Runtime>,
    inner: Either<Owned, TransactionKernelManaged>,
    is_sent: bool,
}

impl Drop for Packet {
    fn drop(&mut self) {
        self.perform_cleanup()
    }
}

impl Packet {
    pub fn reader<'a>(&'a self) -> Reader<'a> {
        Reader::new(self)
    }

    pub(crate) fn from_kernel(runtime: Arc<Runtime>, kernel: TransactionKernelManaged) -> Self {
        Self {
            runtime: runtime,
            inner: Either::Right(kernel),
            is_sent: false,
        }
    }

    pub fn get_data(&self) -> &[u8] {
        match &self.inner {
            Either::Left(x) => &x.data,
            Either::Right(x) => x.get_data().data_slice,
        }
    }

    pub fn get_offsets(&self) -> &[usize] {
        match &self.inner {
            Either::Left(x) => &x.offsets,
            Either::Right(x) => x.get_data().offsets,
        }
    }

    fn perform_cleanup(&mut self) {
        match &self.inner {
            Either::Right(kernel) => {
                let mut cmd = Vec::new();
                cmd.extend_from_slice(&Command::FreeBuffer.as_bytes());
                cmd.extend_from_slice(&kernel.get_kernel_buf().to_ne_bytes());

                match binder_read_write(self.runtime.binder_dev.as_fd(), &cmd, &mut []) {
                    Ok((_, read_count)) => Ok(read_count),
                    Err((Errno::EAGAIN, (_, read_bytes))) => Ok(read_bytes),
                    Err((e, ..)) => Err(anyhow!("Cannot do BINDER_WRITE_READ: {e}")),
                }
                .expect("Cannot free binder kernel buffer");
            }

            Either::Left(owned) => {
                if !self.is_sent {
                    drop_objects(&self.runtime, &owned.data, &owned.offsets)
                }
            }
        }
    }

    pub fn recycle(self) -> Writer {
        assert!(
            self.inner.is_left(),
            "Cannot recycle packet containing kernel binder buffer"
        );

        let runtime = self.runtime.clone();
        let mut packet = ManuallyDrop::new(self);
        packet.perform_cleanup();

        // SAFETY: Just wanted to move 'inner' out without trigger drop code
        // because we already called cleanup. Note: kinda
        let mut owned = unsafe { ptr::read(&packet.inner) }.left().unwrap();

        owned.data.clear();
        owned.offsets.clear();
        Writer::new_recycled(runtime, owned.data, owned.offsets)
    }

    // Appropriately does needed strong count increments
    // # Safety
    // caller must call this only once, for each time
    // this packeet is sent. This directly will increment
    // necessary strong counters on each objects like local
    // references
    pub(crate) unsafe fn objects_sent(&mut self) {
        for_each_object(self.get_data(), self.get_offsets(), |object| match object {
            ObjectParsed::LocalReference(ObjectRefLocal { data, .. }) => {
                self.runtime
                    .local_objects
                    .get(data)
                    .expect("Cannot find object")
                    .control
                    .write()
                    .unwrap()
                    .has_strong = true;
            }
            ObjectParsed::RemoteReference(_) => (),
        });
        self.is_sent = true;
    }
}

fn for_each_object<F>(data: &[u8], offsets: &[usize], mut func: F)
where
    F: FnMut(ObjectParsed),
{
    for &offset in offsets {
        let ty = ObjectParsed::try_from_bytes(&data[offset..]).expect("expecting data is valid");
        func(ty)
    }
}

fn drop_objects(runtime: &Arc<Runtime>, data: &[u8], offsets: &[usize]) {
    for_each_object(data, offsets, |object| match object {
        ObjectParsed::LocalReference(ObjectRefLocal { data, .. }) => {
            let object = runtime.local_objects.get(data).unwrap();
            let control = object.control.read().unwrap();
            if control.has_strong || control.has_weak {
                // Kernel have reference to it, so do nothing
                // it will later sent BR_RELEASE and BR_DECREFS
                return;
            }

            let object = runtime.local_objects.take(data).unwrap();
            let control = object.control.read().unwrap();
            assert!(
                !control.has_strong && !control.has_weak,
                "Kernel pull reference form nowhere :<"
            );
        }
        ObjectParsed::RemoteReference(_) => (),
    });
}
