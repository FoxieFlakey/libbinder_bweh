// The data for packet is de/serialized with serde
// For binder type to work serializer has to specially
// handle to keep track offset of each types. Binder's
// transaction need to know where those are

mod passthru;

use std::{
    mem::{self, ManuallyDrop},
    os::fd::{AsFd, OwnedFd},
    ptr,
    sync::Arc,
};

use anyhow::anyhow;
use either::Either;
// Right now the writer is raw format
// later make this more flexible OR
// make it the only writer
pub use libbinder_sys::transaction::TransactionFlag;
use libbinder_sys::{
    commands::Command, transaction::TransactionKernelManaged, types::ObjectParsed,
    write_read::binder_read_write,
};
use nix::errno::Errno;
pub use passthru::RawFormat as Writer;

use crate::object::Object;

struct Owned {
    data: Vec<u8>,
    offsets: Vec<usize>,
}

pub struct Packet {
    binder_dev: Option<Arc<OwnedFd>>,
    inner: Either<Owned, TransactionKernelManaged>,
}

impl Drop for Packet {
    fn drop(&mut self) {
        match &self.inner {
            Either::Right(kernel) => {
                let mut cmd = Vec::new();
                cmd.extend_from_slice(&Command::FreeBuffer.as_bytes());
                cmd.extend_from_slice(&kernel.get_kernel_buf().to_ne_bytes());

                match binder_read_write(self.binder_dev.take().unwrap().as_fd(), &cmd, &mut []) {
                    Ok((_, read_count)) => Ok(read_count),
                    Err((Errno::EAGAIN, (_, read_bytes))) => Ok(read_bytes),
                    Err((e, ..)) => Err(anyhow!("Cannot do BINDER_WRITE_READ: {e}")),
                }
                .expect("Cannot free binder kernel buffer");
            }

            Either::Left(owned) => drop_objects(&owned.data, &owned.offsets),
        }
    }
}

impl Writer {
    pub fn finish(mut self) -> Packet {
        Packet {
            binder_dev: None,
            inner: Either::Left(Owned {
                data: mem::take(&mut self.data),
                offsets: mem::take(&mut self.offsets),
            }),
        }
    }
}

impl Packet {
    pub fn into_owned(self) -> Self {
        let mut new_data = Vec::with_capacity(self.get_data().len());
        let mut new_offsets = Vec::with_capacity(self.get_offsets().len());

        new_data.extend_from_slice(self.get_data());
        new_offsets.extend_from_slice(self.get_offsets());

        Self {
            binder_dev: None,
            inner: Either::Left(Owned {
                data: new_data,
                offsets: new_offsets,
            }),
        }
    }

    pub(crate) fn from_kernel(binder_dev: Arc<OwnedFd>, kernel: TransactionKernelManaged) -> Self {
        Self {
            binder_dev: Some(binder_dev),
            inner: Either::Right(kernel),
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

    pub fn into_writer(self) -> Writer {
        let packet = ManuallyDrop::new(self.into_owned());
        // SAFETY: Just wanted to move 'inner' out without trigger drop code
        // the binder_dev must be None here, so no drop code need to run and drop code
        // for inner, is moved here to be dropped later
        let mut owned = unsafe { ptr::read(&packet.inner) }.left().unwrap();

        owned.data.clear();
        owned.offsets.clear();
        Writer {
            data: owned.data,
            offsets: owned.offsets,
        }
    }

    // Appropriately does needed strong count increments
    pub(crate) fn objects_sent(&self) {
        for_each_object(self.get_data(), self.get_offsets(), |object| match object {
            ObjectParsed::LocalReference(x) => {
                // SAFETY: Trust da kernel
                let reference = unsafe {
                    Arc::from_raw(ptr::with_exposed_provenance::<Box<dyn Object>>(x.data))
                };
                mem::forget(reference.clone());
                mem::forget(reference);
            }
            ObjectParsed::RemoteReference(_) => (),
        });
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

fn drop_objects(data: &[u8], offsets: &[usize]) {
    for_each_object(data, offsets, |object| match object {
        ObjectParsed::LocalReference(x) => {
            // SAFETY: Trust da kernel
            unsafe { Arc::from_raw(ptr::with_exposed_provenance::<Box<dyn Object>>(x.data)) };
        }
        ObjectParsed::RemoteReference(_) => (),
    });
}
