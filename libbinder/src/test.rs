use std::mem;

use crate::{ContextManagerInfo, Runtime, SERVICE_MANAGER, object, packet};

pub fn lib_main() {
    println!("Hello world!");

    match std::env::args()
        .collect::<Vec<_>>()
        .get(1)
        .map(String::as_str)
    {
        Some("context_manager") => {
            mem::forget(Runtime::new("/dev/binder", ContextManagerInfo::Concrete(())).unwrap());
            loop {
                nix::unistd::sleep(2);
            }
        }
        Some("app") => {
            let rt = Runtime::new("/dev/binder", ContextManagerInfo::Remote(())).unwrap();

            let packet = {
                let mut w = packet::Writer::new();
                w.write_u8(0x29);
                w.write_u64(0x38);
                w.finish()
            };

            rt.send_packet(
                0x2929,
                object::Flag::OneWay.into(),
                &packet,
                SERVICE_MANAGER,
            )
            .unwrap();
        }
        Some(x) => {
            eprintln!("Unknown mode: {x}");
        }
        None => {
            eprintln!("Mode must be supplied, either 'context_manager' or 'app'");
        }
    }
}
