use std::{
    io::{self, Write},
    sync::Arc,
};

use libbinder::{
    ContextManagerInfo, Runtime,
    object::{B, ObjectTrait},
    proxy::Proxy,
};

use crate::{
    interface::{
        calculator::{self, ICalculator},
        service_manager::IServiceManager,
    },
    proxy::{calculator::ICalculatorProxy, service_manager::IServiceManagerProxy},
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

    let service = manager
        .get_service(calculator::SERVICE_ID)
        .expect("Getting calculator service");
    let calculator =
        &ICalculatorProxy::from_proxy(Proxy::from_object(service)).unwrap() as &dyn ICalculator;

    println!("Interacting mode :3");
    println!("Commands available, '+', '-', '*', '%', 'check' and 'stop'");
    println!("Exmaple usage: input '+,2,4.0' => 6.0");

    loop {
        print!("> ");
        io::stdout().flush().unwrap();
        let mut line = String::new();
        io::stdin().read_line(&mut line).unwrap();
        let mut args = line.split(',').map(|x| x.trim());

        let Some(cmd) = args.next() else {
            continue;
        };

        match cmd {
            "+" | "-" | "*" | "/" | "%" => {
                let Some(a) = args.next().map(|x| x.parse::<f32>().ok()).flatten() else {
                    println!("expecting number for argument #1");
                    continue;
                };
                let Some(b) = args.next().map(|x| x.parse::<f32>().ok()).flatten() else {
                    println!("expecting number for argument #2");
                    continue;
                };

                let ret = match cmd {
                    "+" => calculator.add(a, b),
                    "-" => calculator.sub(a, b),
                    "*" => calculator.mul(a, b),
                    "/" => calculator.div(a, b),
                    "%" => calculator.modulo(a, b),
                    _ => unreachable!(),
                };

                match ret {
                    Ok(ret) => {
                        println!("Result is {ret}");
                    }
                    Err(e) => {
                        println!("Error while executing: {e}");
                    }
                }
            }
            "check" => match manager.health_check() {
                Ok(_) => println!("Health check done"),
                Err(e) => println!("Cannot perform health check: {e}"),
            },
            "stop" => {
                manager.shutdown();
                println!("Good bye! UwU");
                break;
            }
            x => println!("Unknown command '{x}'"),
        }
    }
}
