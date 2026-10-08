use std::sync::{Arc, Weak};

use libbinder::{ContextManagerInfo, Runtime, object::B};
use libbinder_basic::{TryFromProxy, packetable::Serde};

use crate::{
    implementation::file_server::ImplFileServer,
    interface::{
        FILE_SERVER_SERVICE_ID, IFileServer, IServiceManager, iservicemanager::ProxyIServiceManager,
    },
};

pub fn main() {
    let runtime = Runtime::new(
        "/dev/binder",
        ContextManagerInfo::Remote(Box::new(|proxy| {
            Ok(<dyn IServiceManager>::try_from_proxy(proxy)?)
        })),
    )
    .unwrap();

    let manager = runtime
        .get_manager()
        .downcast_ref::<ProxyIServiceManager>()
        .unwrap() as &dyn IServiceManager;

    let file_server = Arc::new_cyclic(|weak| {
        B::new(ImplFileServer::new(
            Arc::downgrade(&runtime),
            weak.clone() as Weak<B<dyn IFileServer>>,
        ))
    });

    manager
        .register(
            file_server.clone(),
            Serde(FILE_SERVER_SERVICE_ID.to_string()),
        )
        .expect("Cannot register file server");

    file_server.wait_shutdown();
}
