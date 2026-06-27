use anyhow::{Context, anyhow};
use bytemuck::{Pod, Zeroable};
use bytemuck_utils::PodData;

use crate::types::reference::ObjectRefRaw;

const TYPE_LARGE: u8 = 0x85;

const fn pack_chars(c1: u8, c2: u8, c3: u8, c4: u8) -> u32 {
    ((c1 as u32) << 24) | ((c2 as u32) << 16) | ((c3 as u32) << 8) | (c4 as u32)
}

pub(crate) const BINDER: u32 = pack_chars(b's', b'b', b'*', TYPE_LARGE);
pub(crate) const WEAK_BINDER: u32 = pack_chars(b'w', b'b', b'*', TYPE_LARGE);
pub(crate) const HANDLE: u32 = pack_chars(b's', b'h', b'*', TYPE_LARGE);
pub(crate) const WEAK_HANDLE: u32 = pack_chars(b'w', b'h', b'*', TYPE_LARGE);
pub(crate) const FD: u32 = pack_chars(b'f', b'd', b'*', TYPE_LARGE);
pub(crate) const FDA: u32 = pack_chars(b'f', b'd', b'a', TYPE_LARGE);
pub(crate) const PTR: u32 = pack_chars(b'p', b't', b'*', TYPE_LARGE);

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
