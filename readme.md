# msg-transmitter
[![Build Status](https://travis-ci.org/ChijinZ/msg-transmitter.svg?branch=master)](https://travis-ci.org/ChijinZ/msg-transmitter)
## Overview
It is a library of single server multiple clients model. The main purpose of this library is helping users more focus on communication logic instead of low-level networking design. User can transmit any structs between server and client.

## dependances
- Main networking architecture impletmented by asynchronous framework [tokio](https://github.com/tokio-rs/tokio) and [futures](https://github.com/rust-lang-nursery/futures-rs).
- User data are transfered to bytes by serialization framework [serde](https://github.com/serde-rs/serde) and binary encoder/decoder crate [bincode](https://github.com/TyOverby/bincode).

## Usage

## example
A basic u32-transmitting example:
```rust
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
```
More examples can be found [here](https://github.com/ChijinZ/msg-transmitter/tree/master/examples).
## Design
For server, when *start_server* is called, it will start binding and listening the target port and spawning two tokio-style tasks for each socket. One task for receiving message from socket and processing user's logic, another task for sending message to socket. After the first task processes user's logic, it will send message to another task through *mpsc::channel*.

For client, when *start_client* is called, it will start connecting the target port and creating a socket, sending register information (a name presented by *String*), spawning two tokio-style tasks for each socket. Two tasks work with each other by the same way as server tasks do.
For now, all our networking is based on TCP. All user data are transfered to bytes through a simple protocol. 

    +++++++++++++++++++++++++++++++++++++++++++++++
     Data_size(4 bytes) | State(1 byte) |   Data
    +++++++++++++++++++++++++++++++++++++++++++++++

Data_size is a 4-bytes head to represent the number of bytes of **state** and **Data**. So each stream can't transmit message which contains more than 2^32 bytes data.

State represents the state of this stream, it just has two states for now. When state equals **0**, the data is register informaiton; when state equals **1**, the data is user's message.