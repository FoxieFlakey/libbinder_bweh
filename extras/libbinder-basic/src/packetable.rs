use std::{
    os::fd::{AsFd, OwnedFd},
    sync::Arc,
};

use anyhow::{Context, anyhow};
use bytemuck::Pod;
use libbinder::object::B;

use crate::{TryFromProxy, reader::Reader, writer::Writer};

pub trait Packetable {
    type Deserialized<'a>;

    fn serialize(&self, writer: &mut Writer) -> anyhow::Result<()>;
    fn deserialize<'a>(reader: &mut Reader<'a>) -> anyhow::Result<Self::Deserialized<'a>>;
}

impl Packetable for anyhow::Error {
    type Deserialized<'a> = anyhow::Error;

    fn serialize(&self, writer: &mut Writer) -> anyhow::Result<()> {
        writer.write_str(&self.to_string());
        Ok(())
    }

    fn deserialize<'a>(reader: &mut Reader<'a>) -> anyhow::Result<Self::Deserialized<'a>> {
        Ok(anyhow!("{}", reader.read_str()?))
    }
}

impl Packetable for () {
    type Deserialized<'a> = ();

    fn serialize(&self, _: &mut Writer) -> anyhow::Result<()> {
        Ok(())
    }

    fn deserialize<'a>(_: &mut Reader<'a>) -> anyhow::Result<Self::Deserialized<'a>> {
        Ok(())
    }
}

impl Packetable for OwnedFd {
    type Deserialized<'a> = Self;

    fn serialize(&self, writer: &mut Writer) -> anyhow::Result<()> {
        writer.write_fd(self.as_fd())
    }

    fn deserialize<'a>(reader: &mut Reader<'a>) -> anyhow::Result<Self::Deserialized<'a>> {
        Ok(reader.read_fd()?)
    }
}

impl<T: Packetable, E: Packetable> Packetable for Result<T, E> {
    type Deserialized<'a> = Result<T::Deserialized<'a>, E::Deserialized<'a>>;

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

    fn deserialize<'a>(reader: &mut Reader<'a>) -> anyhow::Result<Self::Deserialized<'a>> {
        if reader.read_bool().context("Reading whether result is Ok")? {
            Ok(Ok(T::deserialize(reader)?))
        } else {
            Ok(Err(E::deserialize(reader)?))
        }
    }
}

impl<T: Packetable> Packetable for Option<T> {
    type Deserialized<'a> = Option<T::Deserialized<'a>>;

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

    fn deserialize<'a>(reader: &mut Reader<'a>) -> anyhow::Result<Self::Deserialized<'a>> {
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

impl Packetable for &str {
    type Deserialized<'a> = &'a str;

    fn serialize(&self, writer: &mut Writer) -> anyhow::Result<()> {
        writer.write_str(self);
        Ok(())
    }

    fn deserialize<'a>(reader: &mut Reader<'a>) -> anyhow::Result<Self::Deserialized<'a>> {
        Ok(reader.read_str()?)
    }
}

impl Packetable for str {
    type Deserialized<'a> = String;

    fn serialize(&self, writer: &mut Writer) -> anyhow::Result<()> {
        writer.write_str(self);
        Ok(())
    }

    fn deserialize<'a>(reader: &mut Reader<'a>) -> anyhow::Result<Self::Deserialized<'a>> {
        Ok(reader.read_str()?.to_string())
    }
}

impl Packetable for String {
    type Deserialized<'a> = String;

    fn serialize(&self, writer: &mut Writer) -> anyhow::Result<()> {
        writer.write_str(self);
        Ok(())
    }

    fn deserialize<'a>(reader: &mut Reader<'a>) -> anyhow::Result<Self::Deserialized<'a>> {
        Ok(reader.read_str()?.to_string())
    }
}

impl<A0: Packetable, A1: Packetable> Packetable for (A0, A1) {
    type Deserialized<'a> = (A0::Deserialized<'a>, A1::Deserialized<'a>);

    fn serialize(&self, writer: &mut Writer) -> anyhow::Result<()> {
        self.0.serialize(writer)?;
        self.1.serialize(writer)?;
        Ok(())
    }

    fn deserialize<'a>(reader: &mut Reader<'a>) -> anyhow::Result<Self::Deserialized<'a>> {
        Ok((A0::deserialize(reader)?, A1::deserialize(reader)?))
    }
}

impl<A0: Packetable, A1: Packetable, A2: Packetable> Packetable for (A0, A1, A2) {
    type Deserialized<'a> = (
        A0::Deserialized<'a>,
        A1::Deserialized<'a>,
        A2::Deserialized<'a>,
    );

    fn serialize(&self, writer: &mut Writer) -> anyhow::Result<()> {
        self.0.serialize(writer)?;
        self.1.serialize(writer)?;
        self.2.serialize(writer)?;
        Ok(())
    }

    fn deserialize<'a>(reader: &mut Reader<'a>) -> anyhow::Result<Self::Deserialized<'a>> {
        Ok((
            A0::deserialize(reader)?,
            A1::deserialize(reader)?,
            A2::deserialize(reader)?,
        ))
    }
}

impl<A0: Packetable, A1: Packetable, A2: Packetable, A3: Packetable> Packetable
    for (A0, A1, A2, A3)
{
    type Deserialized<'a> = (
        A0::Deserialized<'a>,
        A1::Deserialized<'a>,
        A2::Deserialized<'a>,
        A3::Deserialized<'a>,
    );

    fn serialize(&self, writer: &mut Writer) -> anyhow::Result<()> {
        self.0.serialize(writer)?;
        self.1.serialize(writer)?;
        self.2.serialize(writer)?;
        Ok(())
    }

    fn deserialize<'a>(reader: &mut Reader<'a>) -> anyhow::Result<Self::Deserialized<'a>> {
        Ok((
            A0::deserialize(reader)?,
            A1::deserialize(reader)?,
            A2::deserialize(reader)?,
            A3::deserialize(reader)?,
        ))
    }
}

impl<T: Pod> Packetable for Vec<T> {
    type Deserialized<'a> = Vec<T>;

    fn serialize(&self, writer: &mut Writer) -> anyhow::Result<()> {
        writer.write_usize(self.len());
        writer.write_buf_slice_pod_without_len(self);
        Ok(())
    }

    fn deserialize<'a>(reader: &mut Reader<'a>) -> anyhow::Result<Self::Deserialized<'a>> {
        let len = reader.read_usize()?;
        let buf = reader.read_buf()?;
        let mut vec = Vec::new();
        vec.extend_from_slice(bytemuck::cast_slice(&buf[..len * size_of::<T>()]));
        Ok(vec)
    }
}

impl<T: Pod> Packetable for &[T] {
    type Deserialized<'a> = &'a [T];

    fn serialize(&self, writer: &mut Writer) -> anyhow::Result<()> {
        writer.write_usize(self.len());
        writer.write_buf_slice_pod_without_len(self);
        Ok(())
    }

    fn deserialize<'a>(reader: &mut Reader<'a>) -> anyhow::Result<Self::Deserialized<'a>> {
        let len = reader.read_usize()?;
        let buf = reader.read_buf()?;

        Ok(bytemuck::cast_slice(&buf[..len * size_of::<T>()]))
    }
}

impl<T: TryFromProxy + ?Sized> Packetable for Arc<B<T>> {
    type Deserialized<'a> = Arc<B<T>>;

    fn serialize(&self, writer: &mut Writer) -> anyhow::Result<()> {
        writer.write_ref(&TryFromProxy::into_base(self.clone()));
        Ok(())
    }

    fn deserialize<'a>(reader: &mut Reader<'a>) -> anyhow::Result<Self::Deserialized<'a>> {
        Ok(T::try_from_proxy(reader.read_reference()?)?)
    }
}

macro_rules! impl_write_primitives {
    ($($t:ty, $method:ident, $method_read:ident);* $(;)?) => {
        $(
            impl Packetable for $t {
                type Deserialized<'a> = Self;

                fn serialize(&self, writer: &mut Writer) -> anyhow::Result<()> {
                    writer.$method(*self);
                    Ok(())
                }

                fn deserialize<'a>(reader: &mut Reader<'a>) -> anyhow::Result<Self::Deserialized<'a>> {
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
