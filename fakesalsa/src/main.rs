use hex_literal::hex;
use std::io::prelude::*;
use std::net::{TcpListener, TcpStream};
use std::process;

fn controller_connection(mut stream: TcpStream) {
    loop {
        let mut command_buffer = [0; 13];
        match stream.read(&mut command_buffer) {
            Ok(0) => {
                println!("Client closed connection.");
                break;
            }
            Ok(13) => {
                println!("Client sent: {:02X?}", command_buffer);
                // FIXME: Send proper response based on sent command
                println!("Sending ACK");
                // FIXME: Error handling
                stream.write(&hex!("57000000000000000000000020")).unwrap();
            }
            Ok(n) => {
                println!(
                    "Client sent {} bytes, expected 13. Data: {:02X?}",
                    n, command_buffer
                );
            }
            _ => {
                // FIXME: Handle these errors more gracefully.
                println!("Something went wrong!");
            }
        };
    }
}

fn main() {
    let address = "127.0.0.1:3001";
    let listener = match TcpListener::bind(address) {
        Ok(listener) => listener,
        Err(err) => {
            println!("Failed to bind to address {} ({})", address, err);
            process::exit(1);
        }
    };
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => controller_connection(stream),
            Err(err) => {
                println!("Failed to accept connection ({})", err);
            }
        }
    }
}
