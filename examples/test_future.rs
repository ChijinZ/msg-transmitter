extern crate msg_transmitter;
extern crate tokio;

#[macro_use]
extern crate serde_derive;
extern crate bincode;

use tokio::prelude::*;
use msg_transmitter::*;

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
enum Message {
    VecOfF32msg(VecOfF32),
    Endmsg(End),
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
struct VecOfF32 {
    vec: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
struct End;

fn main() {
    let a = std::env::args().skip(1).collect::<Vec<_>>();
    match a.first().unwrap().as_str() {
        "client" => client(),
        "server" => server(),
        _ => panic!("failed"),
    };
}

fn server() {
    let server: TCPMsgServer<Message> = TCPMsgServer::new("127.0.0.1:6666", "server");
    server.start_server(Message::VecOfF32msg(VecOfF32 { vec: 0 }), |msg: Message| {
        println!("{:?}", msg);
        match msg {
            Message::VecOfF32msg(vec_of_32) => {
                if vec_of_32.vec < 10 {
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
    let client: TCPMsgClient<Message> = TCPMsgClient::new("127.0.0.1:6666", "client");
    // x is used to test whether the closure can change the outer mutable variable
    let mut sign = Sign::new();
    client.start_client(move |msg: Message| {
        println!("{:?}", msg);
        match msg {
            Message::VecOfF32msg(mut vec_of_32) => {
                vec_of_32.vec += 1;
                sign.if_true = true;
                vec![Message::VecOfF32msg(vec_of_32)]
            }
            Message::Endmsg(_) => {
                println!("Outer count is {:?}", sign);
                std::process::exit(0)
            }
        }
    }, sign);
}

#[derive(Debug, Clone, Copy)]
struct Sign {
    val: Message,
    if_true: bool,
}

impl Sign {
    fn new() -> Sign {
        Sign { if_true: false, val: Message::Endmsg(End) }
    }
}

impl Future for Sign {
    type Item = Message;
    type Error = std::io::Error;
    fn poll(&mut self) -> Result<Async<Self::Item>, Self::Error> {
        if self.if_true {
            return Ok(Async::Ready(self.val));
        } else {
            return Ok(Async::NotReady);
        }
    }
}
