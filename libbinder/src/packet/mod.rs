// The data for packet is de/serialized with serde
// For binder type to work serializer has to specially
// handle to keep track offset of each types. Binder's
// transaction need to know where those are

mod passthru;

// Right now the writer is raw format
// later make this more flexible OR
// make it the only writer
pub use libbinder_sys::transaction::TransactionFlag;
pub use passthru::RawFormat as Writer;

pub struct Packet {
    pub(crate) data: Vec<u8>,
    pub(crate) offsets: Vec<usize>,
}

impl Writer {
    pub fn finish(self) -> Packet {
        Packet {
            data: self.data,
            offsets: self.offsets,
        }
    }
}

impl Packet {
    pub fn into_writer(mut self) -> Writer {
        self.data.clear();
        self.offsets.clear();
        Writer {
            data: self.data,
            offsets: self.offsets,
        }
    }
}
