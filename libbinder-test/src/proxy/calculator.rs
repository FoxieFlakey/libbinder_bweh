use std::sync::Arc;

use anyhow::{Context, bail};
use enumflags2::BitFlags;
use libbinder::{
    Runtime,
    object::{Flag, ObjectTrait, TransactionError},
    packet::{self, Packet},
    proxy::Proxy,
};

use crate::{
    interface::{
        REPLY_SUCCESS,
        calculator::{self, ICalculator},
        object::IObject,
        service::IService,
    },
    proxy::{self, service::IServiceProxy},
};

pub struct ICalculatorProxy(IServiceProxy);

impl ICalculatorProxy {
    pub fn from_proxy(proxy: Proxy) -> anyhow::Result<Self> {
        let super_proxy = IServiceProxy::from_proxy(proxy)?;
        if !super_proxy
            .has_interface(calculator::ID)
            .context("Cannot check if remote supports ICalculator")?
        {
            bail!("Remote object doesnt support ICalculator");
        }
        Ok(Self(super_proxy))
    }
}

impl ObjectTrait for ICalculatorProxy {
    fn get_remote<'a>(&'a self) -> Option<&'a Proxy> {
        self.0.get_remote()
    }

    fn get_runtime(&self) -> Arc<Runtime> {
        self.0.get_runtime()
    }

    fn on_transaction(
        &self,
        code: u32,
        flags: BitFlags<Flag>,
        message: &mut Packet,
    ) -> Result<Option<(u32, Packet)>, TransactionError> {
        self.0.on_transaction(code, flags, message)
    }
}

impl IObject for ICalculatorProxy {
    fn has_interface(&self, interface: &str) -> anyhow::Result<bool> {
        self.0.has_interface(interface)
    }
}

impl IService for ICalculatorProxy {
    fn stop(&self) -> anyhow::Result<()> {
        self.0.stop()
    }

    fn say_hello(&self) -> anyhow::Result<()> {
        self.0.say_hello()
    }
}

impl ICalculator for ICalculatorProxy {
    fn add(&self, a: f32, b: f32) -> anyhow::Result<f32> {
        let mut writer = packet::Writer::new(self.0.get_runtime());
        writer.write_bytes(a.to_ne_bytes());
        writer.write_bytes(b.to_ne_bytes());

        let (code, packet) = self
            .on_transaction(
                calculator::ADD_CODE,
                BitFlags::default(),
                &mut writer.finish(),
            )
            .context("Cannot perform transaction")?
            .expect("This suppose be non oneway transaction");
        if code != REPLY_SUCCESS {
            return Err(proxy::decode_error(&packet));
        }

        let response = packet.get_data();
        if response.len() < size_of::<f32>() {
            bail!("Remote returned short response")
        }

        Ok(f32::from_ne_bytes(*response.as_array().unwrap()))
    }

    fn sub(&self, a: f32, b: f32) -> anyhow::Result<f32> {
        let mut writer = packet::Writer::new(self.0.get_runtime());
        writer.write_bytes(a.to_ne_bytes());
        writer.write_bytes(b.to_ne_bytes());

        let (code, packet) = self
            .on_transaction(
                calculator::SUB_CODE,
                BitFlags::default(),
                &mut writer.finish(),
            )
            .context("Cannot perform transaction")?
            .expect("This suppose be non oneway transaction");
        if code != REPLY_SUCCESS {
            return Err(proxy::decode_error(&packet));
        }

        let response = packet.get_data();
        if response.len() < size_of::<f32>() {
            bail!("Remote returned short response")
        }

        Ok(f32::from_ne_bytes(*response.as_array().unwrap()))
    }

    fn mul(&self, a: f32, b: f32) -> anyhow::Result<f32> {
        let mut writer = packet::Writer::new(self.0.get_runtime());
        writer.write_bytes(a.to_ne_bytes());
        writer.write_bytes(b.to_ne_bytes());

        let (code, packet) = self
            .on_transaction(
                calculator::MUL_CODE,
                BitFlags::default(),
                &mut writer.finish(),
            )
            .context("Cannot perform transaction")?
            .expect("This suppose be non oneway transaction");
        if code != REPLY_SUCCESS {
            return Err(proxy::decode_error(&packet));
        }

        let response = packet.get_data();
        if response.len() < size_of::<f32>() {
            bail!("Remote returned short response")
        }

        Ok(f32::from_ne_bytes(*response.as_array().unwrap()))
    }

    fn div(&self, a: f32, b: f32) -> anyhow::Result<f32> {
        let mut writer = packet::Writer::new(self.0.get_runtime());
        writer.write_bytes(a.to_ne_bytes());
        writer.write_bytes(b.to_ne_bytes());

        let (code, packet) = self
            .on_transaction(
                calculator::DIV_CODE,
                BitFlags::default(),
                &mut writer.finish(),
            )
            .context("Cannot perform transaction")?
            .expect("This suppose be non oneway transaction");
        if code != REPLY_SUCCESS {
            return Err(proxy::decode_error(&packet));
        }

        let response = packet.get_data();
        if response.len() < size_of::<f32>() {
            bail!("Remote returned short response")
        }

        Ok(f32::from_ne_bytes(*response.as_array().unwrap()))
    }

    fn modulo(&self, a: f32, b: f32) -> anyhow::Result<f32> {
        let mut writer = packet::Writer::new(self.0.get_runtime());
        writer.write_bytes(a.to_ne_bytes());
        writer.write_bytes(b.to_ne_bytes());

        let (code, packet) = self
            .on_transaction(
                calculator::MODULO_CODE,
                BitFlags::default(),
                &mut writer.finish(),
            )
            .context("Cannot perform transaction")?
            .expect("This suppose be non oneway transaction");
        if code != REPLY_SUCCESS {
            return Err(proxy::decode_error(&packet));
        }

        let response = packet.get_data();
        if response.len() < size_of::<f32>() {
            bail!("Remote returned short response")
        }

        Ok(f32::from_ne_bytes(*response.as_array().unwrap()))
    }
}
