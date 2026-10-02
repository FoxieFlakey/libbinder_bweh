use std::sync::Weak;

use anyhow::anyhow;
use libbinder::{
    Runtime,
    object::{B, Flag, ObjectTrait},
    packet::{self, Packet},
    proxy::Proxy,
};

use crate::{
    implementation::service::ImplService,
    interface::{
        REPLY_ERROR, REPLY_SUCCESS,
        calculator::{self, ICalculator},
        object::IObject,
        service::IService,
    },
};

pub struct ImplCalculator {
    base: ImplService,
    derived: Weak<B<dyn ICalculator>>,
}

impl ImplCalculator {
    pub fn new(runtime: Weak<Runtime>, derived: Weak<B<dyn ICalculator>>) -> Self {
        Self {
            base: ImplService::new(runtime, derived.clone() as Weak<B<dyn IService>>),
            derived,
        }
    }

    pub fn base(&self) -> &ImplService {
        &self.base
    }
}

impl ObjectTrait for ImplCalculator {
    fn get_remote<'a>(&'a self) -> Option<&'a Proxy> {
        self.base.get_remote()
    }

    fn get_runtime<'a>(&'a self) -> &'a Weak<Runtime> {
        self.base.get_runtime()
    }

    fn on_transaction(
        &self,
        code: u32,
        flags: enumflags2::BitFlags<Flag>,
        message: &mut Packet,
    ) -> Option<(u32, Packet)> {
        let mut reader = message.reader();
        let response = match code {
            calculator::ADD_CODE => {
                let mut a_raw = [0; 4];
                match reader.read_bytes(&mut a_raw) {
                    Ok(_) => {
                        let mut b_raw = [0; 4];
                        match reader.read_bytes(&mut b_raw) {
                            Ok(_) => {
                                let ret = self
                                    .derived
                                    .upgrade()
                                    .unwrap()
                                    .add(f32::from_ne_bytes(a_raw), f32::from_ne_bytes(b_raw));

                                ret.map(|x| {
                                    let mut writer =
                                        packet::Writer::new(self.get_runtime().upgrade().unwrap());
                                    writer.write_bytes(x.to_ne_bytes());
                                    Some(writer.finish())
                                })
                            }
                            Err(e) => Err(anyhow!("Cannot read 'a' for ADD_CODE: {e}")),
                        }
                    }
                    Err(e) => Err(anyhow!("Cannot read 'a' for ADD_CODE: {e}")),
                }
            }
            calculator::SUB_CODE => {
                let mut a_raw = [0; 4];
                match reader.read_bytes(&mut a_raw) {
                    Ok(_) => {
                        let mut b_raw = [0; 4];
                        match reader.read_bytes(&mut b_raw) {
                            Ok(_) => {
                                let ret = self
                                    .derived
                                    .upgrade()
                                    .unwrap()
                                    .sub(f32::from_ne_bytes(a_raw), f32::from_ne_bytes(b_raw));

                                ret.map(|x| {
                                    let mut writer =
                                        packet::Writer::new(self.get_runtime().upgrade().unwrap());
                                    writer.write_bytes(x.to_ne_bytes());
                                    Some(writer.finish())
                                })
                            }
                            Err(e) => Err(anyhow!("Cannot read 'a' for ADD_CODE: {e}")),
                        }
                    }
                    Err(e) => Err(anyhow!("Cannot read 'a' for ADD_CODE: {e}")),
                }
            }
            calculator::MUL_CODE => {
                let mut a_raw = [0; 4];
                match reader.read_bytes(&mut a_raw) {
                    Ok(_) => {
                        let mut b_raw = [0; 4];
                        match reader.read_bytes(&mut b_raw) {
                            Ok(_) => {
                                let ret = self
                                    .derived
                                    .upgrade()
                                    .unwrap()
                                    .mul(f32::from_ne_bytes(a_raw), f32::from_ne_bytes(b_raw));

                                ret.map(|x| {
                                    let mut writer =
                                        packet::Writer::new(self.get_runtime().upgrade().unwrap());
                                    writer.write_bytes(x.to_ne_bytes());
                                    Some(writer.finish())
                                })
                            }
                            Err(e) => Err(anyhow!("Cannot read 'a' for ADD_CODE: {e}")),
                        }
                    }
                    Err(e) => Err(anyhow!("Cannot read 'a' for ADD_CODE: {e}")),
                }
            }
            calculator::DIV_CODE => {
                let mut a_raw = [0; 4];
                match reader.read_bytes(&mut a_raw) {
                    Ok(_) => {
                        let mut b_raw = [0; 4];
                        match reader.read_bytes(&mut b_raw) {
                            Ok(_) => {
                                let ret = self
                                    .derived
                                    .upgrade()
                                    .unwrap()
                                    .div(f32::from_ne_bytes(a_raw), f32::from_ne_bytes(b_raw));

                                ret.map(|x| {
                                    let mut writer =
                                        packet::Writer::new(self.get_runtime().upgrade().unwrap());
                                    writer.write_bytes(x.to_ne_bytes());
                                    Some(writer.finish())
                                })
                            }
                            Err(e) => Err(anyhow!("Cannot read 'a' for ADD_CODE: {e}")),
                        }
                    }
                    Err(e) => Err(anyhow!("Cannot read 'a' for ADD_CODE: {e}")),
                }
            }
            calculator::MODULO_CODE => {
                let mut a_raw = [0; 4];
                match reader.read_bytes(&mut a_raw) {
                    Ok(_) => {
                        let mut b_raw = [0; 4];
                        match reader.read_bytes(&mut b_raw) {
                            Ok(_) => {
                                let ret = self
                                    .derived
                                    .upgrade()
                                    .unwrap()
                                    .modulo(f32::from_ne_bytes(a_raw), f32::from_ne_bytes(b_raw));

                                ret.map(|x| {
                                    let mut writer =
                                        packet::Writer::new(self.get_runtime().upgrade().unwrap());
                                    writer.write_bytes(x.to_ne_bytes());
                                    Some(writer.finish())
                                })
                            }
                            Err(e) => Err(anyhow!("Cannot read 'a' for ADD_CODE: {e}")),
                        }
                    }
                    Err(e) => Err(anyhow!("Cannot read 'a' for ADD_CODE: {e}")),
                }
            }
            _ => return self.base.on_transaction(code, flags, message),
        };

        match response {
            Ok(Some(response)) => Some((REPLY_SUCCESS, response)),
            Ok(None) => {
                assert!(
                    flags.contains(Flag::OneWay),
                    "Expecting reply, but got none"
                );
                None
            }
            Err(e) => {
                if flags.contains(Flag::OneWay) {
                    return None;
                }

                let mut writer = packet::Writer::new(self.get_runtime().upgrade().unwrap());
                writer.write_bytes(format!("{e:#}"));
                Some((REPLY_ERROR, writer.finish()))
            }
        }
    }
}

impl IObject for ImplCalculator {
    fn has_interface(&self, interface: &str) -> anyhow::Result<bool> {
        match interface {
            calculator::ID => Ok(true),
            _ => self.base.has_interface(interface),
        }
    }
}

impl IService for ImplCalculator {
    fn stop(&self) {
        self.base.stop()
    }

    fn say_hello(&self) -> anyhow::Result<()> {
        println!(
            "[Calculator] Hi from calculator, requested by {}",
            self.get_runtime()
                .upgrade()
                .unwrap()
                .get_caller_identity()
                .sender_pid
        );
        self.base.say_hello()
    }
}

impl ICalculator for ImplCalculator {
    fn add(&self, a: f32, b: f32) -> anyhow::Result<f32> {
        Ok(a + b)
    }

    fn sub(&self, a: f32, b: f32) -> anyhow::Result<f32> {
        Ok(a - b)
    }

    fn mul(&self, a: f32, b: f32) -> anyhow::Result<f32> {
        Ok(a * b)
    }

    fn div(&self, a: f32, b: f32) -> anyhow::Result<f32> {
        Ok(a / b)
    }

    fn modulo(&self, a: f32, b: f32) -> anyhow::Result<f32> {
        Ok(a % b)
    }
}
