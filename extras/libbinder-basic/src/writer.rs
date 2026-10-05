use std::{os::fd::BorrowedFd, sync::Arc};

use libbinder::{
    Runtime,
    object::{B, ObjectTrait},
    packet::Packet,
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

    pub fn write_ref<T>(&mut self, reference: &Arc<B<T>>)
    where
        T: ObjectTrait,
    {
        self.align_object();
        let reference = reference.clone() as Arc<B<dyn ObjectTrait>>;
        self.0.write_reference(reference);
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
