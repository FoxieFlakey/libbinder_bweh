use std::{
    os::fd::{AsFd, OwnedFd},
    path::Path,
    sync::Arc,
};

use anyhow::{Context, anyhow, bail};
use enumflags2::BitFlags;
use libbinder_sys::{
    commands::Command,
    transaction::{
        Transaction, TransactionDataCommon, TransactionFlag, TransactionNotKernelMananged,
    },
    types::reference::{ObjectRef, ObjectRefRemote},
    write_read::binder_read_write,
};
use nix::{
    errno::Errno,
    fcntl::{OFlag, open},
    poll::{PollFd, PollFlags, PollTimeout, poll},
    sys::stat::Mode,
};

use crate::{mmap::Mmap, packet::Packet, return_parser::RetIterator};

mod mmap;
pub mod object;
pub mod packet;
mod return_parser;

pub fn lib_main() {
    println!("Hello world!");

    let rt = Runtime::new("/dev/binder").unwrap();
    let packet = {
        let mut w = packet::Writer::new();
        w.write_u8(0x29);
        w.write_u64(0x38);
        w.finish()
    };

    rt.send_packet(
        0x2929,
        object::Flag::OneWay.into(),
        &packet,
        None,
        SERVICE_MANAGER,
    )
    .unwrap();
}

pub struct Runtime {
    binder_dev: OwnedFd,
    _mmap: Mmap,
}

pub const READ_BUF_SIZE: usize = 256;
pub const SERVICE_MANAGER: ObjectRef = ObjectRef::Remote(ObjectRefRemote {
    data_handle: 0,
    extra_local_data: 0,
});
pub const BINDER_BUFFER_SIZE: usize = 4 * 1024 * 1024;

impl Runtime {
    pub fn new<P>(path: P) -> anyhow::Result<Arc<Runtime>>
    where
        P: AsRef<Path>,
    {
        let dev = open(
            path.as_ref(),
            OFlag::O_RDWR | OFlag::O_CLOEXEC | OFlag::O_NONBLOCK,
            Mode::all(),
        )
        .context("Opening binder dev")?;
        let rt = Self {
            _mmap: Mmap::new(dev.as_fd(), BINDER_BUFFER_SIZE)
                .context("Trying to map buffer for binder")?,
            binder_dev: dev,
        };
        Ok(Arc::new(rt))
    }

    pub(crate) fn do_read_write(
        &self,
        mut write_buf: &[u8],
        mut read_buf: &mut [u8],
    ) -> anyhow::Result<usize> {
        let mut total_read_bytes = 0;
        loop {
            let mut pollfd = [PollFd::new(
                self.binder_dev.as_fd(),
                PollFlags::POLLIN | PollFlags::POLLOUT,
            )];

            poll(&mut pollfd, PollTimeout::NONE)
                .context("Cannot poll until binder device is ready")?;

            if !pollfd[0].revents().unwrap().is_empty() {
                match binder_read_write(self.binder_dev.as_fd(), &write_buf, read_buf) {
                    Ok((_, read_count)) => return Ok(total_read_bytes + read_count),
                    Err((Errno::EAGAIN, (write_bytes, read_bytes))) => {
                        write_buf = &write_buf[write_bytes..];
                        read_buf = &mut read_buf[read_bytes..];
                        total_read_bytes += read_bytes;
                    }
                    Err((e, ..)) => return Err(anyhow!("Cannot do BINDER_WRITE_READ: {e}")),
                }
            }
        }
    }

    pub(crate) fn send_packet(
        &self,
        code: u32,
        flags: BitFlags<object::Flag>,
        packet: &Packet,
        reply: Option<&mut Packet>,
        target: ObjectRef,
    ) -> anyhow::Result<()> {
        let mut flags_out = BitFlags::default();
        if flags.contains(object::Flag::OneWay) {
            flags_out |= TransactionFlag::OneWay;
        }

        let transaction = Transaction::NotKernelManaged(TransactionNotKernelMananged {
            data: TransactionDataCommon {
                code,
                data_slice: &packet.data,
                flags: flags_out,
                offsets: &packet.offsets,
                target,
            },
        });

        let mut write_buf = Vec::new();
        write_buf.extend_from_slice(&Command::SendTransaction.as_bytes());
        transaction.with_bytes(|x| write_buf.extend_from_slice(x));

        let mut ret_buf = [0; READ_BUF_SIZE];
        let bytes_read = self
            .do_read_write(&write_buf, &mut ret_buf)
            .context("Cannot send packet")?;
        let read = &ret_buf[0..bytes_read];

        // SAFETY: Kernel jsut wrote it
        for ret in unsafe { RetIterator::new(self.binder_dev.as_fd(), &read) } {
            match ret {
                return_parser::RetVal::Err(e) => {
                    bail!("Error sending packet (kernel returned BR_ERROR): {e}")
                }
                return_parser::RetVal::Ok => panic!("Not expecting BR_OK"),
                return_parser::RetVal::TransactionComplete => (),
                return_parser::RetVal::Transaction(transaction) => {
                    todo!("Handle nested transaction")
                }
                return_parser::RetVal::Reply(transaction) => todo!("Handle reply"),
                return_parser::RetVal::DeadBinder(_) => (),
                return_parser::RetVal::DeadReply => bail!("Target died"),
                return_parser::RetVal::SpawnLooper => (),
            }
        }
        Ok(())
    }
}
