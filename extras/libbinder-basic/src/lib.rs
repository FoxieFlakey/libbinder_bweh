// This library exposes useful types and helpers for using libbinder
pub mod interface;
pub mod packetable;
pub mod reader;
pub mod writer;

pub static REPLY_SUCCESS: u32 = 0;
pub static REPLY_FAILURE: u32 = 1;
