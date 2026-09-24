use std::{mem, sync::Arc};

use enumflags2::BitFlags;

use crate::{
    ContextManagerInfo, Runtime,
    object::{self, Object},
    packet,
};

struct Concrete;

impl Object for Concrete {
    fn on_transaction(
        &self,
        code: u32,
        _flags: enumflags2::BitFlags<object::Flag>,
        _message: &packet::Packet,
        _reply: Option<(&mut u32, &mut BitFlags<object::Flag>, &mut packet::Writer)>,
    ) -> anyhow::Result<()> {
        println!("Handled code: {code}");
        Ok(())
    }
}

pub fn lib_main() {
    println!("Hello world!");

    match std::env::args()
        .collect::<Vec<_>>()
        .get(1)
        .map(String::as_str)
    {
        Some("context_manager") => {
            mem::forget(
                Runtime::new(
                    "/dev/binder",
                    ContextManagerInfo::Concrete(Arc::new(Box::new(Concrete))),
                )
                .unwrap(),
            );
            loop {
                nix::unistd::sleep(2);
            }
        }
        Some("app") => {
            let rt = Runtime::new(
                "/dev/binder",
                ContextManagerInfo::Remote(Box::new(|x| Ok(Arc::new(Box::new(x))))),
            )
            .unwrap();

            let packet = {
                let mut w = packet::Writer::new();
                w.write_u8(0x29);
                w.write_u64(0x38);
                w.finish()
            };

            rt.get_manager()
                .on_transaction(0x2929, object::Flag::OneWay.into(), &packet, None)
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
