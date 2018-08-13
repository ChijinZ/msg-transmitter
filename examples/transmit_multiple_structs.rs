//! A example to show how to transmit multiple structs.
//!
//! If you want to transmit multiple structs, you should make your struct derive Serialize and
//! Deserialize trait and wrap them in an enum.
//!
//! You can test this out by running:
//!
//!     cargo run --example transmit_multiple_structs server
//!
//! And then in another window run:
//!
//!     cargo run --example transmit_multiple_structs client
//!

extern crate msg_transmitter;
#[macro_use]
extern crate serde_derive;
extern crate bincode;
extern crate tokio;

use msg_transmitter::*;

use std::env;

#[derive(Serialize, Deserialize, Debug, Clone)]
enum Message {
    VecOfF32msg(VecOfF32),
    Endmsg(End),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct VecOfF32 {
    vec: Vec<u32>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct End;

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
    let server_task = server.start_server(Message::VecOfF32msg(VecOfF32 { vec: vec![] }), |client_name: String, msg: Message| {
        println!("{}: {:?}", client_name, msg);
        match msg {
            Message::VecOfF32msg(vec_of_32) => {
                if vec_of_32.vec.len() < 10 {
                    vec![(client_name, Message::VecOfF32msg(vec_of_32))]
                } else {
                    vec![(client_name, Message::Endmsg(End))]
                }
            }
            Message::Endmsg(_) => {
                std::process::exit(0)
            }
        }
    });
    tokio::run(server_task);
}

fn client() {
    let client = create_tcp_client("127.0.0.1:6666", "client");
    // x is used to test whether the closure can change the outer mutable variable
    let mut x: u32 = 0;
    let client_task = client.start_client(move |msg: Message| {
        println!("{:?}", msg);
        match msg {
            Message::VecOfF32msg(mut vec_of_32) => {
                x += 1;
                vec_of_32.vec.push(1);
                vec![Message::VecOfF32msg(vec_of_32)]
            }
            Message::Endmsg(_) => {
                println!("Outer count is {:?}", x);
                std::process::exit(0)
            }
        }
    });
    tokio::run(client_task);
}

