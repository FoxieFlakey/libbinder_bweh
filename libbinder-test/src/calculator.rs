use std::sync::{Arc, Weak};

use libbinder::{ContextManagerInfo, Runtime, object::B};

use libbinder_basic::{TryFromProxy, packetable::Serde};

use crate::{
    implementation::calculator::ImplCalculator,
    interface::{
        CALCULATOR_SERVICE_ID, ICalculator, IServiceManager, iservicemanager::ProxyIServiceManager,
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

    let calculator = Arc::new_cyclic(|weak| {
        B::new(ImplCalculator::new(
            Arc::downgrade(&runtime),
            weak.clone() as Weak<B<dyn ICalculator>>,
        ))
    });

    manager
        .register(calculator.clone(), Serde(CALCULATOR_SERVICE_ID.to_string()))
        .expect("Cannot register calculator");

    calculator.wait_shutdown();
}
