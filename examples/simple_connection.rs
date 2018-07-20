extern crate msg_transmitter;
use msg_transmitter::{MsgServer,MsgClient};
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
    let server: MsgServer<u32> = MsgServer::new("127.0.0.1:6666");
    fn process(msg: u32) -> Vec<(String, u32)> {
        println!("{}", msg);
        vec![("client".to_string(), msg + 1)]
    }
    server.start_server("server", 0, process);
}

fn client() {
    let client: MsgClient<u32> = MsgClient::new("127.0.0.1:6666");
    fn process(msg: u32) -> Vec<(u32)> {
        println!("{}", msg);
        if msg < 20 {
            vec![msg + 1]
        } else {
            std::process::exit(0);
        }
    }
    client.start_client("client", process);
}