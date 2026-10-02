use std::os::fd::{AsRawFd, BorrowedFd, FromRawFd, OwnedFd};

use anyhow::{Context, bail};
use bytemuck::{Pod, Zeroable};
use bytemuck_utils::PodData;

use crate::{BinderUsize, object, types::ObjectHeaderRaw};

pub const fn size_for_raw() -> usize {
    size_of::<FdRaw>()
}

pub fn with_raw_bytes<F, R>(fd: BorrowedFd<'_>, func: F) -> R
where
    F: FnOnce(&[u8]) -> R,
{
    let raw = FdRaw {
        header: ObjectHeaderRaw { kind: object::FD },
        cookie: 0,
        fd_and_pad: FdAndPadUnion {
            fd: fd.as_raw_fd() as u32,
        },
        pad_flags: 0,
    };

    func(bytemuck::bytes_of(&raw))
}

// NOTE: This takes ownership of FD from in here
// DO NOT call this twice, there would be two OwnedFd
// pointing to same FD
pub(crate) fn try_from_bytes(bytes: &[u8]) -> anyhow::Result<OwnedFd> {
    let header = PodData::<ObjectHeaderRaw>::try_from_bytes(&bytes[..size_of::<ObjectHeaderRaw>()])
        .context("Converting to raw object header")?;
    if header.kind != object::FD {
        bail!("Incorrect type passed for try_from_bytes")
    }

    let raw = PodData::<FdRaw>::try_from_bytes(bytes).context("Converting to raw fd")?;

    // SAFETY: nah lets ball
    let fd = unsafe { raw.fd_and_pad.fd };
    Ok(
        // SAFETY: Kernel make the FD valid :3
        unsafe { OwnedFd::from_raw_fd(fd as i32) },
    )
}

#[derive(Clone, Copy, Zeroable, Pod)]
#[repr(C)]
struct FdRaw {
    header: ObjectHeaderRaw,
    pad_flags: u32,
    fd_and_pad: FdAndPadUnion,
    cookie: BinderUsize,
}

#[repr(C)]
#[derive(Clone, Copy, Zeroable)]
union FdAndPadUnion {
    pad_binder: BinderUsize,
    fd: u32,
}

unsafe impl Pod for FdAndPadUnion {}
