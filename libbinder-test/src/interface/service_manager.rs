use std::sync::Arc;

use libbinder::object::B;

use crate::interface::{
    object::{self, IObject},
    service::IService,
};

pub trait IServiceManager: IObject {
    // oneway
    fn shutdown(&self);
    fn register(&self, service: Arc<B<dyn IService>>, name: &str) -> anyhow::Result<()>;
    fn unregister(&self, name: &str) -> anyhow::Result<()>;
    fn get_service(&self, name: &str) -> anyhow::Result<Arc<B<dyn IService>>>;
    fn health_check(&self) -> anyhow::Result<()>;
}

pub const ID: &str = "foxie.iservice_manager";

pub const SHUTDOWN_CODE: u32 = object::NEXT_TRANSACTION_CODE + 0;
pub const REGISTER_CODE: u32 = object::NEXT_TRANSACTION_CODE + 1;
pub const UNREGISTER_CODE: u32 = object::NEXT_TRANSACTION_CODE + 2;
pub const GET_SERVICE_CODE: u32 = object::NEXT_TRANSACTION_CODE + 3;
pub const HEALTH_CHECK_CODE: u32 = object::NEXT_TRANSACTION_CODE + 4;

#[expect(unused)]
pub const NEXT_TRANSACTION_CODE: u32 = object::NEXT_TRANSACTION_CODE + 5;
