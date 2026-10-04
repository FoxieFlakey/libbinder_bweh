use std::{
    cell::RefCell,
    collections::HashMap,
    ffi::CString,
    io,
    os::fd::{AsFd, OwnedFd},
    panic,
    path::Path,
    sync::{
        Arc, Mutex, OnceLock, Weak,
        atomic::{AtomicU64, Ordering},
    },
    thread::{self, JoinHandle, ThreadId},
};

use anyhow::{Context, anyhow, bail};
use either::Either;
use enumflags2::BitFlags;
use libbinder_sys::{
    commands::Command,
    transaction::{
        Transaction, TransactionDataCommon, TransactionKernelManaged, TransactionNotKernelMananged,
        TransactionNotKernelManangedSg,
    },
    types::reference::{ObjectRef, ObjectRefLocal, ObjectRefRemote},
    write_read::binder_read_write,
};
use nix::{
    fcntl::{OFlag, open},
    poll::{PollFd, PollFlags, PollTimeout, poll},
    sys::stat::Mode,
    unistd::{Pid, Uid, geteuid, getpid},
};
use sharded_slab::Slab;
use thread_local::ThreadLocal;

use crate::{
    mmap::Mmap,
    object::{B, CallerIdentity, ObjectTrait, TransactionError},
    packet::Packet,
    pipe::Pipe,
    proxy::Proxy,
    return_parser::{RetIterator, RetVal},
};

mod mmap;
pub mod object;
pub mod packet;
mod pipe;
pub mod proxy;
mod return_parser;

pub struct Runtime {
    id: u64,
    binder_dev: Arc<OwnedFd>,
    shutdown_pipe: Arc<Pipe<bool>>,
    threads: Mutex<HashMap<ThreadId, JoinHandle<()>>>,
    manager: OnceLock<Arc<B<dyn ObjectTrait>>>,
    local_objects: Slab<Arc<B<dyn ObjectTrait>>>,
    identity_stack: ThreadLocal<RefCell<Vec<CallerIdentity>>>,
    death_callbacks: Slab<Mutex<Option<Box<dyn FnOnce() + Send>>>>,
    freeze_callbacks: Slab<Mutex<Option<Box<dyn FnOnce(bool) + Send>>>>,
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

pub struct DeathNotificationToken {
    runtime_id: u64,
    index: usize,
}
pub struct FreezeNotificationToken {
    runtime_id: u64,
    index: usize,
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
        static ID: AtomicU64 = AtomicU64::new(0);
        let rt = Arc::new(Self {
            id: ID.fetch_add(1, Ordering::Relaxed),
            _mmap: mmap,
            binder_dev: dev.clone(),
            shutdown_pipe: shutdown_pipe.clone(),
            manager: OnceLock::new(),
            threads: Mutex::new(HashMap::new()),
            local_objects: Slab::new(),
            identity_stack: ThreadLocal::new(),
            death_callbacks: Slab::new(),
            freeze_callbacks: Slab::new(),
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
                builder(Proxy {
                    rt: Arc::downgrade(&rt),
                    reference: Either::Right(SERVICE_MANAGER),
                })?
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
        write_buf: &[u8],
        read_buf: &mut [u8],
    ) -> anyhow::Result<usize> {
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

    pub fn get_manager(&self) -> &Arc<B<dyn ObjectTrait>> {
        self.manager.get().expect("Manager is not initialized")
    }

    pub(crate) fn send_packet(
        self: &Arc<Runtime>,
        code: u32,
        flags: BitFlags<object::Flag>,
        packet: &mut Packet,
        target: ObjectRefRemote,
    ) -> Result<Option<(u32, Packet)>, TransactionError> {
        let flags_out = object::Flag::into_raw(flags);
        let is_one_way = flags.contains(object::Flag::OneWay);

        let transaction = Transaction::NotKernelManagedSg(TransactionNotKernelManangedSg {
            data: TransactionNotKernelMananged {
                data: TransactionDataCommon {
                    code,
                    data_slice: &packet.get_data(),
                    flags: flags_out,
                    offsets: &packet.get_offsets(),
                    target: ObjectRef::Remote(target),
                    secctx: None,
                    sender_euid: 0,
                    sender_pid: 0,
                },
            },
            buffers_size: packet.get_buffers_size(),
        });

        let mut write_buf = Vec::new();
        write_buf.extend_from_slice(&Command::SendTransactionSG.as_bytes());
        transaction.with_bytes(|x| write_buf.extend_from_slice(x));
        self.do_read_write(&write_buf, &mut [])
            .expect("Cannot perform BINDER_WRITE_READ to send transaction");
        unsafe { packet.objects_sent() };
        drop(write_buf);

        let mut ret_buf = [0; READ_BUF_SIZE];
        let mut reply = None;
        let mut is_completed = false;

        loop {
            let bytes_read = self
                .do_read_write(&[], &mut ret_buf)
                .expect("Cannot perform BINDER_WRITE_READ to wait for result");
            let read = &ret_buf[0..bytes_read];

            // SAFETY: Kernel jsut wrote it
            let mut err = None;
            self.handle_ret_values(unsafe { RetIterator::new(&read) }, |ret| match ret {
                return_parser::RetVal::Err(e) => {
                    err = Some(TransactionError::KernelError(io::Error::from_raw_os_error(
                        e,
                    )));
                }
                return_parser::RetVal::FailedTransaction => {
                    err = Some(TransactionError::KernelCantSend);
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
                        Transaction::NotKernelManagedSg(_) | Transaction::NotKernelManaged(_) => {
                            unreachable!("This has to be from kernel")
                        }
                    };

                    assert!(
                        reply.is_none(),
                        "Kernely sent double BR_REPLY when it should not"
                    );

                    reply = Some((code, Packet::from_kernel(self.clone(), kernel)));
                }
                return_parser::RetVal::DeadReply => err = Some(TransactionError::TargetDied),
                return_parser::RetVal::FrozenTarget => err = Some(TransactionError::TargetFrozen),
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
            // If we not set oneway flag, kernel CANNOT return
            // succesful transaction IF it doesn't provide BR_REPLY
            panic!("Unexpected missing reply for non one way transaction")
        } else if is_one_way && reply.is_some() {
            // If we set oneway flag, kernel CANNOT return BR_REPLY
            panic!("Received unexpected reply for one way transactions")
        }

        Ok(reply)
    }

    fn push_identity(&self, identity: CallerIdentity) {
        self.identity_stack
            .get_or_default()
            .borrow_mut()
            .push(identity);
    }

    fn pop_identity(&self) {
        self.identity_stack
            .get_or_default()
            .borrow_mut()
            .pop()
            .expect("Unbalanced transaction stack?");
    }

    pub fn get_caller_identity(&self) -> CallerIdentity {
        self.identity_stack
            .get_or_default()
            .borrow_mut()
            .last()
            .map(|x| x.clone())
            .unwrap_or_else(|| CallerIdentity {
                sender_euid: geteuid(),
                sender_pid: getpid(),
                sender_security_ctx: None,
            })
    }

    // NOTE: This is no-op on local object, so it returns None
    // else return Some(token). Token can be used to unregister
    pub fn attach_death_callback<T: ObjectTrait + ?Sized, F: FnOnce() + Send + 'static>(
        &self,
        remote: &Arc<B<T>>,
        callback: F,
    ) -> Option<DeathNotificationToken> {
        remote.get_remote().map(|x| {
            let mut remote = x
                .reference
                .clone()
                .right()
                .expect("get_remote returns local proxy when it must not");
            remote.extra_local_data = self
                .death_callbacks
                .insert(Mutex::new(Some(Box::new(callback))))
                .unwrap();

            let mut buf = Vec::new();
            buf.extend_from_slice(&Command::RequestDeathNotification.as_bytes());
            buf.extend_from_slice(&remote.data_handle.to_ne_bytes());
            buf.extend_from_slice(&remote.extra_local_data.to_ne_bytes());
            self.do_read_write(&buf, &mut [])
                .expect("Cannot request death notification");

            DeathNotificationToken {
                runtime_id: self.id,
                index: remote.extra_local_data,
            }
        })
    }

    pub fn detach_death_callback(
        &self,
        token: DeathNotificationToken,
    ) -> Box<dyn FnOnce() + Send + 'static> {
        assert!(
            token.runtime_id == self.id,
            "attempting to detach death callback belonging to other runtime"
        );

        self.death_callbacks
            .take(token.index)
            .unwrap()
            .get_mut()
            .unwrap()
            .take()
            .unwrap()
    }

    // NOTE: This is no-op on local object, so it returns None
    // else return Some(token). Token can be used to unregister
    pub fn attach_freeze_callback<T: ObjectTrait + ?Sized, F: FnOnce(bool) + Send + 'static>(
        &self,
        remote: &Arc<B<T>>,
        callback: F,
    ) -> Option<FreezeNotificationToken> {
        remote.get_remote().map(|x| {
            let mut remote = x
                .reference
                .clone()
                .right()
                .expect("get_remote returns local proxy when it must not");
            remote.extra_local_data = self
                .freeze_callbacks
                .insert(Mutex::new(Some(Box::new(callback))))
                .unwrap();

            let mut buf = Vec::new();
            buf.extend_from_slice(&Command::RequestDeathNotification.as_bytes());
            buf.extend_from_slice(&remote.data_handle.to_ne_bytes());
            buf.extend_from_slice(&remote.extra_local_data.to_ne_bytes());
            self.do_read_write(&buf, &mut [])
                .expect("Cannot request freeze notification");

            FreezeNotificationToken {
                runtime_id: self.id,
                index: remote.extra_local_data,
            }
        })
    }

    pub fn detach_freeze_callback(
        &self,
        token: FreezeNotificationToken,
    ) -> Box<dyn FnOnce(bool) + Send + 'static> {
        assert!(
            token.runtime_id == self.id,
            "attempting to detach freeze callback belonging to other runtime"
        );

        self.freeze_callbacks
            .take(token.index)
            .unwrap()
            .get_mut()
            .unwrap()
            .take()
            .unwrap()
    }

    // handle transaction that comes
    fn handle_transaction(self: &Arc<Runtime>, transaction: TransactionKernelManaged) {
        let target = match transaction.get_data().target {
            ObjectRef::Local(x) => x.data,
            ObjectRef::Remote(_) => {
                panic!("Should not receive reference to remote object")
            }
        };

        self.push_identity(object::CallerIdentity {
            sender_euid: Uid::from_raw(transaction.get_data().sender_euid),
            sender_pid: Pid::from_raw(transaction.get_data().sender_pid),
            sender_security_ctx: transaction.get_data().secctx.map(CString::from),
        });

        let code = transaction.get_data().code;
        let flags = object::Flag::from_raw(transaction.get_data().flags);
        let mut packet = Packet::from_kernel(self.clone(), transaction);
        let meta = self.local_objects.get(target).unwrap();
        let control = meta.control.read().unwrap();
        if !control.has_strong && !control.has_weak {
            panic!("Attempting to handle transaction on object that was already removed")
        }

        let ret = meta
            .on_transaction(code, flags, &mut packet)
            .expect("Local on_transaction cannot return Err");
        drop(packet);

        self.pop_identity();

        if flags.contains(object::Flag::OneWay) {
            // no need to handle replying
            return;
        }

        let Some((reply_code, reply)) = ret else {
            panic!("This is non oneway transaction but reply is not provided");
        };

        let mut write_buf = Vec::new();
        write_buf.extend_from_slice(&Command::SendReplySG.as_bytes());
        let transaction = Transaction::NotKernelManagedSg(TransactionNotKernelManangedSg {
            data: TransactionNotKernelMananged {
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
            },
            buffers_size: reply.get_buffers_size(),
        });
        transaction.with_bytes(|x| write_buf.extend_from_slice(x));
        self.do_read_write(&write_buf, &mut [])
            .expect("Cannot send reply");
    }

    fn inc_remote_ref(&self, remote: &ObjectRefRemote) {
        let mut buf = Vec::new();
        buf.extend_from_slice(&Command::Acquire.as_bytes());
        buf.extend_from_slice(&remote.data_handle.to_ne_bytes());
        self.do_read_write(&buf, &mut [])
            .expect("Cannot send BC_ACQUIRE for remote reference");
    }

    fn dec_remote_ref(&self, remote: &ObjectRefRemote) {
        let mut buf = Vec::new();
        buf.extend_from_slice(&Command::Release.as_bytes());
        buf.extend_from_slice(&remote.data_handle.to_ne_bytes());
        self.do_read_write(&buf, &mut [])
            .expect("Cannot send BC_RELEASE for remote reference");
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
                return_parser::RetVal::ClearDeathNotificationDone(cookie) => {
                    let _ = self
                        .death_callbacks
                        .take(cookie)
                        .expect("Unknown death callback");
                }
                return_parser::RetVal::ClearFreezeNotificationDone(cookie) => {
                    let _ = self
                        .freeze_callbacks
                        .take(cookie)
                        .expect("Unknown freeze callback");
                }
                return_parser::RetVal::DeadBinder(cookie) => {
                    self.death_callbacks
                        .take(cookie)
                        .expect("Unknown death callback")
                        .get_mut()
                        .unwrap()
                        .take()
                        .unwrap()();

                    let mut buf = Vec::new();
                    buf.extend_from_slice(&Command::DeathNotificationDone.as_bytes());
                    buf.extend_from_slice(&cookie.to_ne_bytes());
                    self.do_read_write(&buf, &mut [])
                        .expect("Cannot send death notification done");
                }
                return_parser::RetVal::FrozenBinder {
                    is_now_frozen,
                    cookie,
                } => {
                    self.freeze_callbacks
                        .take(cookie)
                        .expect("Unknown frozen callback")
                        .get_mut()
                        .unwrap()
                        .take()
                        .unwrap()(is_now_frozen);

                    let mut buf = Vec::new();
                    buf.extend_from_slice(&Command::FreezeNotificationDone.as_bytes());
                    buf.extend_from_slice(&cookie.to_ne_bytes());
                    self.do_read_write(&buf, &mut [])
                        .expect("Cannot send freeze notification done");
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
