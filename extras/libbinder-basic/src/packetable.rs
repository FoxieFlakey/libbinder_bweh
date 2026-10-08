use std::{
    ops::{Deref, DerefMut},
    os::fd::{AsFd, OwnedFd},
    sync::Arc,
};

use anyhow::{Context, anyhow};
use bytemuck::Pod;
use libbinder::object::B;
use serde::{Deserialize, Serialize};

use crate::{TryFromProxy, reader::Reader, writer::Writer};

pub trait Packetable<'a>: Sized {
    fn serialize(&self, writer: &mut Writer) -> anyhow::Result<()>;
    fn deserialize(reader: &mut Reader<'a>) -> anyhow::Result<Self>;
}

impl<'a> Packetable<'a> for anyhow::Error {
    fn serialize(&self, writer: &mut Writer) -> anyhow::Result<()> {
        writer.write_str(&self.to_string());
        Ok(())
    }

    fn deserialize(reader: &mut Reader<'a>) -> anyhow::Result<Self> {
        Ok(anyhow!("{}", reader.read_str()?))
    }
}

impl<'a> Packetable<'a> for () {
    fn serialize(&self, _: &mut Writer) -> anyhow::Result<()> {
        Ok(())
    }

    fn deserialize(_: &mut Reader<'a>) -> anyhow::Result<Self> {
        Ok(())
    }
}

impl<'a> Packetable<'a> for OwnedFd {
    fn serialize(&self, writer: &mut Writer) -> anyhow::Result<()> {
        writer.write_fd(self.as_fd())
    }

    fn deserialize(reader: &mut Reader<'a>) -> anyhow::Result<Self> {
        Ok(reader.read_fd()?)
    }
}

impl<'a, T: Packetable<'a>, E: Packetable<'a>> Packetable<'a> for Result<T, E> {
    fn serialize(&self, writer: &mut Writer) -> anyhow::Result<()> {
        match self {
            Ok(x) => {
                writer.write_bool(true);
                x.serialize(writer).context("Writing Ok value")?;
                Ok(())
            }
            Err(x) => {
                writer.write_bool(false);
                x.serialize(writer).context("Writing Err value")?;
                Ok(())
            }
        }
    }

    fn deserialize(reader: &mut Reader<'a>) -> anyhow::Result<Self> {
        if reader.read_bool().context("Reading whether result is Ok")? {
            Ok(Ok(T::deserialize(reader)?))
        } else {
            Ok(Err(E::deserialize(reader)?))
        }
    }
}

impl<'a, T: Packetable<'a>> Packetable<'a> for Option<T> {
    fn serialize(&self, writer: &mut Writer) -> anyhow::Result<()> {
        match self {
            Some(x) => {
                writer.write_bool(true);
                x.serialize(writer).context("Writing Some value")?;
                Ok(())
            }
            None => {
                writer.write_bool(false);
                Ok(())
            }
        }
    }

    fn deserialize(reader: &mut Reader<'a>) -> anyhow::Result<Self> {
        if reader
            .read_bool()
            .context("Reading whether option is Some")?
        {
            Ok(Some(T::deserialize(reader)?))
        } else {
            Ok(None)
        }
    }
}

impl<'a> Packetable<'a> for &'a str {
    fn serialize(&self, writer: &mut Writer) -> anyhow::Result<()> {
        writer.write_str(self);
        Ok(())
    }

    fn deserialize(reader: &mut Reader<'a>) -> anyhow::Result<Self> {
        Ok(reader.read_str()?)
    }
}

impl<'a> Packetable<'a> for String {
    fn serialize(&self, writer: &mut Writer) -> anyhow::Result<()> {
        writer.write_str(self);
        Ok(())
    }

    fn deserialize(reader: &mut Reader<'a>) -> anyhow::Result<Self> {
        Ok(reader.read_str()?.to_string())
    }
}

impl<'a, A0: Packetable<'a>, A1: Packetable<'a>> Packetable<'a> for (A0, A1) {
    fn serialize(&self, writer: &mut Writer) -> anyhow::Result<()> {
        self.0.serialize(writer)?;
        self.1.serialize(writer)?;
        Ok(())
    }

    fn deserialize(reader: &mut Reader<'a>) -> anyhow::Result<Self> {
        Ok((A0::deserialize(reader)?, A1::deserialize(reader)?))
    }
}

impl<'a, A0: Packetable<'a>, A1: Packetable<'a>, A2: Packetable<'a>> Packetable<'a>
    for (A0, A1, A2)
{
    fn serialize(&self, writer: &mut Writer) -> anyhow::Result<()> {
        self.0.serialize(writer)?;
        self.1.serialize(writer)?;
        self.2.serialize(writer)?;
        Ok(())
    }

    fn deserialize(reader: &mut Reader<'a>) -> anyhow::Result<Self> {
        Ok((
            A0::deserialize(reader)?,
            A1::deserialize(reader)?,
            A2::deserialize(reader)?,
        ))
    }
}

impl<'a, A0: Packetable<'a>, A1: Packetable<'a>, A2: Packetable<'a>, A3: Packetable<'a>>
    Packetable<'a> for (A0, A1, A2, A3)
{
    fn serialize(&self, writer: &mut Writer) -> anyhow::Result<()> {
        self.0.serialize(writer)?;
        self.1.serialize(writer)?;
        self.2.serialize(writer)?;
        self.3.serialize(writer)?;
        Ok(())
    }

    fn deserialize(reader: &mut Reader<'a>) -> anyhow::Result<Self> {
        Ok((
            A0::deserialize(reader)?,
            A1::deserialize(reader)?,
            A2::deserialize(reader)?,
            A3::deserialize(reader)?,
        ))
    }
}

impl<'a, T: Pod> Packetable<'a> for Vec<T> {
    fn serialize(&self, writer: &mut Writer) -> anyhow::Result<()> {
        writer.write_usize(self.len());
        writer.write_buf_slice_pod_without_len(self);
        Ok(())
    }

    fn deserialize(reader: &mut Reader<'a>) -> anyhow::Result<Self> {
        let len = reader.read_usize()?;
        let buf = reader.read_buf()?;
        let mut vec = Vec::new();
        vec.extend_from_slice(bytemuck::cast_slice(&buf[..len * size_of::<T>()]));
        Ok(vec)
    }
}

impl<'a, T: Pod> Packetable<'a> for &'a [T] {
    fn serialize(&self, writer: &mut Writer) -> anyhow::Result<()> {
        writer.write_usize(self.len());
        writer.write_buf_slice_pod_without_len(self);
        Ok(())
    }

    fn deserialize(reader: &mut Reader<'a>) -> anyhow::Result<Self> {
        let len = reader.read_usize()?;
        let buf = reader.read_buf()?;

        Ok(bytemuck::cast_slice(&buf[..len * size_of::<T>()]))
    }
}

impl<'a, T: TryFromProxy + ?Sized> Packetable<'a> for Arc<B<T>> {
    fn serialize(&self, writer: &mut Writer) -> anyhow::Result<()> {
        writer.write_ref(&TryFromProxy::into_base(self.clone()));
        Ok(())
    }

    fn deserialize(reader: &mut Reader<'a>) -> anyhow::Result<Self> {
        Ok(T::try_from_proxy(reader.read_reference()?)?)
    }
}

macro_rules! impl_write_primitives {
    ($($t:ty, $method:ident, $method_read:ident);* $(;)?) => {
        $(
            impl<'a> Packetable<'a> for $t {
                fn serialize(&self, writer: &mut Writer) -> anyhow::Result<()> {
                    writer.$method(*self);
                    Ok(())
                }

                fn deserialize(reader: &mut Reader<'a>) -> anyhow::Result<Self> {
                    Ok(reader.$method_read()?)
                }
            }
        )*
    };
}

impl_write_primitives!(
    u8,   write_u8, read_u8;
    u16,  write_u16, read_u16;
    u32,  write_u32, read_u32;
    u64,  write_u64, read_u64;
    i8,   write_i8, read_i8;
    i16,  write_i16, read_i16;
    i32,  write_i32, read_i32;
    i64,  write_i64, read_i64;
    f32,   write_f32, read_f32;
    f64,   write_f64, read_f64;
    usize,   write_usize, read_usize;
    isize,   write_isize, read_isize;
    bool, write_bool, read_bool;
);

pub struct Serde<T>(pub T);

impl<T> Deref for Serde<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for Serde<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

// Serde support
impl<'a, T: Serialize + Deserialize<'a>> Packetable<'a> for Serde<T> {
    fn serialize(&self, writer: &mut Writer) -> anyhow::Result<()> {
        Ok(self.0.serialize(writer)?)
    }

    fn deserialize(reader: &mut Reader<'a>) -> anyhow::Result<Self> {
        Ok(Serde(T::deserialize(reader)?))
    }
}
