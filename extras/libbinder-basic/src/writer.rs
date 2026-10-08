use std::{fmt, os::fd::BorrowedFd, sync::Arc};

use libbinder::{
    Runtime,
    object::{B, ObjectTrait},
    packet::Packet,
};
use serde::{
    Serialize, Serializer,
    ser::{
        self, Error, SerializeMap, SerializeSeq, SerializeStruct, SerializeStructVariant,
        SerializeTuple, SerializeTupleStruct, SerializeTupleVariant,
    },
};

pub struct Writer(pub libbinder::packet::Writer);

impl Writer {
    pub fn new(runtime: Arc<Runtime>) -> Self {
        Self(libbinder::packet::Writer::new(runtime))
    }

    pub fn clear(&mut self) {
        self.0.clear();
    }

    pub fn finish(self) -> Packet {
        self.0.finish()
    }

    fn align_object(&mut self) {
        let needed = libbinder::packet::Writer::min_object_align();
        let actual = self.0.get_current_offset();
        if actual.is_multiple_of(needed) {
            return;
        }

        let padding = actual.next_multiple_of(needed) - actual;
        for _ in 0..padding {
            self.0.write_bytes(&[0]);
        }
    }

    pub fn write_fd(&mut self, fd: BorrowedFd<'_>) -> anyhow::Result<()> {
        self.align_object();
        self.0.write_fd(fd)
    }

    pub fn write_buf<T>(&mut self, bytes: T)
    where
        T: AsRef<[u8]> + 'static,
    {
        self.0.write_buf(bytes);
    }

    pub fn write_ref(&mut self, reference: &Arc<B<dyn ObjectTrait>>) {
        self.align_object();
        self.0.write_reference(reference.clone());
    }

    pub fn write_bytes_raw<T>(&mut self, bytes: T)
    where
        T: AsRef<[u8]>,
    {
        self.0.write_bytes(bytes);
    }

    pub fn write_bool(&mut self, bool: bool) {
        if bool {
            self.0.write_bytes(&[1]);
        } else {
            self.0.write_bytes(&[0]);
        }
    }

    pub fn write_str(&mut self, string: &str) {
        self.write_usize(string.len());
        self.write_bytes_raw(string.as_bytes());
    }

    #[cfg(feature = "bytemuck")]
    pub fn write_pod<T>(&mut self, data: &T)
    where
        T: bytemuck::Pod,
    {
        self.write_bytes_raw(bytemuck::bytes_of(data));
    }

    #[cfg(feature = "bytemuck")]
    pub fn write_buf_slice_pod_without_len<T>(&mut self, data: &[T])
    where
        T: bytemuck::Pod,
    {
        struct AsBytes<T: bytemuck::Pod>(Box<[T]>);

        impl<T: bytemuck::Pod> AsRef<[u8]> for AsBytes<T> {
            fn as_ref(&self) -> &[u8] {
                bytemuck::cast_slice(&self.0)
            }
        }

        self.write_bytes_raw(AsBytes(data.into()));
    }

    #[cfg(feature = "bytemuck")]
    pub fn write_buf_pod<T>(&mut self, data: T)
    where
        T: bytemuck::Pod,
    {
        struct AsBytes<T: bytemuck::Pod>(T);

        impl<T: bytemuck::Pod> AsRef<[u8]> for AsBytes<T> {
            fn as_ref(&self) -> &[u8] {
                bytemuck::bytes_of(&self.0)
            }
        }

        self.write_bytes_raw(AsBytes(data));
    }
}

// Gemini generated
macro_rules! impl_write_primitives {
    ($($t:ty, $method:ident);* $(;)?) => {
        impl Writer {
            $(
                pub fn $method(&mut self, value: $t) {
                    self.write_bytes_raw(&value.to_ne_bytes());
                }
            )*
        }
    };
}

impl_write_primitives!(
    u8,   write_u8;
    u16,  write_u16;
    u32,  write_u32;
    u64,  write_u64;
    i8,   write_i8;
    i16,  write_i16;
    i32,  write_i32;
    i64,  write_i64;
    f32,   write_f32;
    f64,   write_f64;
    usize,   write_usize;
    isize,   write_isize;
);

/// CHAT GPT GENERATED

/// Serde serialization error.
///
/// Replace this with Foxie's existing error type if one already exists.
#[derive(Debug)]
pub struct WriteError(anyhow::Error);

impl fmt::Display for WriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl std::error::Error for WriteError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.0.source()
    }
}

impl ser::Error for WriteError {
    fn custom<T>(msg: T) -> Self
    where
        T: fmt::Display,
    {
        Self(anyhow::anyhow!(msg.to_string()))
    }
}

impl From<anyhow::Error> for WriteError {
    fn from(error: anyhow::Error) -> Self {
        Self(error)
    }
}

impl<'a> Serializer for &'a mut Writer {
    type Ok = ();

    type Error = WriteError;

    // Everything compound is deliberately unsupported,
    // matching Foxie's Reader.

    type SerializeSeq = SeqSerializer<'a>;
    type SerializeTuple = SeqSerializer<'a>;
    type SerializeTupleStruct = SeqSerializer<'a>;
    type SerializeTupleVariant = VariantSerializer<'a>;
    type SerializeMap = MapSerializer<'a>;
    type SerializeStruct = SeqSerializer<'a>;
    type SerializeStructVariant = VariantSerializer<'a>;

    fn serialize_bool(self, value: bool) -> Result<Self::Ok, Self::Error> {
        self.write_bool(value);
        Ok(())
    }

    fn serialize_i8(self, value: i8) -> Result<Self::Ok, Self::Error> {
        self.write_i8(value);
        Ok(())
    }

    fn serialize_i16(self, value: i16) -> Result<Self::Ok, Self::Error> {
        self.write_i16(value);
        Ok(())
    }

    fn serialize_i32(self, value: i32) -> Result<Self::Ok, Self::Error> {
        self.write_i32(value);
        Ok(())
    }

    fn serialize_i64(self, value: i64) -> Result<Self::Ok, Self::Error> {
        self.write_i64(value);
        Ok(())
    }

    fn serialize_i128(self, _value: i128) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::custom("i128 is not supported"))
    }

    fn serialize_u8(self, value: u8) -> Result<Self::Ok, Self::Error> {
        self.write_u8(value);
        Ok(())
    }

    fn serialize_u16(self, value: u16) -> Result<Self::Ok, Self::Error> {
        self.write_u16(value);
        Ok(())
    }

    fn serialize_u32(self, value: u32) -> Result<Self::Ok, Self::Error> {
        self.write_u32(value);
        Ok(())
    }

    fn serialize_u64(self, value: u64) -> Result<Self::Ok, Self::Error> {
        self.write_u64(value);
        Ok(())
    }

    fn serialize_u128(self, _value: u128) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::custom("u128 is not supported"))
    }

    fn serialize_f32(self, value: f32) -> Result<Self::Ok, Self::Error> {
        self.write_f32(value);
        Ok(())
    }

    fn serialize_f64(self, value: f64) -> Result<Self::Ok, Self::Error> {
        self.write_f64(value);
        Ok(())
    }

    fn serialize_char(self, _value: char) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::custom("Serializing 'char' is unsupported"))
    }

    fn serialize_str(self, value: &str) -> Result<Self::Ok, Self::Error> {
        self.write_str(value);
        Ok(())
    }

    fn serialize_bytes(self, value: &[u8]) -> Result<Self::Ok, Self::Error> {
        // Matches deserialize_bytes(), which consumes raw remaining bytes.
        self.write_bytes_raw(value);
        Ok(())
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        self.write_u8(0);
        Ok(())
    }

    fn serialize_some<T>(self, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + Serialize,
    {
        self.write_u8(1);
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }

    fn serialize_newtype_struct<T>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + Serialize,
    {
        // Matches deserialize_newtype_struct().
        value.serialize(self)
    }

    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        let len =
            len.ok_or_else(|| Self::Error::custom("serialize_seq requires a known length"))?;

        self.write_usize(len);

        Ok(SeqSerializer {
            writer: self,
            expected: len,
            written: 0,
        })
    }

    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        self.write_usize(len);

        Ok(SeqSerializer {
            writer: self,
            expected: len,
            written: 0,
        })
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        Ok(SeqSerializer {
            writer: self,
            expected: len,
            written: 0,
        })
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        variant_index: u32,
        _variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        self.write_u32(variant_index);

        Ok(VariantSerializer {
            writer: self,
            expected: len,
            written: 0,
        })
    }

    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        let len =
            len.ok_or_else(|| Self::Error::custom("serialize_map requires a known length"))?;

        self.write_usize(len);

        Ok(MapSerializer {
            writer: self,
            expected: len,
            written: 0,
            waiting_for_value: false,
        })
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        Ok(SeqSerializer {
            writer: self,
            expected: len,
            written: 0,
        })
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        variant_index: u32,
        _variant: &'static str,
        fields: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        self.write_u32(variant_index);

        Ok(VariantSerializer {
            writer: self,
            expected: fields,
            written: 0,
        })
    }

    fn is_human_readable(&self) -> bool {
        false
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        variant_index: u32,
        _variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        self.write_u32(variant_index);
        Ok(())
    }

    fn serialize_newtype_variant<T>(
        self,
        _name: &'static str,
        variant_index: u32,
        _variant: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + Serialize,
    {
        self.write_u32(variant_index);
        value.serialize(self)
    }
}

pub struct SeqSerializer<'a> {
    writer: &'a mut Writer,
    expected: usize,
    written: usize,
}

impl<'a> SerializeSeq for SeqSerializer<'a> {
    type Ok = ();
    type Error = WriteError;

    fn serialize_element<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut *self.writer)?;
        self.written += 1;
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        if self.written != self.expected {
            return Err(WriteError::custom(format!(
                "sequence length mismatch: expected {}, wrote {}",
                self.expected, self.written
            )));
        }

        Ok(())
    }
}

impl<'a> SerializeTuple for SeqSerializer<'a> {
    type Ok = ();
    type Error = WriteError;

    fn serialize_element<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut *self.writer)?;
        self.written += 1;
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        if self.written != self.expected {
            return Err(WriteError::custom(format!(
                "tuple length mismatch: expected {}, wrote {}",
                self.expected, self.written
            )));
        }

        Ok(())
    }
}

impl<'a> SerializeTupleStruct for SeqSerializer<'a> {
    type Ok = ();
    type Error = WriteError;

    fn serialize_field<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut *self.writer)?;
        self.written += 1;
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        if self.written != self.expected {
            return Err(WriteError::custom(format!(
                "tuple struct length mismatch: expected {}, wrote {}",
                self.expected, self.written
            )));
        }

        Ok(())
    }
}

pub struct VariantSerializer<'a> {
    writer: &'a mut Writer,
    expected: usize,
    written: usize,
}

impl<'a> VariantSerializer<'a> {
    fn write<T>(&mut self, value: &T) -> Result<(), WriteError>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut *self.writer)?;
        self.written += 1;
        Ok(())
    }

    fn finish(self) -> Result<(), WriteError> {
        if self.written != self.expected {
            return Err(WriteError::custom(format!(
                "enum variant field count mismatch: expected {}, wrote {}",
                self.expected, self.written
            )));
        }

        Ok(())
    }
}

impl<'a> SerializeTupleVariant for VariantSerializer<'a> {
    type Ok = ();
    type Error = WriteError;

    fn serialize_field<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        self.write(value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.finish()
    }
}

impl<'a> SerializeStructVariant for VariantSerializer<'a> {
    type Ok = ();
    type Error = WriteError;

    fn serialize_field<T>(&mut self, _key: &'static str, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        self.write(value)
    }

    fn skip_field(&mut self, _key: &'static str) -> Result<(), Self::Error> {
        // For a binary IPC serializer, skipping a field means
        // simply not writing it.
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.finish()
    }
}

impl<'a> SerializeStruct for SeqSerializer<'a> {
    type Ok = ();
    type Error = WriteError;

    fn serialize_field<T>(&mut self, _key: &'static str, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut *self.writer)?;
        self.written += 1;
        Ok(())
    }

    fn skip_field(&mut self, _key: &'static str) -> Result<(), Self::Error> {
        // Nothing is written for skipped fields.
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        if self.written != self.expected {
            return Err(WriteError::custom(format!(
                "struct field count mismatch: expected {}, wrote {}",
                self.expected, self.written
            )));
        }

        Ok(())
    }
}

pub struct MapSerializer<'a> {
    writer: &'a mut Writer,
    expected: usize,
    written: usize,
    waiting_for_value: bool,
}

impl<'a> SerializeMap for MapSerializer<'a> {
    type Ok = ();
    type Error = WriteError;

    fn serialize_key<T>(&mut self, key: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        if self.waiting_for_value {
            return Err(WriteError::custom(
                "serialize_key called before serialize_value",
            ));
        }

        key.serialize(&mut *self.writer)?;
        self.waiting_for_value = true;

        Ok(())
    }

    fn serialize_value<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        if !self.waiting_for_value {
            return Err(WriteError::custom(
                "serialize_value called without serialize_key",
            ));
        }

        value.serialize(&mut *self.writer)?;
        self.waiting_for_value = false;
        self.written += 1;

        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        if self.waiting_for_value {
            return Err(WriteError::custom("map ended while waiting for a value"));
        }

        if self.written != self.expected {
            return Err(WriteError::custom(format!(
                "map entry count mismatch: expected {}, wrote {}",
                self.expected, self.written
            )));
        }

        Ok(())
    }
}
