use std::sync::Arc;

use either::Either;
use thiserror::Error;

use crate::{object::ObjectTrait, packet::Packet, proxy::Proxy};

pub struct Reader<'a> {
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
        Ok(())
    }

    pub fn read_reference(&mut self) -> Result<Either<Arc<Box<dyn ObjectTrait>>, Proxy>, Error> {
        todo!()
    }
}
