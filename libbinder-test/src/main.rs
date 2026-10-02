mod app;
mod calculator;
mod implementation;
mod interface;
mod once_event;
mod proxy;
mod service_manager;

pub fn main() {
    match std::env::args()
        .collect::<Vec<_>>()
        .get(1)
        .map(String::as_str)
    {
        Some("app") => app::main(),
        Some("service_manager") => service_manager::main(),
        Some("calculator") => calculator::main(),
        Some(x) => eprintln!("Unknown mode: {x}"),
        None => eprintln!("Mode has to be provided"),
    }
}
