use std::sync::Arc;

use libbinder_sys::types::{ObjectParsed, reference::ObjectRefLocal};
use thiserror::Error;

use crate::{
    Runtime,
    object::{B, ObjectTrait},
    packet::Packet,
};

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

    pub fn read_reference(&mut self) -> Result<Arc<B<dyn ObjectTrait>>, Error> {
        if self.current_offset
            != *self
                .offsets
                .first()
                .ok_or(Error::AttemptingToReadBinderObjectOnWrongOffset)?
        {
            return Err(Error::AttemptingToReadBinderObjectOnWrongOffset);
        }

        match ObjectParsed::try_from_bytes(&self.data).expect("expecting data is valid") {
            ObjectParsed::LocalReference(ObjectRefLocal { data, .. }) => {
                Ok(self.runtime.local_objects.get(data).unwrap().clone())
            }
            ObjectParsed::RemoteReference(_) => todo!(),
        }
    }
}
