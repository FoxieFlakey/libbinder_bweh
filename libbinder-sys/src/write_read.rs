use std::os::fd::{AsRawFd, BorrowedFd};

use nix::errno::Errno;

use crate::{BinderUsize, ioctl};

#[repr(C)]
pub(crate) struct ReadWrite {
    write_buffer_size: BinderUsize,
    write_buffer_consumed: BinderUsize,
    write_buffer: BinderUsize,

    read_buffer_size: BinderUsize,
    read_buffer_filled_size: BinderUsize,
    read_buffer: BinderUsize,
}

// Returns to new subslice where .0 is bytes that kernel read
// from write buffer and .1 is bytes that kernel written to the
// read buffer and return the same as sucess with additional error
// so .0 bytes kernel read from write buf and .1 is bytes that
// kernel writes to read buffer
//
// On error the content of read buffer is not determined
pub fn binder_read_write(
    fd: BorrowedFd,
    write_buf: &[u8],
    read_buf: &mut [u8],
) -> Result<(usize, usize), (Errno, (usize, usize))> {
    let mut rw = ReadWrite {
        read_buffer_filled_size: 0,
        write_buffer_consumed: 0,

        write_buffer: write_buf.as_ptr().addr(),
        write_buffer_size: write_buf.len(),

        read_buffer: read_buf.as_ptr().addr(),
        read_buffer_size: read_buf.len(),
    };

    unsafe { ioctl::ioctl_binder_write_read(fd.as_raw_fd(), &raw mut rw) }
        .map_err(|x| (x, (rw.write_buffer_consumed, rw.read_buffer_filled_size)))?;

    Ok((rw.write_buffer_consumed, rw.read_buffer_filled_size))
}
