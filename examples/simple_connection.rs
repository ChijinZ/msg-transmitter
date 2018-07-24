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


extern crate msg_transmitter;

use msg_transmitter::{MsgServer, MsgClient};
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
    let server: MsgServer<u32> = MsgServer::new("127.0.0.1:6666", "server");
    server.start_server(0, |msg: u32| {
        println!("{}", msg);
        vec![("client".to_string(), msg + 1)]
    });
}

fn client() {
    let client: MsgClient<u32> = MsgClient::new("127.0.0.1:6666", "client");
    client.start_client(|msg: u32| {
        println!("{}", msg);
        if msg < 20 {
            vec![msg + 1]
        } else {
            std::process::exit(0);
        }
    });
}