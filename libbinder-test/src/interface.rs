use std::{os::fd::OwnedFd, sync::Arc};

use libbinder::object::{B, ObjectTrait, TransactionError};
use libbinder_basic::{binder_ipc_object, packetable::Serde};

#[binder_ipc_object(root = true, interface_id = "foxie.IObject")]
pub trait IObject: ObjectTrait {
    fn has_interface(&self, interface: &str) -> Result<bool, TransactionError>;
}

#[binder_ipc_object(interface_id = "foxie.iservice")]
pub trait IService: IObject {
    fn stop(&self) -> Result<(), TransactionError>;
    fn say_hello(&self) -> Result<(), TransactionError>;
}

#[binder_ipc_object(interface_id = "foxie.IServiceManager")]
pub trait IServiceManager: IObject {
    fn shutdown(&self) -> Result<(), TransactionError>;
    fn register(
        &self,
        service: Arc<B<dyn IService>>,
        name: Serde<String>,
    ) -> Result<(), TransactionError>;
    fn unregister(&self, name: &str) -> Result<(), TransactionError>;
    fn get_service(&self, name: &str) -> Result<Arc<B<dyn IService>>, TransactionError>;
    fn health_check(&self) -> Result<(), TransactionError>;

    // Debug stuffs :3 because get node info for remote ref needs
    // service manager
    fn get_refcount(
        &self,
        remote: Arc<B<dyn ObjectTrait>>,
    ) -> Result<(usize, usize), TransactionError>;
}

#[binder_ipc_object(interface_id = "foxie.ICalculator")]
pub trait ICalculator: IService {
    fn add(&self, a: f32, b: f32) -> Result<f32, TransactionError>;
    fn sub(&self, a: f32, b: f32) -> Result<f32, TransactionError>;
    fn mul(&self, a: f32, b: f32) -> Result<f32, TransactionError>;
    fn div(&self, a: f32, b: f32) -> Result<f32, TransactionError>;
    fn modulo(&self, a: f32, b: f32) -> Result<f32, TransactionError>;
}

#[binder_ipc_object(interface_id = "foxie.IFileServer")]
pub trait IFileServer: IService {
    // Demonstrates the BINDER_TYPE_FD usage
    fn open_file(&self, path: &str) -> Result<OwnedFd, TransactionError>;

    // Demonstrates the BINDER_TYPE_PTR usage
    fn read_file(&self, path: &str) -> Result<Vec<u8>, TransactionError>;

    // Demonstrates the BINDER_TYPE_PTR usage
    fn write_file(&self, path: &str, buf: &[u8]) -> Result<(), TransactionError>;
}

pub const CALCULATOR_SERVICE_ID: &str = "foxie.calculator";
pub const FILE_SERVER_SERVICE_ID: &str = "foxie.fileserver";
