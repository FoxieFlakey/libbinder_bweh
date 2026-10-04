use std::io;
use std::os::fd::AsFd;
use std::sync::{Arc, Weak};

use either::Either;
use libbinder_sys::types::reference::ObjectRefRemote;

use crate::{
    Runtime,
    object::{B, ObjectTrait},
};

pub struct Proxy {
    pub(crate) rt: Weak<Runtime>,
    pub(crate) reference: Either<Arc<B<dyn ObjectTrait>>, ObjectRefRemote>,
}

impl Clone for Proxy {
    fn clone(&self) -> Self {
        if let Either::Right(remote) = &self.reference {
            // We need to notify kernel that the local reference is cloned
            self.rt.upgrade().unwrap().inc_remote_ref(remote);
        }

        Self {
            rt: self.rt.clone(),
            reference: self.reference.clone(),
        }
    }
}

impl PartialEq for Proxy {
    fn eq(&self, other: &Self) -> bool {
        match (&self.reference, &other.reference) {
            (Either::Left(a), Either::Left(b)) => Arc::ptr_eq(a, b),
            (Either::Left(_), Either::Right(_)) | (Either::Right(_), Either::Left(_)) => false,
            (Either::Right(a), Either::Right(b)) => a == b,
        }
    }
}

impl Drop for Proxy {
    fn drop(&mut self) {
        let Some(rt) = self.rt.upgrade() else {
            return;
        };

        if let Either::Right(x) = &self.reference {
            rt.dec_remote_ref(x);
        }
    }
}

impl Proxy {
    pub fn from_object(local: Arc<B<dyn ObjectTrait>>) -> Self {
        match local.get_remote() {
            Some(proxy) => Self {
                rt: Arc::downgrade(&local.get_runtime()),
                reference: Either::Right(
                    *proxy
                        .reference
                        .as_ref()
                        .right()
                        .inspect(|remote_ref| {
                            // We need to notify kernel that the local reference is cloned
                            proxy.rt.upgrade().unwrap().inc_remote_ref(*remote_ref);
                        })
                        .expect(".get_remote returns non remote reference!"),
                ),
            },
            None => Self {
                rt: Arc::downgrade(&local.get_runtime()),
                reference: Either::Left(local),
            },
        }
    }

    // NOTE: This will only work if caller is context manager
    // Returns None, if this proxy is not remote reference
    pub fn get_remote_strong_count(&self) -> Result<Option<usize>, io::Error> {
        match self.reference.as_ref() {
            Either::Left(_) => Ok(None),
            Either::Right(remote) => {
                let info = libbinder_sys::binder_get_remote_node_info(
                    self.rt.upgrade().unwrap().binder_dev.as_fd(),
                    remote,
                )?;

                Ok(Some(info.strong_count.try_into().unwrap()))
            }
        }
    }

    // NOTE: This will only work if caller is context manager
    // Returns None, if this proxy is not remote reference
    pub fn get_remote_weak_count(&self) -> Result<Option<usize>, io::Error> {
        match self.reference.as_ref() {
            Either::Left(_) => Ok(None),
            Either::Right(remote) => {
                let info = libbinder_sys::binder_get_remote_node_info(
                    self.rt.upgrade().unwrap().binder_dev.as_fd(),
                    remote,
                )?;

                Ok(Some(info.strong_count.try_into().unwrap()))
            }
        }
    }
}

impl ObjectTrait for Proxy {
    fn get_remote<'a>(&'a self) -> Option<&'a Proxy> {
        if self.reference.is_right() {
            Some(self)
        } else {
            None
        }
    }

    fn get_runtime(&self) -> Arc<Runtime> {
        self.rt.upgrade().unwrap()
    }

    fn on_transaction(
        &self,
        code: u32,
        flags: enumflags2::BitFlags<crate::object::Flag>,
        message: &mut crate::packet::Packet,
    ) -> Result<Option<(u32, crate::packet::Packet)>, crate::object::TransactionError> {
        match &self.reference {
            Either::Left(local) => local.on_transaction(code, flags, message),
            Either::Right(x) => {
                let rt: Arc<Runtime> = self
                    .rt
                    .upgrade()
                    .expect("Runtime is not alive anymore for Binder proxy");
                rt.send_packet(code, flags, message, *x)
            }
        }
    }
}
