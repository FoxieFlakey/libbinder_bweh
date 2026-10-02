use libbinder::object::ObjectTrait;

pub trait IObject: ObjectTrait {
    fn has_interface(&self, interface: &str) -> anyhow::Result<bool>;
}

pub const ID: &str = "foxie.iobject";

pub const HAS_INTERFACE_CODE: u32 = 0;
pub const NEXT_TRANSACTION_CODE: u32 = 1;
