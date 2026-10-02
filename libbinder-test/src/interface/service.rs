use crate::interface::object::{self, IObject};

pub trait IService: IObject {
    // oneway
    fn stop(&self);
    fn say_hello(&self) -> anyhow::Result<()>;
}

pub const ID: &str = "foxie.iservice";

pub const STOP_CODE: u32 = object::NEXT_TRANSACTION_CODE + 0;
pub const SAY_HELLO_CODE: u32 = object::NEXT_TRANSACTION_CODE + 1;
pub const NEXT_TRANSACTION_CODE: u32 = object::NEXT_TRANSACTION_CODE + 2;
