use std::os::fd::OwnedFd;

use anyhow::{Context, anyhow};
use bytemuck::{Pod, Zeroable};
use bytemuck_utils::PodData;

use crate::types::{
    buffer::Buffer,
    reference::{ObjectRef, ObjectRefLocal, ObjectRefRaw, ObjectRefRemote},
};

const TYPE_LARGE: u8 = 0x85;

const fn pack_chars(c1: u8, c2: u8, c3: u8, c4: u8) -> u32 {
    ((c1 as u32) << 24) | ((c2 as u32) << 16) | ((c3 as u32) << 8) | (c4 as u32)
}

pub const FLAT_BINDER_FLAG_PRIORITY_MASK: u32 = 0xff;
pub const FLAT_BINDER_FLAG_ACCEPTS_FDS: u32 = 0x100;
pub const FLAT_BINDER_FLAG_TXN_SECURITY_CTX: u32 = 0x1000;

pub(crate) const BINDER: u32 = pack_chars(b's', b'b', b'*', TYPE_LARGE);
pub(crate) const WEAK_BINDER: u32 = pack_chars(b'w', b'b', b'*', TYPE_LARGE);
pub(crate) const HANDLE: u32 = pack_chars(b's', b'h', b'*', TYPE_LARGE);
pub(crate) const WEAK_HANDLE: u32 = pack_chars(b'w', b'h', b'*', TYPE_LARGE);
pub(crate) const FD: u32 = pack_chars(b'f', b'd', b'*', TYPE_LARGE);
pub(crate) const FDA: u32 = pack_chars(b'f', b'd', b'a', TYPE_LARGE);
pub(crate) const PTR: u32 = pack_chars(b'p', b't', b'*', TYPE_LARGE);

pub mod buffer;
pub mod fd;
pub mod reference;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Type {
    RemoteReference,
    LocalReference,
    WeakRemoteReference,
    WeakLocalReference,
    FileDescriptor,
    FileDescriptorArray,
    ByteBuffer,
}

impl Type {
    pub fn bytes_needed() -> usize {
        size_of::<ObjectHeaderRaw>()
    }

    // This is alignment for the object offset inside
    // the data buffer (the alignment of the data buffer itself is ignored)
    pub fn alignment_in_buffer_needed() -> usize {
        // All requires 32-bit aligned
        // See https://github.com/torvalds/linux/blob/3f9f0252130e7dd60d41be0802bf58f6471c691d/drivers/android/binder.c#L1792
        size_of::<u32>()
    }

    pub fn from_bytes(bytes: &[u8]) -> Type {
        Self::try_from_bytes(bytes).unwrap()
    }

    // Tells how many bytes needed for given type
    pub fn type_size_with_header(&self) -> usize {
        match self {
            Type::LocalReference => size_of::<ObjectRefRaw>(),
            Type::RemoteReference => size_of::<ObjectRefRaw>(),
            Type::ByteBuffer => buffer::Buffer::size_for_raw(),
            Type::FileDescriptor => fd::size_for_raw(),

            _ => todo!(),
        }
    }

    pub fn try_from_bytes(bytes: &[u8]) -> anyhow::Result<Type> {
        let raw = PodData::<ObjectHeaderRaw>::try_from_bytes(bytes)
            .context("Converting to raw object header")?;
        match raw.kind {
            BINDER => Ok(Type::LocalReference),
            HANDLE => Ok(Type::RemoteReference),
            WEAK_BINDER => Ok(Type::WeakLocalReference),
            WEAK_HANDLE => Ok(Type::WeakRemoteReference),
            FD => Ok(Type::FileDescriptor),
            FDA => Ok(Type::FileDescriptorArray),
            PTR => Ok(Type::ByteBuffer),
            _ => Err(anyhow!("Unknown object type")),
        }
    }
}

// Equivalent to struct binder_object_header
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub(crate) struct ObjectHeaderRaw {
    pub(crate) kind: u32,
}

pub enum ObjectParsed {
    LocalReference(ObjectRefLocal),
    RemoteReference(ObjectRefRemote),
    ByteBuffer(buffer::Buffer<'static>),
    Fd(OwnedFd),
}

impl ObjectParsed {
    // # Safety
    // Caller must ensure if there pointers in here
    // it has to be valid and any objects with lifetime
    // is 'static because limitation, its caller responsiblity
    // to anchor it something safer!
    // Also has to ensure only try_from_bytes on single instance
    // of bytes. Some object takes exclusive ownership of FD and such
    pub unsafe fn try_from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        let ty = Type::try_from_bytes(&bytes[..size_of::<ObjectHeaderRaw>()])
            .context("Cannot get type")?;
        Ok(match ty {
            Type::RemoteReference => {
                let payload = &bytes[..ty.type_size_with_header()];
                ObjectParsed::RemoteReference(
                    match ObjectRef::try_from_bytes(payload)
                        .context("Cannot parse remote reference")?
                    {
                        ObjectRef::Local(_) => unreachable!(),
                        ObjectRef::Remote(x) => x,
                    },
                )
            }
            Type::LocalReference => {
                let payload = &bytes[..ty.type_size_with_header()];
                ObjectParsed::LocalReference(
                    match ObjectRef::try_from_bytes(payload)
                        .context("Cannot parse local reference")?
                    {
                        ObjectRef::Local(x) => x,
                        ObjectRef::Remote(_) => unreachable!(),
                    },
                )
            }
            Type::WeakRemoteReference => todo!(),
            Type::WeakLocalReference => todo!(),
            Type::FileDescriptor => {
                let payload = &bytes[..ty.type_size_with_header()];
                ObjectParsed::Fd(fd::try_from_bytes(payload).context("Cannot parse FD object")?)
            }
            Type::FileDescriptorArray => todo!(),
            Type::ByteBuffer => {
                let payload = &bytes[..ty.type_size_with_header()];
                // SAFETY: Caller ensure that pointer in byte buffer is valid if its
                // byte buffer
                ObjectParsed::ByteBuffer(unsafe { Buffer::try_from_bytes(payload)? })
            }
        })
    }
}
