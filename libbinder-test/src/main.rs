use std::{
    mem,
    sync::{Arc, Weak},
    thread,
    time::Duration,
};

use enumflags2::BitFlags;

use libbinder::{
    ContextManagerInfo, Runtime,
    object::{self, B, ObjectFlags, ObjectTrait},
    packet::{self, Packet},
    proxy::Proxy,
};

struct Concrete(Weak<Runtime>, String);

impl ObjectTrait for Concrete {
    fn get_remote<'a>(&'a self) -> Option<&'a Proxy> {
        None
    }

    fn get_runtime<'a>(&'a self) -> &'a Weak<Runtime> {
        &self.0
    }

    fn on_transaction(
        &self,
        code: u32,
        _flags: enumflags2::BitFlags<object::Flag>,
        message: &mut Packet,
    ) -> Option<(u32, Packet)> {
        let rt: Arc<Runtime> = self.0.upgrade().unwrap();
        println!(
            "Handled code in {}: {code} from PID {} and EUID {}, security context {}",
            self.1,
            rt.get_caller_identity().sender_pid,
            rt.get_caller_identity().sender_euid,
            rt.get_caller_identity()
                .sender_security_ctx
                .map(|x| format!("{}", x.to_string_lossy()))
                .unwrap_or("unknown".to_string())
        );

        if code == 2929 {
            println!("Special code received 2929 calling back to specific one :333");
            let obj = message.reader().read_reference().unwrap();
            let mut packet = {
                let mut w = packet::Writer::new(self.0.upgrade().unwrap());
                w.write_reference(Arc::new(B::new(Concrete(
                    self.0.clone(),
                    "app".to_string(),
                ))));
                w.write_bytes(0x29u8.to_ne_bytes());
                w.write_bytes(0x38u32.to_ne_bytes());
                w.finish()
            };

            obj.on_transaction(1111, BitFlags::default(), &mut packet)
                .unwrap();
        }

        if code == 1111 {
            println!("Special code received 1111 :333");
        }
        Some((0, packet::Writer::new(self.0.upgrade().unwrap()).finish()))
    }
}

pub fn main() {
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
                        Ok(Arc::new(B::new_with_flags(
                            Concrete(Arc::downgrade(rt), "context manager".to_string()),
                            ObjectFlags {
                                want_transaction_security_context: false,
                                ..Default::default()
                            },
                        )))
                    })),
                )
                .unwrap(),
            );
            loop {
                thread::sleep(Duration::from_secs(2));
            }
        }
        Some("app") => {
            let rt = Runtime::new(
                "/dev/binder",
                ContextManagerInfo::Remote(Box::new(|x| Ok(Arc::new(B::new(x))))),
            )
            .unwrap();

            let mut packet = {
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
                .on_transaction(2929, BitFlags::default(), &mut packet)
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
