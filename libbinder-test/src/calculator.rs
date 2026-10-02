use std::sync::{Arc, Weak};

use libbinder::{
    ContextManagerInfo, Runtime,
    object::{B, ObjectTrait},
};

use crate::{
    implementation::calculator::ImplCalculator,
    interface::{
        calculator::{self, ICalculator},
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

    let calculator = Arc::new_cyclic(|weak| {
        B::new(ImplCalculator::new(
            Arc::downgrade(&runtime),
            weak.clone() as Weak<B<dyn ICalculator>>,
        ))
    });

    manager
        .register(calculator.clone(), calculator::SERVICE_ID)
        .expect("Cannot register calculator");

    calculator.base().wait_shutdown();
}
