//! A example to show how to transmit multiple structs.
//!
//! If you want to transmit multiple structs, you should make your struct derive Serialize and
//! Deserialize trait and wrap them in an enum.
//!
//! You can test this out by running:
//!
//!     cargo run --example transmit_multiple_struct server
//!
//! And then in another window run:
//!
//!     cargo run --example transmit_multiple_struct client
//!

extern crate msg_transmitter;
#[macro_use]
extern crate serde_derive;
extern crate bincode;

use msg_transmitter::{MsgServer, MsgClient};

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
    let server: MsgServer<Message> = MsgServer::new("127.0.0.1:6666", "server");
    server.start_server(Message::VecOfF32msg(VecOfF32 { vec: vec![] }), |msg: Message| {
        println!("{:?}", msg);
        match msg {
            Message::VecOfF32msg(vec_of_32) => {
                if vec_of_32.vec.len() < 10 {
                    vec![("client".to_string(), Message::VecOfF32msg(vec_of_32))]
                } else {
                    vec![("client".to_string(), Message::Endmsg(End))]
                }
            }
            Message::Endmsg(_) => {
                std::process::exit(0)
            }
        }
    });
}

fn client() {
    let client: MsgClient<Message> = MsgClient::new("127.0.0.1:6666", "client");
    // x is used to test whether the closure can change the outer mutable variable
    let mut x: u32 = 0;
    client.start_client(move |msg: Message| {
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
}

