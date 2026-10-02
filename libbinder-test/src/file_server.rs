use std::sync::{Arc, Weak};

use libbinder::{
    ContextManagerInfo, Runtime,
    object::{B, ObjectTrait},
};

use crate::{
    implementation::file_server::ImplFileServer,
    interface::{
        file_server::{self, IFileServer},
        service_manager::IServiceManager,
    },
    proxy::service_manager::IServiceManagerProxy,
};

pub fn main() {
    let runtime = Runtime::new(
        "/dev/binder",
        ContextManagerInfo::Remote(Box::new(|proxy| {
            IServiceManagerProxy::from_proxy(proxy)
                .map(|x| Arc::new(B::new(x)) as Arc<B<dyn ObjectTrait>>)
        })),
    )
    .unwrap();

    let manager = runtime
        .get_manager()
        .downcast_ref::<IServiceManagerProxy>()
        .unwrap() as &dyn IServiceManager;

    let file_server = Arc::new_cyclic(|weak| {
        B::new(ImplFileServer::new(
            Arc::downgrade(&runtime),
            weak.clone() as Weak<B<dyn IFileServer>>,
        ))
    });

    manager
        .register(file_server.clone(), file_server::SERVICE_ID)
        .expect("Cannot register file server");

    file_server.base().wait_shutdown();
}
