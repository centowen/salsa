use std::net::TcpListener;
use std::process;

fn main() {
    let address = "127.0.0.1:3001";
    let listener = match TcpListener::bind(address) {
        Ok(listener) => listener,
        Err(err) => {
            println!("Failed to bind to address {} ({})", address, err);
            process::exit(1);
        }
    };
    for _ in listener.incoming() {
        break;
    }
}
