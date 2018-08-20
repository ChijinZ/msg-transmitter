//! A example to show how to combine client and server into one task.
//!
//! 'master' is a server to control 'manager' when to stop (10s after starting
//! in our example).
//!
//!'manager' run a big task including two small tasks:
//! one for connecting to 'master' socket, when receiving a *stop* message, process exits;
//! one for listening another socket as server, when receiving 'master' *stop*, send stop
//! message (*0* in our example) to its client.
//!
//! 'driver' is a client to connect 'manager', and as a counter to send&receive u32.
//!
//! You can test this out by running:
//!
//!     cargo run --example master_and_slave  master
//!
//! And then fastly (within 10s) in another two windows run:
//!
//!     cargo run --example master_and_slave  manager
//!
//!     cargo run --example master_and_slave  slave
//!

extern crate msg_transmitter;
extern crate tokio;
extern crate futures;
#[macro_use]
extern crate serde_derive;
extern crate bincode;

use tokio::prelude::*;
use msg_transmitter::*;

use std::env;

#[derive(Serialize, Deserialize, Debug, Clone)]
enum Message {
    Start(),
    Stop(),
}


fn main() {
    let a = env::args().skip(1).collect::<Vec<_>>();
    match a.first().unwrap().as_str() {
        "master" => master(),
        "manager" => manager(),
        "driver" => driver(),
        _ => panic!("failed"),
    };
}

fn master() {
    use std::{thread, time};
    let srv = create_tcp_server("127.0.0.1:7777", "master");
    let master_task = srv.start_server(Message::Start(), |client_name, msg: Message| {
        println!("master receive {:?} from {}", msg, client_name);
        println!("sleep");
        let ten_millis = time::Duration::from_millis(10000);
        thread::sleep(ten_millis);
        println!("awake");
        vec![(client_name, Message::Stop())]
    });
    tokio::run(master_task);
}

fn manager() {
    use futures::sync::mpsc;
    use std::sync::{Arc, Mutex};
    let future_task = future::lazy(|| {
        let srv = create_tcp_server("127.0.0.1:6666", "management_server");
        let clt = create_tcp_client("127.0.0.1:7777", "management_client");
        let connections = srv.connections.clone();

        let clt_task = clt.start_client(move |msg: Message| {
            println!("manager receive {:?} from master", msg);
            match msg {
                Message::Stop() => {
                    for (_, tx) in connections.lock().unwrap().iter_mut() {
                        (*tx).try_send(Some(0)).unwrap();
                    }
                    //std::process::exit(0);
                }
                _ =>{}
            }
            vec![msg]
        });
        tokio::spawn(clt_task);

        let srv_task = srv.start_server(1, |client_name, msg: u32| {
            println!("manager receive {} from {}", msg, client_name);
            vec![(client_name, msg + 1)]
        });
        tokio::spawn(srv_task);
        Ok(())
    }).map_err(|e: std::io::Error| println!("{:?}", e));
    tokio::run(future_task);
}

fn driver() {
    let clt = create_tcp_client("127.0.0.1:6666", "driver");
    let client_task = clt.start_client(|msg: u32| {
        println!("{}", msg);
        if msg != 0 {
            vec![msg + 1]
        } else {
            println!("oh god");
            std::process::exit(0);
        }
    });
    tokio::run(client_task);
}