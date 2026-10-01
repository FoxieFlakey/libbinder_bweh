use std::{
    collections::HashMap,
    ffi::CString,
    os::fd::{AsFd, OwnedFd},
    panic,
    path::Path,
    sync::{Arc, Mutex, OnceLock, Weak},
    thread::{self, JoinHandle, ThreadId},
};

use anyhow::{Context, anyhow, bail};
use enumflags2::BitFlags;
use libbinder_sys::{
    commands::Command,
    transaction::{
        Transaction, TransactionDataCommon, TransactionKernelManaged, TransactionNotKernelMananged,
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
use sharded_slab::Slab;

use crate::{
    mmap::Mmap,
    object::{B, ObjectTrait},
    packet::Packet,
    pipe::Pipe,
    proxy::Proxy,
    return_parser::{RetIterator, RetVal},
};

mod mmap;
pub mod object;
pub mod packet;
mod pipe;
mod proxy;
mod return_parser;

pub struct Runtime {
    binder_dev: Arc<OwnedFd>,
    shutdown_pipe: Arc<Pipe<bool>>,
    threads: Mutex<HashMap<ThreadId, JoinHandle<()>>>,
    manager: OnceLock<Arc<B<dyn ObjectTrait>>>,
    local_objects: Slab<Arc<B<dyn ObjectTrait>>>,
    _mmap: Mmap,
}

impl Drop for Runtime {
    fn drop(&mut self) {
        self.shutdown_pipe.write_blocking(true).unwrap();
        for (_, join_handle) in self.threads.get_mut().unwrap().drain() {
            if join_handle.thread().id() == thread::current().id() {
                // dont wait on ourself
                continue;
            }

            join_handle.join().unwrap();
        }
    }
}

const READ_BUF_SIZE: usize = 256;
const SERVICE_MANAGER: ObjectRefRemote = ObjectRefRemote {
    data_handle: 0,
    extra_local_data: 0,
};
const BINDER_BUFFER_SIZE: usize = 4 * 1024 * 1024;

pub enum ContextManagerInfo {
    Concrete(Box<dyn FnOnce(&Arc<Runtime>) -> anyhow::Result<Arc<B<dyn ObjectTrait>>>>),
    Remote(Box<dyn FnOnce(Proxy) -> anyhow::Result<Arc<B<dyn ObjectTrait>>>>),
}

impl Runtime {
    pub fn new<P>(path: P, context_manager: ContextManagerInfo) -> anyhow::Result<Arc<Runtime>>
    where
        P: AsRef<Path>,
    {
        let dev = Arc::new(
            open(path.as_ref(), OFlag::O_RDWR | OFlag::O_CLOEXEC, Mode::all())
                .context("Opening binder dev")?,
        );

        let shutdown_pipe = Arc::new(Pipe::new().context("Creating shutdown pipe")?);
        let mmap = Mmap::new(dev.as_fd(), BINDER_BUFFER_SIZE)
            .context("Trying to map buffer for binder")?;
        let rt = Arc::new(Self {
            _mmap: mmap,
            binder_dev: dev.clone(),
            shutdown_pipe: shutdown_pipe.clone(),
            manager: OnceLock::new(),
            threads: Mutex::new(HashMap::new()),
            local_objects: Slab::new(),
        });

        // Context manager may want to perform calls to remote too
        // and remote may calls back
        rt.spawn_looper(false);

        let mgr = match context_manager {
            ContextManagerInfo::Concrete(manager) => {
                let manager = manager(&rt)?;
                let id = rt.add_object(manager.clone());
                manager.control.write().unwrap().has_strong = true;

                // We become client, here would be do some calls to binder
                // to say we're the manager and also remember the concrete
                libbinder_sys::binder_set_context_mgr(
                    dev.as_fd(),
                    &ObjectRefLocal {
                        data: id,
                        extra_data: 0,
                    },
                    manager.flags.into_flags(),
                )
                .context("Cannot become context manager")?;
                manager
            }
            ContextManagerInfo::Remote(builder) => {
                // We become client, here would be creating local
                // service manager proxy
                let mgr = builder(Proxy {
                    rt: Arc::downgrade(&rt),
                    remote_ref: SERVICE_MANAGER,
                })?;
                rt.add_object(mgr.clone());
                mgr
            }
        };
        rt.manager.set(mgr).ok().unwrap();

        Ok(rt)
    }

    fn add_object(self: &Arc<Runtime>, object: Arc<B<dyn ObjectTrait>>) -> usize {
        let mut control = object.control.write().unwrap();
        let id;
        if let Some((idx, weak_rt)) = &control.live_slot {
            if !Weak::ptr_eq(&Arc::downgrade(self), weak_rt) {
                panic!("Attempting to use object belonging to other runtime!");
            }
            id = *idx;
            drop(control);
        } else {
            let entry = self.local_objects.vacant_entry().unwrap();
            id = entry.key();
            control.live_slot = Some((id, Arc::downgrade(self)));
            drop(control);
            entry.insert(object);
        }

        id
    }

    fn spawn_looper(self: &Arc<Runtime>, is_spawned_by_kernel: bool) {
        let dev = self.binder_dev.clone();
        let shutdown_pipe = self.shutdown_pipe.clone();
        let rt = self.clone();
        let handle = thread::spawn(move || worker(dev, shutdown_pipe, rt, is_spawned_by_kernel));
        assert!(
            self.threads
                .lock()
                .unwrap()
                .insert(handle.thread().id(), handle)
                .is_none(),
            "Must not exist already"
        );
    }

    pub(crate) fn do_read_write(
        &self,
        mut write_buf: &[u8],
        mut read_buf: &mut [u8],
    ) -> anyhow::Result<usize> {
        if true {
            match binder_read_write(self.binder_dev.as_fd(), &write_buf, read_buf) {
                Ok((write_size, read_count)) => {
                    assert!(
                        write_size == write_buf.len(),
                        "kernel didnt process everything"
                    );
                    return Ok(read_count);
                }
                Err((e, ..)) => return Err(anyhow!("Cannot do BINDER_WRITE_READ: {e}")),
            }
        }

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

    pub fn get_manager(&self) -> &Arc<B<dyn ObjectTrait>> {
        self.manager.get().expect("Manager is not initialized")
    }

    pub(crate) fn send_packet(
        self: &Arc<Runtime>,
        code: u32,
        flags: BitFlags<object::Flag>,
        packet: &mut Packet,
        target: ObjectRef,
    ) -> anyhow::Result<Option<(u32, Packet)>> {
        let flags_out = object::Flag::into_raw(flags);
        let is_one_way = flags.contains(object::Flag::OneWay);

        let transaction = Transaction::NotKernelManaged(TransactionNotKernelMananged {
            data: TransactionDataCommon {
                code,
                data_slice: &packet.get_data(),
                flags: flags_out,
                offsets: &packet.get_offsets(),
                target,
                secctx: None,
                sender_euid: 0,
                sender_pid: 0,
            },
        });

        let mut write_buf = Vec::new();
        write_buf.extend_from_slice(&Command::SendTransaction.as_bytes());
        transaction.with_bytes(|x| write_buf.extend_from_slice(x));
        self.do_read_write(&write_buf, &mut [])
            .context("Cannot send packet")?;

        let mut ret_buf = [0; READ_BUF_SIZE];
        let mut reply = None;
        let mut is_completed = false;
        let mut is_first_time = true;

        loop {
            let bytes_read = self
                .do_read_write(&[], &mut ret_buf)
                .context("Cannot wait for reply/transaction complete")?;
            let read = &ret_buf[0..bytes_read];

            if is_first_time {
                // SAFETY: The packet did succesfully sent out
                unsafe { packet.objects_sent() };
                write_buf.clear();
                is_first_time = false;
            }

            // SAFETY: Kernel jsut wrote it
            let mut err = None;
            self.handle_ret_values(unsafe { RetIterator::new(&read) }, |ret| match ret {
                return_parser::RetVal::Err(e) => {
                    err = Some(anyhow::anyhow!(
                        "Error sending packet (kernel returned BR_ERROR): {e}"
                    ));
                }
                return_parser::RetVal::FailedTransaction => {
                    err = Some(anyhow::anyhow!(
                        "Kernel cannot send transaction for some reason, See dmesg"
                    ));
                }
                return_parser::RetVal::Ok => panic!("Not expecting BR_OK"),
                return_parser::RetVal::TransactionComplete => {
                    is_completed = true;
                }
                return_parser::RetVal::Transaction(Transaction::KernelManaged(transaction)) => {
                    self.handle_transaction(transaction);
                }
                return_parser::RetVal::Transaction(Transaction::NotKernelManaged(_)) => {
                    unreachable!("Kernel should not return not kernel managed transactions")
                }
                return_parser::RetVal::Reply(transaction) => {
                    assert!(!is_one_way, "Kernel sent reply for one way??");
                    let code = transaction.get_common().code;
                    let kernel = match transaction {
                        Transaction::KernelManaged(x) => x,
                        Transaction::NotKernelManaged(_) => {
                            unreachable!("This has to be from kernel")
                        }
                    };

                    if reply.is_some() {
                        err = Some(anyhow!("Kernel sent double reply??"));
                    }

                    reply = Some((code, Packet::from_kernel(self.clone(), kernel)));
                }
                return_parser::RetVal::DeadReply => err = Some(anyhow!("Target died")),
                _ => unreachable!(),
            });

            if let Some(err) = err {
                return Err(err);
            }

            if is_completed && reply.is_some() {
                // Received both the BR_TRANSACTION_COMPLETE and the BR_REPLY
                break;
            } else if is_completed && is_one_way {
                // Only BR_TRANSACTION_COMPLETE is sent for one way transaction
                break;
            }
        }

        if !is_one_way && reply.is_none() {
            bail!("Remote didnt send reply")
        } else if is_one_way && reply.is_some() {
            bail!("Remote sent reply for one way transactions")
        }

        Ok(reply)
    }

    // handle transaction that comes
    fn handle_transaction(self: &Arc<Runtime>, transaction: TransactionKernelManaged) {
        let target = match transaction.get_data().target {
            ObjectRef::Local(x) => x.data,
            ObjectRef::Remote(_) => {
                panic!("Should not receive reference to remote object")
            }
        };

        let caller_identity = object::CallerIdentity {
            sender_euid: transaction.get_data().sender_euid,
            sender_pid: transaction.get_data().sender_pid,
            sender_security_ctx: transaction.get_data().secctx.map(CString::from),
        };
        let code = transaction.get_data().code;
        let flags = object::Flag::from_raw(transaction.get_data().flags);
        let mut packet = Packet::from_kernel(self.clone(), transaction);
        let meta = self.local_objects.get(target).unwrap();
        let control = meta.control.read().unwrap();
        if !control.has_strong && !control.has_weak {
            panic!("Attempting to handle transaction on object that was already removed")
        }

        let (reply_code, reply) = meta
            .on_transaction(code, flags, &mut packet, Some(caller_identity))
            .expect("Cannot perform transaction");
        drop(packet);

        if flags.contains(object::Flag::OneWay) {
            // no need to handle replying
            return;
        }

        let mut write_buf = Vec::new();
        write_buf.extend_from_slice(&Command::SendReply.as_bytes());
        let transaction = Transaction::NotKernelManaged(TransactionNotKernelMananged {
            data: TransactionDataCommon {
                code: reply_code,
                data_slice: &reply.get_data(),
                flags: BitFlags::default(),
                offsets: &reply.get_offsets(),
                target: ObjectRef::Local(ObjectRefLocal {
                    data: 0,
                    extra_data: 0,
                }),
                secctx: None,
                sender_euid: 0,
                sender_pid: 0,
            },
        });
        transaction.with_bytes(|x| write_buf.extend_from_slice(x));
        self.do_read_write(&write_buf, &mut [])
            .expect("Cannot send reply");
    }

    fn handle_ret_values<F>(self: &Arc<Runtime>, ret_iterator: RetIterator<'_>, mut handler: F)
    where
        F: FnMut(RetVal<'_>),
    {
        for ret in ret_iterator {
            match ret {
                return_parser::RetVal::Ok => (),
                return_parser::RetVal::Err(e) => panic!("Unexpected error: {e}"),
                return_parser::RetVal::Transaction(Transaction::NotKernelManaged(_)) => {
                    unreachable!("Kernel should not return non kernel managed transaction")
                }
                return_parser::RetVal::Transaction(Transaction::KernelManaged(transaction)) => {
                    self.handle_transaction(transaction);
                }
                return_parser::RetVal::DeadBinder(_) => (),
                return_parser::RetVal::SpawnLooper => {
                    self.spawn_looper(true);
                }
                return_parser::RetVal::AcquireStrong(ObjectRefLocal { data, .. }) => {
                    let meta = self
                        .local_objects
                        .get(data)
                        .expect("Cannot find local object");
                    meta.control.write().unwrap().has_strong = true;

                    let mut buf = Vec::new();
                    buf.extend_from_slice(&Command::AcquireDone.as_bytes());
                    buf.extend_from_slice(&data.to_ne_bytes());
                    buf.extend_from_slice(&(0usize).to_ne_bytes());
                    self.do_read_write(&buf, &mut [])
                        .expect("Cannot send BC_ACQUIRE_DONE");
                }
                return_parser::RetVal::ReleaseStrong(ObjectRefLocal { data, .. }) => {
                    let meta = self
                        .local_objects
                        .get(data)
                        .expect("Cannot find local object");
                    let mut control = meta.control.write().unwrap();
                    assert!(
                        control.has_strong,
                        "kernel sent inconsistent state for {data}"
                    );
                    control.has_strong = false;

                    if !control.has_weak {
                        drop(control);
                        drop(meta);
                        self.local_objects
                            .take(data)
                            .expect("Cannot remove local object");
                    }
                }
                return_parser::RetVal::AcquireWeak(ObjectRefLocal { data, .. }) => {
                    let meta = self
                        .local_objects
                        .get(data)
                        .expect("Cannot find local object");
                    meta.control.write().unwrap().has_weak = true;
                    let mut buf = Vec::new();
                    buf.extend_from_slice(&Command::AcquireWeakDone.as_bytes());
                    buf.extend_from_slice(&data.to_ne_bytes());
                    buf.extend_from_slice(&(0usize).to_ne_bytes());
                    self.do_read_write(&buf, &mut [])
                        .expect("Cannot send BC_INCREFS_DONE");
                }
                return_parser::RetVal::ReleaseWeak(ObjectRefLocal { data, .. }) => {
                    let meta = self
                        .local_objects
                        .get(data)
                        .expect("Cannot find local object");
                    let mut control = meta.control.write().unwrap();
                    assert!(
                        control.has_weak,
                        "kernel sent inconsistent state for {data}"
                    );
                    control.has_weak = false;

                    if !control.has_strong {
                        drop(control);
                        drop(meta);
                        self.local_objects
                            .take(data)
                            .expect("Cannot remove local object");
                    }
                }
                x => handler(x),
            }
        }
    }

    fn loop_once(self: &Arc<Runtime>) {
        let mut read_buf = [0; READ_BUF_SIZE];
        let read_bytes = self
            .do_read_write(&[], &mut read_buf)
            .expect("Cannot read incoming transactions");
        self.handle_ret_values(unsafe { RetIterator::new(&read_buf[..read_bytes]) }, |x| {
            if !matches!(x, RetVal::TransactionComplete) {
                unreachable!("Unexpected")
            }
        });
    }

    fn exit_looper(&self) {
        self.do_read_write(&Command::ExitLooper.as_bytes(), &mut [])
            .expect("Cannot exit as looper");
    }

    fn register_looper(&self) {
        self.do_read_write(&Command::RegisterLooper.as_bytes(), &mut [])
            .expect("Cannot register as looper");
    }

    fn enter_looper(&self) {
        self.do_read_write(&Command::EnterLooper.as_bytes(), &mut [])
            .expect("Cannot enter as looper");
    }

    fn worker_died(self: &Arc<Runtime>, id: ThreadId) {
        let mut threads = self.threads.lock().unwrap();
        threads
            .remove(&id)
            .expect("Current thread must already exists in threads list");

        // Thread has to be one thread exists. spawn it
        if threads.len() == 0 {
            drop(threads);
            self.spawn_looper(false);
        }
    }
}

fn worker(
    dev: Arc<OwnedFd>,
    shutdown_pipe: Arc<Pipe<bool>>,
    runtime_strong: Arc<Runtime>,
    is_spawned_by_kernel: bool,
) {
    if is_spawned_by_kernel {
        runtime_strong.register_looper();
    } else {
        runtime_strong.enter_looper();
    }
    let runtime = Arc::downgrade(&runtime_strong);
    drop(runtime_strong);

    let ret = panic::catch_unwind(panic::AssertUnwindSafe(|| {
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
                    x.loop_once();
                } else {
                    // This might indicate runtime has shutdown BUT there race that
                    // this might be reached BEFORE the runtime init completed
                    // so this is no-op
                }
            }
        }
    }));

    if let Err(e) = ret {
        if let Some(x) = runtime.upgrade() {
            x.exit_looper();
            x.worker_died(thread::current().id());
        }
        panic::resume_unwind(e);
    }

    if let Some(x) = runtime.upgrade() {
        x.exit_looper();
    }
}
