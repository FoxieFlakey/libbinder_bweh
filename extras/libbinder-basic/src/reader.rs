use std::{borrow::Cow, os::fd::OwnedFd, str::Utf8Error};

use bytemuck::{Pod, PodCastError};
use libbinder::{object::TransactionError, packet::ReadError as LibBinderReadError, proxy::Proxy};

// NOTE: if read failed, there no guarantee the state remains unchanged
// clone it before trying if thats needed
#[derive(Clone)]
pub struct Reader<'a>(pub libbinder::packet::Reader<'a>);

#[derive(thiserror::Error, Debug)]
pub enum ReadError {
    #[error("Cannot read data")]
    ReadError(
        #[source]
        #[from]
        LibBinderReadError,
    ),
    #[error("Size of byte buffer is not exact")]
    SizeOfBufferNotExact,
    #[error("String is not valid UTF-8")]
    StringIsNotUTF8(
        #[source]
        #[from]
        Utf8Error,
    ),
    #[error("Try from proxy error")]
    TryFromProxyError(
        #[source]
        #[from]
        TransactionError,
    ),
}

impl<'a> Reader<'a> {
    pub fn read_bytes_raw(&mut self, buf: &mut [u8]) -> Result<(), LibBinderReadError> {
        self.0.read_bytes(buf)
    }

    pub fn read_str(&mut self) -> Result<&'a str, ReadError> {
        let len = self.read_usize()?;
        let data = self.0.get_rest_of_data();
        if data.len() < len {
            return Err(ReadError::ReadError(LibBinderReadError::OutOfBound));
        }

        Ok(str::from_utf8(&data[..len])?)
    }

    fn align_object(&mut self) -> Result<(), LibBinderReadError> {
        let needed = libbinder::packet::Writer::min_object_align();
        let actual = self.0.get_current_offset();
        if actual.is_multiple_of(needed) {
            return Ok(());
        }

        let padding = actual.next_multiple_of(needed) - actual;
        self.0.skip_bytes(padding)
    }

    pub fn read_reference<T>(&mut self) -> Result<T, ReadError>
    where
        T: TryFrom<Proxy, Error = TransactionError>,
    {
        self.align_object()?;

        // Rewind reader if failed
        let reference = self.0.read_reference()?;
        Ok(T::try_from(reference)?)
    }

    pub fn read_fd(&mut self) -> Result<OwnedFd, ReadError> {
        self.align_object()?;
        Ok(self.0.read_fd()?)
    }

    pub fn read_buf(&mut self) -> Result<&'a [u8], ReadError> {
        self.align_object()?;
        Ok(self.0.read_buf()?)
    }

    pub fn read_pod<T>(&mut self) -> Result<Cow<'a, T>, ReadError>
    where
        T: Pod + Sized,
    {
        let bytes = self.0.get_rest_of_data();
        if bytes.len() < size_of::<T>() {
            return Err(ReadError::ReadError(LibBinderReadError::OutOfBound));
        }

        match bytemuck::try_from_bytes::<T>(&bytes[..size_of::<T>()]) {
            Ok(x) => {
                self.0.skip_bytes(size_of::<T>())?;
                Ok(Cow::Borrowed(x))
            }
            Err(PodCastError::TargetAlignmentGreaterAndInputNotAligned) => {
                let owned = bytemuck::pod_read_unaligned(bytes);
                Ok(Cow::Owned(owned))
            }
            Err(bytemuck::PodCastError::OutputSliceWouldHaveSlop)
            | Err(bytemuck::PodCastError::SizeMismatch)
            | Err(bytemuck::PodCastError::AlignmentMismatch) => unreachable!(),
        }
    }

    // Reading from byte buffer, requires size of buffer be exact
    pub fn read_buf_pod<T>(&mut self) -> Result<Cow<'a, T>, ReadError>
    where
        T: Pod + Sized,
    {
        let bytes = self.0.read_buf()?;
        if bytes.len() != size_of::<T>() {
            return Err(ReadError::SizeOfBufferNotExact);
        }

        match bytemuck::try_from_bytes::<T>(&bytes[..size_of::<T>()]) {
            Ok(x) => {
                self.0.skip_bytes(size_of::<T>())?;
                Ok(Cow::Borrowed(x))
            }
            Err(PodCastError::TargetAlignmentGreaterAndInputNotAligned) => {
                let owned = bytemuck::pod_read_unaligned(bytes);
                Ok(Cow::Owned(owned))
            }
            Err(bytemuck::PodCastError::OutputSliceWouldHaveSlop)
            | Err(bytemuck::PodCastError::SizeMismatch)
            | Err(bytemuck::PodCastError::AlignmentMismatch) => unreachable!(),
        }
    }
}

// Gemini generated
macro_rules! impl_read_primitives {
    ($($t:ty, $method:ident);* $(;)?) => {
        impl Reader<'_> {
            $(
                pub fn $method(&mut self) -> Result<$t, LibBinderReadError> {
                    let mut buf = [0u8; size_of::<$t>()];
                    self.read_bytes_raw(&mut buf)?;
                    Ok(<$t>::from_ne_bytes(buf))
                }
            )*
        }
    };
}

impl_read_primitives!(
    u8,   read_u8;
    u16,  read_u16;
    u32,  read_u32;
    u64,  read_u64;
    i8,   read_i8;
    i16,  read_i16;
    i32,  read_i32;
    i64,  read_i64;
    f32,   read_f32;
    f64,   read_f64;
    usize,   read_usize;
    isize,   read_isize;
);
