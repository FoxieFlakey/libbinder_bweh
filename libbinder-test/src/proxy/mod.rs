use anyhow::anyhow;
use libbinder::packet::Packet;

pub mod calculator;
pub mod object;
pub mod service;
pub mod service_manager;

fn decode_error(packet: &Packet) -> anyhow::Error {
    match str::from_utf8(packet.get_data()) {
        Ok(x) => {
            anyhow!("remote error: {x}")
        }
        Err(e) => {
            anyhow!("cannot parse remote error reply: {e}")
        }
    }
}
