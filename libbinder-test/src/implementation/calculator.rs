use std::sync::Weak;

use libbinder::{Runtime, object::B};

use crate::{
    implementation::service::ImplService,
    interface::{ICalculator, IObject, IService, icalculator},
};

pub struct ImplCalculator {
    base: ImplService,
}

impl ImplCalculator {
    pub fn new(runtime: Weak<Runtime>, this: Weak<B<dyn ICalculator>>) -> Self {
        Self {
            base: ImplService::new(runtime, this),
        }
    }

    pub fn wait_shutdown(&self) {
        self.base.wait_shutdown();
    }
}

icalculator::decode_and_dispatch!(ImplCalculator, base);

impl IObject for ImplCalculator {
    fn has_interface(&self, interface: &str) -> anyhow::Result<bool> {
        match interface {
            icalculator::ID => Ok(true),
            _ => self.base.has_interface(interface),
        }
    }
}

impl IService for ImplCalculator {
    fn stop(&self) -> anyhow::Result<()> {
        println!("bye bye from calculator :<");
        self.base.stop()
    }

    fn say_hello(&self) -> anyhow::Result<()> {
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
