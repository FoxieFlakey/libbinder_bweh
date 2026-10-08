use std::{
    mem,
    os::fd::{AsFd, OwnedFd},
    sync::Arc,
};

use anyhow::{Context, anyhow};
use either::Either;
use libbinder_sys::types::{
    ObjectParsed,
    buffer::Buffer,
    fd,
    reference::{ObjectRef, ObjectRefLocal},
};
use nix::fcntl::FdFlag;
use thiserror::Error;

use crate::{Runtime, packet::Packet, proxy::Proxy};

#[derive(Clone)]
pub struct Reader<'a> {
    runtime: &'a Arc<Runtime>,
    data: &'a [u8],
    offsets: &'a [usize],
    current_offset: usize,
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("Attempting to read binder object at wrong offset")]
    AttemptingToReadBinderObjectOnWrongOffset,
    #[error("Out of bound while reading")]
    OutOfBound,
    #[error("Invalid object type")]
    InvalidObjectType,
    #[error("Error while reading FD")]
    ErrorReadingFD(#[source] anyhow::Error),
}

impl<'a> Reader<'a> {
    pub fn new(packet: &'a Packet) -> Self {
        Self {
            runtime: &packet.runtime,
            current_offset: 0,
            data: packet.get_data(),
            offsets: packet.get_offsets(),
        }
    }

    pub fn read_bytes(&mut self, ret: &mut [u8]) -> Result<(), Error> {
        if self.data.len() < ret.len() {
            return Err(Error::OutOfBound);
        }

        ret.copy_from_slice(&self.data[self.current_offset..self.current_offset + ret.len()]);
        self.current_offset += ret.len();

        if let Some(&first) = self.offsets.first() {
            if self.current_offset > first {
                self.offsets = &self.offsets[1..];
            }
        }
        Ok(())
    }

    pub fn get_current_offset(&mut self) -> usize {
        self.current_offset
    }

    pub fn skip_bytes(&mut self, count: usize) -> Result<(), Error> {
        if self.current_offset + count > self.data.len() {
            return Err(Error::OutOfBound);
        }

        self.current_offset += count;

        if let Some(&first) = self.offsets.first() {
            if self.current_offset > first {
                self.offsets = &self.offsets[1..];
            }
        }
        Ok(())
    }

    pub fn get_rest_of_data(&self) -> &'a [u8] {
        &self.data[self.current_offset..]
    }

    pub fn read_buf(&mut self) -> Result<&'a [u8], Error> {
        if self.current_offset
            != *self
                .offsets
                .first()
                .ok_or(Error::AttemptingToReadBinderObjectOnWrongOffset)?
        {
            return Err(Error::AttemptingToReadBinderObjectOnWrongOffset);
        }

        // SAFETY: The data that came to packet is closely
        // controlled to be only contain valid object with valid
        // pointers
        let ret = match unsafe { ObjectParsed::try_from_bytes(&self.data) }
            .expect("expecting data is valid")
        {
            ObjectParsed::ByteBuffer(buf) => Ok(buf.buffer),
            _ => return Err(Error::InvalidObjectType),
        };
        self.current_offset += Buffer::size_for_raw();
        self.offsets = &self.offsets[1..];
        ret
    }

    pub fn read_fd(&mut self) -> Result<OwnedFd, Error> {
        if self.current_offset
            != *self
                .offsets
                .first()
                .ok_or(Error::AttemptingToReadBinderObjectOnWrongOffset)?
        {
            return Err(Error::AttemptingToReadBinderObjectOnWrongOffset);
        }

        match unsafe { ObjectParsed::try_from_bytes(&self.data) }.expect("expecting data is valid")
        {
            ObjectParsed::LocalReference(_)
            | ObjectParsed::RemoteReference(_)
            | ObjectParsed::ByteBuffer(_) => Err(Error::InvalidObjectType),
            ObjectParsed::Fd(owned_fd) => {
                // At the moment the owned_fd is not actually can be returnable
                // its owned by the source Packet
                let duped = nix::unistd::dup(owned_fd.as_fd());
                // This is owned by the source packet, dont drop it
                mem::forget(owned_fd);

                let duped = duped
                    .context("Cannot duplicate fd")
                    .map_err(Error::ErrorReadingFD)?;

                let mut flags = FdFlag::from_bits(
                    nix::fcntl::fcntl(duped.as_fd(), nix::fcntl::F_GETFD)
                        .context("Getting FD flags")
                        .map_err(Error::ErrorReadingFD)?,
                )
                .ok_or(anyhow!("Cannot create FdFlag"))
                .map_err(Error::ErrorReadingFD)?;
                flags |= FdFlag::FD_CLOEXEC;
                nix::fcntl::fcntl(duped.as_fd(), nix::fcntl::F_SETFD(flags))
                    .context("Setting FD flags to have O_CLOEXEC")
                    .map_err(Error::ErrorReadingFD)?;

                self.current_offset += fd::size_for_raw();
                self.offsets = &self.offsets[1..];
                Ok(duped)
            }
        }
    }

    pub fn read_reference(&mut self) -> Result<Proxy, Error> {
        if self.current_offset
            != *self
                .offsets
                .first()
                .ok_or(Error::AttemptingToReadBinderObjectOnWrongOffset)?
        {
            return Err(Error::AttemptingToReadBinderObjectOnWrongOffset);
        }

        // SAFETY: The data that came to packet is closely
        // controlled to be only contain valid object with valid
        // pointers
        let ret = match unsafe { ObjectParsed::try_from_bytes(&self.data) }
            .expect("expecting data is valid")
        {
            ObjectParsed::LocalReference(ObjectRefLocal { data, .. }) => Ok(Proxy::from_object(
                self.runtime.local_objects.get(data).unwrap().clone(),
            )),

            ObjectParsed::RemoteReference(remote) => {
                self.runtime.inc_remote_ref(&remote);
                Ok(Proxy {
                    rt: Arc::downgrade(self.runtime),
                    reference: Either::Right(remote),
                })
            }
            _ => return Err(Error::InvalidObjectType),
        };

        self.current_offset += ObjectRef::size_in_bytes_for_raw();
        self.offsets = &self.offsets[1..];
        ret
    }
}
