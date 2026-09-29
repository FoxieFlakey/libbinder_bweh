use std::{
    mem,
    sync::{Arc, Weak},
};

use enumflags2::BitFlags;

use crate::{
    ContextManagerInfo, Runtime,
    object::{self, B, ObjectTrait},
    packet,
};

struct Concrete(Weak<Runtime>, String);

impl ObjectTrait for Concrete {
    fn on_transaction(
        &self,
        code: u32,
        _flags: enumflags2::BitFlags<object::Flag>,
        message: &packet::Packet,
        _reply: Option<(&mut u32, &mut BitFlags<object::Flag>, &mut packet::Writer)>,
    ) -> anyhow::Result<()> {
        println!("Handled code in {}: {code}", self.1);

        if code == 2929 {
            println!("Special code received 2929 calling back to specific one :333");
            let obj = message.reader().read_reference().unwrap();
            let packet = {
                let mut w = packet::Writer::new(self.0.upgrade().unwrap());
                w.write_reference(Arc::new(B::new(Concrete(
                    self.0.clone(),
                    "app".to_string(),
                ))));
                w.write_bytes(0x29u8.to_ne_bytes());
                w.write_bytes(0x38u32.to_ne_bytes());
                w.finish()
            };

            obj.on_transaction(1111, BitFlags::default(), &packet, None)
                .unwrap();
        }

        if code == 1111 {
            println!("Special code received 1111 :333");
        }
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
                    ContextManagerInfo::Concrete(Box::new(|rt| {
                        Ok(Arc::new(B::new(Concrete(
                            Arc::downgrade(rt),
                            "context manager".to_string(),
                        ))))
                    })),
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
                ContextManagerInfo::Remote(Box::new(|x| Ok(Arc::new(B::new(x))))),
            )
            .unwrap();

            let packet = {
                let mut w = packet::Writer::new(rt.clone());
                w.write_reference(Arc::new(B::new(Concrete(
                    Arc::downgrade(&rt),
                    "app".to_string(),
                ))));
                w.write_bytes(0x29u8.to_ne_bytes());
                w.write_bytes(0x38u32.to_ne_bytes());
                w.finish()
            };

            rt.get_manager()
                .on_transaction(2929, BitFlags::default(), &packet, None)
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
