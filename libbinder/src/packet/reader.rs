use std::sync::Arc;

use either::Either;
use libbinder_sys::types::{
    ObjectParsed,
    buffer::Buffer,
    reference::{ObjectRef, ObjectRefLocal},
};
use thiserror::Error;

use crate::{Runtime, packet::Packet, proxy::Proxy};

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

        ret.copy_from_slice(&self.data[..ret.len()]);
        self.data = &self.data[ret.len()..];

        self.current_offset += ret.len();

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

    pub fn read_buf(&mut self) -> Result<Buffer<'a>, Error> {
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
        match unsafe { ObjectParsed::try_from_bytes(&self.data) }.expect("expecting data is valid")
        {
            ObjectParsed::ByteBuffer(buf) => {
                self.current_offset += Buffer::size_for_raw();
                Ok(buf)
            }
            _ => return Err(Error::InvalidObjectType),
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
        match unsafe { ObjectParsed::try_from_bytes(&self.data) }.expect("expecting data is valid")
        {
            ObjectParsed::LocalReference(ObjectRefLocal { data, .. }) => {
                self.current_offset += ObjectRef::size_in_bytes_for_raw();
                Ok(Proxy::from_object(
                    self.runtime.local_objects.get(data).unwrap().clone(),
                ))
            }

            ObjectParsed::RemoteReference(remote) => {
                self.current_offset += ObjectRef::size_in_bytes_for_raw();
                self.runtime.inc_remote_ref(&remote);
                Ok(Proxy {
                    rt: Arc::downgrade(self.runtime),
                    reference: Either::Right(remote),
                })
            }
            _ => return Err(Error::InvalidObjectType),
        }
    }
}
