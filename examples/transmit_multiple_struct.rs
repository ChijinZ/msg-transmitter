extern crate msg_transmitter;
#[macro_use]
extern crate serde_derive;
extern crate bincode;

use msg_transmitter::{MsgServer, MsgClient};

use std::env;

#[derive(Serialize, Deserialize, Debug, Clone)]
enum Message {
    Pointmsg(Point),
    VecOfF32msg(VecOfF32),
    Endmsg(End),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Point {
    x: i32,
    y: i32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct VecOfF32 {
    y: Vec<i32>,
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
    let server: MsgServer<Message> = MsgServer::new("127.0.0.1:6666");
    fn process(msg: Message) -> Vec<(String, Message)> {
        println!("{:?}", msg);
        match msg {
            Message::Pointmsg(point) => {
                vec![("client".to_string(), Message::Pointmsg(point))]
            }
            Message::VecOfF32msg(vec_of_32) => {
                if vec_of_32.y.len() < 10 {
                    vec![("client".to_string(), Message::VecOfF32msg(vec_of_32))]
                } else {
                    vec![("client".to_string(), Message::Endmsg(End))]
                }
            }
            Message::Endmsg(_) => {
                vec![("client".to_string(), Message::Pointmsg(Point { x: 1, y: 1 }))]
            }
            _ => {
                panic!("unexpected message")
            }
        }
    }
    server.start_server("server", Message::Pointmsg(Point { x: 1, y: 1 }), process);
}

fn client() {
    let client: MsgClient<Message> = MsgClient::new("127.0.0.1:6666");
    fn process(msg: Message) -> Vec<Message> {
        println!("{:?}", msg);
        match msg {
            Message::Pointmsg(point) => {
                vec![Message::VecOfF32msg(VecOfF32 { y: vec![point.y + point.x] })]
            }
            Message::VecOfF32msg(vec_of_32) => {
                vec![Message::Pointmsg(Point { x: 1, y: 1 })]
            }
            Message::Endmsg(v) => {
                std::process::exit(0)
            }
            _ => {
                panic!("unexpected message")
            }
        }
    }
    client.start_client("client", process);
}