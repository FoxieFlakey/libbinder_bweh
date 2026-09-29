use std::{mem, sync::Arc};

use enumflags2::BitFlags;

use crate::{
    ContextManagerInfo, Runtime,
    object::{self, ObjectTrait},
    packet,
};

struct Concrete(String);

impl ObjectTrait for Concrete {
    fn on_transaction(
        &self,
        code: u32,
        _flags: enumflags2::BitFlags<object::Flag>,
        _message: &packet::Packet,
        _reply: Option<(&mut u32, &mut BitFlags<object::Flag>, &mut packet::Writer)>,
    ) -> anyhow::Result<()> {
        println!("Handled code in {}: {code}", self.0);
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
                    ContextManagerInfo::Concrete(Arc::new(Box::new(Concrete(
                        "context manager".to_string(),
                    )))),
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
                let mut w = packet::Writer::new(rt.clone());
                w.write_reference(Arc::new(Box::new(Concrete("app".to_string()))));
                w.write_bytes(0x29u8.to_ne_bytes());
                w.write_bytes(0x38u32.to_ne_bytes());
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
