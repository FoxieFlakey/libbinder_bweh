use crate::interface::service::{self, IService};

pub trait ICalculator: IService {
    fn add(&self, a: f32, b: f32) -> anyhow::Result<f32>;
    fn sub(&self, a: f32, b: f32) -> anyhow::Result<f32>;
    fn mul(&self, a: f32, b: f32) -> anyhow::Result<f32>;
    fn div(&self, a: f32, b: f32) -> anyhow::Result<f32>;
    fn modulo(&self, a: f32, b: f32) -> anyhow::Result<f32>;
}

pub const ID: &str = "foxie.icalculator";
pub const SERVICE_ID: &str = "foxie.calculator";

pub const ADD_CODE: u32 = service::NEXT_TRANSACTION_CODE + 0;
pub const SUB_CODE: u32 = service::NEXT_TRANSACTION_CODE + 1;
pub const MUL_CODE: u32 = service::NEXT_TRANSACTION_CODE + 2;
pub const DIV_CODE: u32 = service::NEXT_TRANSACTION_CODE + 3;
pub const MODULO_CODE: u32 = service::NEXT_TRANSACTION_CODE + 4;

#[expect(unused)]
pub const NEXT_TRANSACTION_CODE: u32 = service::NEXT_TRANSACTION_CODE + 5;
