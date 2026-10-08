use std::{borrow::Cow, fmt::Display, os::fd::OwnedFd, str::Utf8Error};

use anyhow::anyhow;
use bytemuck::{Pod, PodCastError};
use libbinder::{object::TransactionError, packet::ReadError as LibBinderReadError, proxy::Proxy};
use serde::{
    Deserializer,
    de::{self, DeserializeSeed, EnumAccess, MapAccess, SeqAccess, VariantAccess, Visitor},
};

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
    #[error("Error from serde")]
    ErrorFromSerde(#[source] anyhow::Error),
}

impl<'a> Reader<'a> {
    pub fn new(packet: &'a libbinder::packet::Packet) -> Self {
        Self(libbinder::packet::Reader::new(packet))
    }

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

    pub fn read_reference(&mut self) -> Result<Proxy, ReadError> {
        self.align_object()?;

        // Rewind reader if failed
        Ok(self.0.read_reference()?)
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

    pub fn read_bool(&mut self) -> Result<bool, ReadError> {
        if self.read_u8()? == 0 {
            Ok(false)
        } else {
            Ok(true)
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

impl serde::de::Error for ReadError {
    fn custom<T>(msg: T) -> Self
    where
        T: Display,
    {
        Self::ErrorFromSerde(anyhow!("serde: {msg}"))
    }
}

impl<'de> Deserializer<'de> for &mut Reader<'de> {
    type Error = ReadError;

    fn deserialize_any<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        Err(ReadError::ErrorFromSerde(anyhow!(
            "Cannot use deserialize_any"
        )))
    }

    fn deserialize_bool<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_bool(self.read_bool()?)
    }

    fn deserialize_i8<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_i8(self.read_i8()?)
    }

    fn deserialize_i16<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_i16(self.read_i16()?)
    }

    fn deserialize_i32<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_i32(self.read_i32()?)
    }

    fn deserialize_i64<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_i64(self.read_i64()?)
    }

    fn deserialize_u8<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_u8(self.read_u8()?)
    }

    fn deserialize_u16<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_u16(self.read_u16()?)
    }

    fn deserialize_u32<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_u32(self.read_u32()?)
    }

    fn deserialize_u64<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_u64(self.read_u64()?)
    }

    fn deserialize_f32<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_f32(self.read_f32()?)
    }

    fn deserialize_f64<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_f64(self.read_f64()?)
    }

    fn deserialize_char<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        Err(ReadError::ErrorFromSerde(anyhow!(
            "Deserializing 'char' is unsupported"
        )))
    }

    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_str(self.read_str()?)
    }

    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_string(self.read_str()?.to_string())
    }

    fn deserialize_bytes<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_bytes(self.0.get_rest_of_data())
    }

    fn deserialize_byte_buf<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_byte_buf(self.0.get_rest_of_data().to_vec())
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        if self.read_u8()? == 0 {
            visitor.visit_none()
        } else {
            visitor.visit_some(self)
        }
    }

    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_unit()
    }

    fn deserialize_i128<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        let _ = visitor;
        Err(serde::de::Error::custom("i128 is not supported"))
    }

    fn deserialize_u128<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        let _ = visitor;
        Err(serde::de::Error::custom("u128 is not supported"))
    }

    fn is_human_readable(&self) -> bool {
        false
    }

    fn deserialize_unit_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_unit()
    }

    fn deserialize_newtype_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        let len = self.read_usize()?;

        visitor.visit_seq(ReaderSeqAccess {
            reader: self,
            remaining: len,
        })
    }

    fn deserialize_tuple<V>(self, len: usize, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_seq(ReaderSeqAccess {
            reader: self,
            remaining: len,
        })
    }

    fn deserialize_tuple_struct<V>(
        self,
        _name: &'static str,
        len: usize,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_seq(ReaderSeqAccess {
            reader: self,
            remaining: len,
        })
    }

    fn deserialize_map<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        let len = self.read_usize()?;

        visitor.visit_map(ReaderMapAccess {
            reader: self,
            remaining: len,
            waiting_for_value: false,
        })
    }

    fn deserialize_struct<V>(
        self,
        _name: &'static str,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_seq(ReaderSeqAccess {
            reader: self,
            remaining: fields.len(),
        })
    }

    fn deserialize_enum<V>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        let variant_index = self.read_u32()?;

        visitor.visit_enum(ReaderEnumAccess {
            reader: self,
            variant_index,
        })
    }

    fn deserialize_identifier<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        Err(ReadError::ErrorFromSerde(anyhow!(
            "Unsupported deserialize_identifier"
        )))
    }

    fn deserialize_ignored_any<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        Err(ReadError::ErrorFromSerde(anyhow!(
            "Unsupported deserialize_ignored_any"
        )))
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

// CHAT GPT generated

struct ReaderSeqAccess<'a, 'de> {
    reader: &'a mut Reader<'de>,
    remaining: usize,
}

impl<'de, 'a> SeqAccess<'de> for ReaderSeqAccess<'a, 'de> {
    type Error = ReadError;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
    where
        T: DeserializeSeed<'de>,
    {
        if self.remaining == 0 {
            return Ok(None);
        }

        self.remaining -= 1;

        seed.deserialize(&mut *self.reader).map(Some)
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.remaining)
    }
}

struct ReaderEnumAccess<'a, 'de> {
    reader: &'a mut Reader<'de>,
    variant_index: u32,
}

impl<'de, 'a> EnumAccess<'de> for ReaderEnumAccess<'a, 'de> {
    type Error = ReadError;
    type Variant = ReaderVariantAccess<'a, 'de>;

    fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self::Variant), Self::Error>
    where
        V: DeserializeSeed<'de>,
    {
        let variant = seed.deserialize(de::value::U32Deserializer::<ReadError>::new(
            self.variant_index,
        ))?;

        Ok((
            variant,
            ReaderVariantAccess {
                reader: self.reader,
            },
        ))
    }
}

struct ReaderVariantAccess<'a, 'de> {
    reader: &'a mut Reader<'de>,
}

impl<'de, 'a> VariantAccess<'de> for ReaderVariantAccess<'a, 'de> {
    type Error = ReadError;

    fn unit_variant(self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value, Self::Error>
    where
        T: DeserializeSeed<'de>,
    {
        seed.deserialize(self.reader)
    }

    fn tuple_variant<V>(self, len: usize, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_seq(ReaderSeqAccess {
            reader: self.reader,
            remaining: len,
        })
    }

    fn struct_variant<V>(
        self,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_seq(ReaderSeqAccess {
            reader: self.reader,
            remaining: fields.len(),
        })
    }
}

struct ReaderMapAccess<'a, 'de> {
    reader: &'a mut Reader<'de>,
    remaining: usize,
    waiting_for_value: bool,
}

impl<'de, 'a> MapAccess<'de> for ReaderMapAccess<'a, 'de> {
    type Error = ReadError;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Self::Error>
    where
        K: DeserializeSeed<'de>,
    {
        if self.remaining == 0 {
            return Ok(None);
        }

        if self.waiting_for_value {
            return Err(ReadError::ErrorFromSerde(anyhow!(
                "next_key_seed called before next_value_seed"
            )));
        }

        let key = seed.deserialize(&mut *self.reader)?;
        self.waiting_for_value = true;

        Ok(Some(key))
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, Self::Error>
    where
        V: DeserializeSeed<'de>,
    {
        if !self.waiting_for_value {
            return Err(ReadError::ErrorFromSerde(anyhow!(
                "next_value_seed called without a key"
            )));
        }

        let value = seed.deserialize(&mut *self.reader)?;

        self.waiting_for_value = false;
        self.remaining -= 1;

        Ok(value)
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.remaining)
    }
}
