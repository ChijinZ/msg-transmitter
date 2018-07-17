extern crate msg_transmitter;
#[macro_use]
extern crate serde_derive;
extern crate bincode;
// use bincode::{deserialize, serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
enum Message {
    processMsg(Process),
    seedMsg(Seed),
    stateMsg(State),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Process {}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Seed {
    x: i32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct State {
    y: Vec<f32>,
}

#[test]
fn test1() {
    let a: msg_transmitter::MsgServer<Message> = msg_transmitter::MsgServer::new("127.0.0.1:6666");

}