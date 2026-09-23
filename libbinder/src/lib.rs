use std::{
    os::fd::{AsFd, OwnedFd},
    path::Path,
    sync::{Arc, OnceLock, Weak},
    thread::{self, JoinHandle},
};

use anyhow::{Context, anyhow, bail};
use enumflags2::BitFlags;
use libbinder_sys::{
    commands::Command,
    transaction::{
        Transaction, TransactionDataCommon, TransactionFlag, TransactionNotKernelMananged,
    },
    types::reference::{ObjectRef, ObjectRefLocal, ObjectRefRemote},
    write_read::binder_read_write,
};
use nix::{
    errno::Errno,
    fcntl::{OFlag, open},
    poll::{PollFd, PollFlags, PollTimeout, poll},
    sys::stat::Mode,
};

use crate::{
    mmap::Mmap, object::Object, packet::Packet, pipe::Pipe, proxy::Proxy,
    return_parser::RetIterator,
};

mod mmap;
pub mod object;
pub mod packet;
mod pipe;
mod proxy;
mod return_parser;
mod test;

pub use test::lib_main;

pub struct Runtime {
    binder_dev: Arc<OwnedFd>,
    shutdown_pipe: Arc<Pipe<bool>>,
    join_handle: JoinHandle<()>,
    manager: OnceLock<Arc<dyn Object>>,
    _mmap: Mmap,
}

impl Drop for Runtime {
    fn drop(&mut self) {
        self.shutdown_pipe.write_blocking(true).unwrap();
        if self.join_handle.thread().id() == thread::current().id() {
            return;
        }
    }
}

pub const READ_BUF_SIZE: usize = 256;
const SERVICE_MANAGER: ObjectRefRemote = ObjectRefRemote {
    data_handle: 0,
    extra_local_data: 0,
};
pub const BINDER_BUFFER_SIZE: usize = 4 * 1024 * 1024;

pub enum ContextManagerInfo {
    Concrete(Arc<dyn Object>),
    Remote(Box<dyn FnOnce(Proxy) -> anyhow::Result<Arc<dyn Object>>>),
}

impl Runtime {
    pub fn new<P>(path: P, context_manager: ContextManagerInfo) -> anyhow::Result<Arc<Runtime>>
    where
        P: AsRef<Path>,
    {
        let dev = Arc::new(
            open(
                path.as_ref(),
                OFlag::O_RDWR | OFlag::O_CLOEXEC | OFlag::O_NONBLOCK,
                Mode::all(),
            )
            .context("Opening binder dev")?,
        );

        let shutdown_pipe = Arc::new(Pipe::new().context("Creating shutdown pipe")?);
        let mmap = Mmap::new(dev.as_fd(), BINDER_BUFFER_SIZE)
            .context("Trying to map buffer for binder")?;
        let rt = Arc::new_cyclic(|weak| {
            let weak = weak.clone();
            let dev = dev.clone();
            Self {
                _mmap: mmap,
                binder_dev: dev.clone(),
                shutdown_pipe: shutdown_pipe.clone(),
                manager: OnceLock::new(),
                join_handle: thread::spawn(move || worker(dev, shutdown_pipe, weak.clone())),
            }
        });

        rt.manager.set(match context_manager {
            ContextManagerInfo::Concrete(manager) => {
                // We become client, here would be do some calls to binder
                // to say we're the manager and also remember the concrete
                libbinder_sys::binder_set_context_mgr(
                    dev.as_fd(),
                    &ObjectRefLocal {
                        data: 0,
                        extra_data: 0,
                    },
                )
                .context("Cannot become context manager")?;
                manager
            }
            ContextManagerInfo::Remote(builder) => {
                // We become client, here would be creating local
                // service manager proxy
                builder(Proxy {
                    rt: Arc::downgrade(&rt),
                    remote_ref: SERVICE_MANAGER,
                })?
            }
        });
        Ok(rt)
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

    pub fn get_manager(&self) -> &Arc<dyn Object> {
        self.manager.get().expect("Manager is not initialized")
    }

    pub(crate) fn send_packet(
        &self,
        code: u32,
        flags: BitFlags<object::Flag>,
        packet: &Packet,
        target: ObjectRef,
    ) -> anyhow::Result<Option<(u32, BitFlags<TransactionFlag>, Packet)>> {
        let mut flags_out = BitFlags::default();
        let is_one_way = flags.contains(object::Flag::OneWay);
        if is_one_way {
            flags_out |= TransactionFlag::OneWay;
        }

        let transaction = Transaction::NotKernelManaged(TransactionNotKernelMananged {
            data: TransactionDataCommon {
                code,
                data_slice: &packet.get_data(),
                flags: flags_out,
                offsets: &packet.get_offsets(),
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

        let mut reply = None;
        // SAFETY: Kernel jsut wrote it
        for ret in unsafe { RetIterator::new(&read) } {
            match ret {
                return_parser::RetVal::Err(e) => {
                    bail!("Error sending packet (kernel returned BR_ERROR): {e}")
                }
                return_parser::RetVal::Ok => panic!("Not expecting BR_OK"),
                return_parser::RetVal::TransactionComplete => (),
                return_parser::RetVal::Transaction(transaction) => {
                    todo!("Handle nested transaction")
                }
                return_parser::RetVal::Reply(transaction) => {
                    assert!(!is_one_way, "Kernel sent reply for one way??");
                    let code = transaction.get_common().code;
                    let flags = transaction.get_common().flags;
                    let kernel = match transaction {
                        Transaction::KernelManaged(x) => x,
                        Transaction::NotKernelManaged(_) => {
                            unreachable!("This has to be from kernel")
                        }
                    };

                    if reply.is_some() {
                        bail!("Kernel sent double reply??")
                    }

                    reply = Some((
                        code,
                        flags,
                        Packet::from_kernel(self.binder_dev.clone(), kernel),
                    ));
                }
                return_parser::RetVal::DeadBinder(_) => (),
                return_parser::RetVal::DeadReply => bail!("Target died"),
                return_parser::RetVal::SpawnLooper => (),
            }
        }

        if !is_one_way && reply.is_none() {
            bail!("Remote didnt send reply")
        }

        Ok(reply)
    }

    // handle transaction that comes
    fn handle_transaction(&self) {
        let mut read_buf = [0; READ_BUF_SIZE];
        let read_bytes = self
            .do_read_write(&[], &mut read_buf)
            .expect("Cannot read incoming transactions");
        for ret in unsafe { RetIterator::new(&read_buf[..read_bytes]) } {
            match ret {
                return_parser::RetVal::TransactionComplete
                | return_parser::RetVal::DeadReply
                | return_parser::RetVal::Ok
                | return_parser::RetVal::Reply(_)
                | return_parser::RetVal::Err(_) => panic!("Unexpected"),
                return_parser::RetVal::Transaction(transaction) => {
                    todo!("handle transaction")
                }
                return_parser::RetVal::DeadBinder(_) => (),
                return_parser::RetVal::SpawnLooper => (),
            }
        }
    }
}

fn worker(dev: Arc<OwnedFd>, shutdown_pipe: Arc<Pipe<bool>>, runtime: Weak<Runtime>) {
    loop {
        let mut pollfd = [
            PollFd::new(dev.as_fd(), PollFlags::POLLIN),
            PollFd::new(shutdown_pipe.get_read_fd(), PollFlags::POLLIN),
        ];

        poll(&mut pollfd, PollTimeout::NONE).unwrap();

        if !pollfd[1].revents().unwrap().is_empty() {
            break;
        }

        if !pollfd[0].revents().unwrap().is_empty() {
            if let Some(x) = runtime.upgrade() {
                x.handle_transaction();
            } else {
                // This might indicate runtime has shutdown BUT there race that
                // this might be reached BEFORE the runtime init completed
                // so this is no-op
            }
        }
    }

    println!("Shutdown triggered, quiting...");
}
