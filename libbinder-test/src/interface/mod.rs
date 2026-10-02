pub mod calculator;
pub mod object;
pub mod service;
pub mod service_manager;

// The packet contains resulting data, according to method
pub const REPLY_SUCCESS: u32 = 0;

// The packet contains bare UTF8 string for error message
pub const REPLY_ERROR: u32 = 1;
