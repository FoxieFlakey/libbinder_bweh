use std::{
    fs::File,
    io::{self, BufRead, BufReader, Write},
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
        file_server::{self, IFileServer},
        service_manager::IServiceManager,
    },
    proxy::{
        calculator::ICalculatorProxy, file_server::IFileServerProxy,
        service_manager::IServiceManagerProxy,
    },
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

    // service manager cannot fetch reference count for its own local object
    // println!(
    //     "Service manager got {} strong refs",
    //     manager
    //         .get_refcount(runtime.get_manager().clone())
    //         .unwrap()
    //         .0
    // );
    // println!(
    //     "Service manager got {} weak refs",
    //     manager
    //         .get_refcount(runtime.get_manager().clone())
    //         .unwrap()
    //         .1
    // );

    let calculator_service = manager
        .get_service(calculator::SERVICE_ID)
        .expect("Getting calculator service");
    println!(
        "Calculator service got {} strong refs",
        manager.get_refcount(calculator_service.clone()).unwrap().0
    );
    println!(
        "Calculator service got {} weak refs",
        manager.get_refcount(calculator_service.clone()).unwrap().1
    );

    let calculator = &ICalculatorProxy::from_proxy(Proxy::from_object(calculator_service)).unwrap()
        as &dyn ICalculator;
    let file_server_service = manager
        .get_service(file_server::SERVICE_ID)
        .expect("Getting file server service");
    println!(
        "File server service got {} strong refs",
        manager.get_refcount(file_server_service.clone()).unwrap().0
    );
    println!(
        "File server service got {} weak refs",
        manager.get_refcount(file_server_service.clone()).unwrap().1
    );

    let file_server = &IFileServerProxy::from_proxy(Proxy::from_object(file_server_service))
        .unwrap() as &dyn IFileServer;

    println!("Interacting mode :3");
    println!("Commands available, '+', '-', '*', '%', 'write', 'read', 'check' and 'stop'");
    println!("Exmaple usage: input '+,2,4.0' => 6.0");

    // Try open .bash_history file in root
    // showing that FD transmitted can be read/write
    // by other unprivileged. This can fail and its fine
    // just create it as root
    match file_server.open_file("/root/.bash_history").map(File::from) {
        Ok(mut x) => {
            println!("First 10 lines of root's bash hitory");
            for (idx, line) in BufReader::new(&mut x).lines().enumerate().take(10) {
                match line {
                    Ok(line) => println!("{idx:2} '{line}'"),
                    Err(e) => {
                        println!("{idx:2} Errored reading {e}")
                    }
                }
            }

            // Write should work but not wanting try, could just accidentally destroyed unwanted stuff
            // keep it as read only test
        }
        Err(e) => println!("[optional test failed] Cannot open .bash_history: {e:#}"),
    }

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
                        println!("Error while executing: {e:#}");
                    }
                }
            }
            "check" => match manager.health_check() {
                Ok(_) => println!("Health check done"),
                Err(e) => println!("Cannot perform health check: {e:#}"),
            },
            "stop" => {
                if let Err(e) = manager.shutdown() {
                    println!("Cannot shutdown manager: {e:#}");
                } else {
                    println!("Good bye! UwU");
                    break;
                }
            }
            "write" => {
                let Some(path) = args.next() else {
                    println!("expecting path name for argument #1");
                    continue;
                };
                let Some(data) = args.next() else {
                    println!("expecting data to write for argument #2");
                    continue;
                };

                if let Err(e) = file_server.write_file(path, data.as_bytes()) {
                    println!("Cannot write to '{path}': {e:#}");
                } else {
                    println!("OK");
                }
            }
            "read" => {
                let Some(path) = args.next() else {
                    println!("expecting path name for argument #1");
                    continue;
                };

                match file_server.read_file(path) {
                    Ok(x) => match str::from_utf8(&x) {
                        Ok(payload) => {
                            println!("Data that is read:");
                            println!("{}", payload);
                        }
                        Err(e) => println!("File contains invalid UTF-8 data: {e}"),
                    },
                    Err(e) => {
                        println!("Cannot read file '{path}': {e:#}");
                    }
                }
            }
            x => println!("Unknown command '{x}'"),
        }
    }
}
