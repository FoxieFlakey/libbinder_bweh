use std::sync::{Arc, Weak};

use libbinder::{ContextManagerInfo, Runtime, object::B};

use crate::{
    implementation::{self, service_manager::ImplManager},
    interface::IServiceManager,
};

pub fn main() {
    let runtime = Runtime::new(
        "/dev/binder",
        ContextManagerInfo::Concrete(Box::new(|rt| {
            Ok(Arc::new_cyclic(|weak| {
                B::new(implementation::service_manager::ImplManager::new(
                    Arc::downgrade(rt),
                    weak.clone() as Weak<B<dyn IServiceManager>>,
                ))
            }))
        })),
    )
    .unwrap();

    let mgr = runtime.get_manager().downcast_ref::<ImplManager>().unwrap();
    mgr.wait_shutdown();
    println!("Service manager shutted down :3");
}
