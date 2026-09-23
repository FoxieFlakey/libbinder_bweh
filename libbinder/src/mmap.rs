use std::{ffi::c_void, num::NonZero, os::fd::BorrowedFd, ptr::NonNull};

use nix::sys::mman::{MapFlags, ProtFlags};

pub struct Mmap {
    ptr: NonNull<c_void>,
    len: usize,
}

impl Drop for Mmap {
    fn drop(&mut self) {
        unsafe { nix::sys::mman::munmap(self.ptr, self.len) }.unwrap();
    }
}

unsafe impl Sync for Mmap {}
unsafe impl Send for Mmap {}

impl Mmap {
    pub fn new(fd: BorrowedFd<'_>, len: usize) -> anyhow::Result<Self> {
        Ok(Self {
            // SAFETY: We never deref this, this only exists so binder driver can use?
            ptr: unsafe {
                nix::sys::mman::mmap(
                    None,
                    NonZero::new(len).unwrap(),
                    ProtFlags::PROT_READ,
                    MapFlags::MAP_PRIVATE,
                    fd,
                    0,
                )?
            },
            len,
        })
    }
}
