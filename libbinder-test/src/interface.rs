use std::{os::fd::OwnedFd, sync::Arc};

use libbinder::object::{B, ObjectTrait};
use libbinder_basic::{binder_ipc_object, packetable::Serde};

#[binder_ipc_object(root = true, interface_id = "foxie.IObject")]
pub trait IObject: ObjectTrait {
    fn has_interface(&self, interface: &str) -> anyhow::Result<bool>;
}

#[binder_ipc_object(interface_id = "foxie.iservice")]
pub trait IService: IObject {
    fn stop(&self) -> anyhow::Result<()>;
    fn say_hello(&self) -> anyhow::Result<()>;
}

#[binder_ipc_object(interface_id = "foxie.IServiceManager")]
pub trait IServiceManager: IObject {
    fn shutdown(&self) -> anyhow::Result<()>;
    fn register(&self, service: Arc<B<dyn IService>>, name: Serde<String>) -> anyhow::Result<()>;
    fn unregister(&self, name: &str) -> anyhow::Result<()>;
    fn get_service(&self, name: &str) -> anyhow::Result<Arc<B<dyn IService>>>;
    fn health_check(&self) -> anyhow::Result<()>;

    // Debug stuffs :3 because get node info for remote ref needs
    // service manager
    fn get_refcount(&self, remote: Arc<B<dyn ObjectTrait>>) -> anyhow::Result<(usize, usize)>;
}

#[binder_ipc_object(interface_id = "foxie.ICalculator")]
pub trait ICalculator: IService {
    fn add(&self, a: f32, b: f32) -> anyhow::Result<f32>;
    fn sub(&self, a: f32, b: f32) -> anyhow::Result<f32>;
    fn mul(&self, a: f32, b: f32) -> anyhow::Result<f32>;
    fn div(&self, a: f32, b: f32) -> anyhow::Result<f32>;
    fn modulo(&self, a: f32, b: f32) -> anyhow::Result<f32>;
}

#[binder_ipc_object(interface_id = "foxie.IFileServer")]
pub trait IFileServer: IService {
    // Demonstrates the BINDER_TYPE_FD usage
    fn open_file(&self, path: &str) -> anyhow::Result<OwnedFd>;

    // Demonstrates the BINDER_TYPE_PTR usage
    fn read_file(&self, path: &str) -> anyhow::Result<Vec<u8>>;

    // Demonstrates the BINDER_TYPE_PTR usage
    fn write_file(&self, path: &str, buf: &[u8]) -> anyhow::Result<()>;
}

pub const CALCULATOR_SERVICE_ID: &str = "foxie.calculator";
pub const FILE_SERVER_SERVICE_ID: &str = "foxie.fileserver";
