use std::{ptr, slice};

use anyhow::{Context, bail};
use bytemuck::{Pod, Zeroable};
use bytemuck_utils::PodData;

use crate::{BinderUsize, object, types::ObjectHeaderRaw};

#[derive(Clone, Copy, Zeroable, Pod)]
#[repr(C)]
struct BufferRaw {
    header: ObjectHeaderRaw,
    flags: u32,
    buffer: BinderUsize,
    length: BinderUsize,
    // This is index of object in 'offset' list
    parent: BinderUsize,
    parent_offset: BinderUsize,
}

const BINDER_BUFFER_FLAG_HAS_PARENT: u32 = 0x01;

// This is only borrow the buffer. It won't frees
// the underlying data
#[derive(Clone)]
pub struct Buffer<'a> {
    pub buffer: &'a [u8],
    pub parent: Option<Parent>,
}

impl<'a> Buffer<'a> {
    pub const fn size_for_raw() -> usize {
        size_of::<BufferRaw>()
    }

    // The raw contains pointer, the 'a has to
    // stay alive till usage of BufferRaw done
    pub fn with_raw_bytes<F, R>(&self, func: F) -> R
    where
        F: FnOnce(&[u8]) -> R,
    {
        let mut raw = BufferRaw {
            header: ObjectHeaderRaw { kind: object::PTR },
            buffer: self.buffer.as_ptr().addr(),
            length: self.buffer.len(),
            flags: 0,
            parent: 0,
            parent_offset: 0,
        };

        if let Some(x) = &self.parent {
            raw.flags |= BINDER_BUFFER_FLAG_HAS_PARENT;
            raw.parent = x.index;
            raw.parent_offset = x.offset;
        }

        func(bytemuck::bytes_of(&raw))
    }

    // # Safety
    // caller must ensure the buffer raw, contains valid pointer
    // such as from kernel, which is guarantee to be valid
    pub(crate) unsafe fn try_from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        let header = PodData::<ObjectHeaderRaw>::try_from_bytes(bytes)
            .context("Converting to raw object header")?;
        if header.kind != object::PTR {
            bail!("Incorrect type passed for try_from_bytes")
        }
        let raw =
            PodData::<BufferRaw>::try_from_bytes(bytes).context("Converting to raw buffer")?;

        Ok(Buffer {
            // SAFETY: Caller ensure that if this is valid buffer it contains valid pointer
            buffer: unsafe {
                slice::from_raw_parts(ptr::with_exposed_provenance(raw.buffer), raw.length)
            },
            parent: if raw.flags & BINDER_BUFFER_FLAG_HAS_PARENT != 0 {
                Some(Parent {
                    index: raw.parent,
                    offset: raw.parent_offset,
                })
            } else {
                None
            },
        })
    }
}

#[derive(Clone)]
pub struct Parent {
    pub index: BinderUsize,
    pub offset: BinderUsize,
}
