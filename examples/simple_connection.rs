//! A simple transmitting-u32 example between server and client.
//!
//! You can test this out by running:
//!
//!     cargo run --example simple_connection server
//!
//! And then in another window run:
//!
//!     cargo run --example simple_connection client
//!
//! If it works, client should receive even number and server should receive odd number until client
//! receives 20.

extern crate tokio;
extern crate msg_transmitter;

use msg_transmitter::*;
use std::env;

fn main() {
    let a = env::args().skip(1).collect::<Vec<_>>();
    match a.first().unwrap().as_str() {
        "client" => client(),
        "server" => server(),
        _ => panic!("failed"),
    };
}

fn server() {
    let server = create_tcp_server("127.0.0.1:6666", "server");
    let server_task = server.start_server(0, |client_name, msg| {
        println!("{}: {}", client_name, msg);
        vec![(client_name, msg + 1)]
    });
    tokio::run(server_task);
}

fn client() {
    let client = create_tcp_client("127.0.0.1:6666", "client");
    let client_task = client.start_client(|msg: u32| {
        println!("{}", msg);
        if msg < 20 {
            vec![msg + 1]
        } else {
            std::process::exit(0);
        }
    });
    tokio::run(client_task);
}