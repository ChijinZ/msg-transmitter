//! # msg-transmitter
//! ## Overview
//! It is a library of single server multiple clients model. The main purpose of this library
//! is helping users more focus on communication logic instead of low-level networking design.
//! User can transmit any structs between server and client.
//!
//! ## dependances
//! - Main networking architecture impletmented by asynchronous
//! framework [tokio](https://github.com/tokio-rs/tokio) and
//! [futures](https://github.com/rust-lang-nursery/futures-rs).
//!
//! - User data are transfered to bytes by serialization framework
//! [serde](https://github.com/serde-rs/serde) and binary encoder/decoder
//! crate [bincode](https://github.com/TyOverby/bincode).
//!
//! ## example
//! Examples can be found [here](https://github.com/ChijinZ/msg-transmitter/tree/master/examples).
//!
//! ## Design
//! For server, when *start_server* is called, it will start binding and listening the target port
//! and spawning two tokio-style tasks for each socket. One task for receiving message from socket
//! and processing user's logic, another task for sending message to socket. After the first task
//! processes user's logic, it will send message to another task through *mpsc::channel*.
//!
//! For client, when *start_client* is called, it will start connecting the target port and creating
//! a socket, sending register information (a name presented by *String*), spawning two tokio-style
//! tasks for each socket. Two tasks work with each other by the same way as server tasks do.
//! For now, all our networking is based on TCP. All user data are transferred to bytes through a
//! simple protocol.
//!
//!    +++++++++++++++++++++++++++++++++++++++++++++++
//!     Data_size(4 bytes) | State(1 byte) |   Data
//!    +++++++++++++++++++++++++++++++++++++++++++++++
//!
//! Data_size is a 4-bytes head to represent the number of bytes of **state** and **Data**. So each
//! stream can't transmit message which contains more than 2^32 bytes data.
//!
//! State represents the state of this stream, it just has two states for now. When state equals
//! **0**, the data is register informaiton; when state equals **1**, the data is user's message.
//!
//! This crate is created by ChijinZ(tlock.chijin@gmail.com).

#![deny(warnings, missing_debug_implementations)]

extern crate tokio;
extern crate futures;
extern crate tokio_codec;
extern crate serde;
extern crate bincode;
extern crate bytes;

use bincode::{deserialize, serialize};

use tokio::prelude::*;
use tokio::io;
use tokio::net;
use tokio_codec::*;
use bytes::{BufMut, BytesMut};
use futures::sync::mpsc;

use std::marker::PhantomData;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use std::net::SocketAddr;

// The number of bytes to represent data size.
const DATA_SIZE: usize = 4;

// This struct is to build a tokio framed to encode messages to bytes and decode bytes to messages.
// 'T' represents user-defined message type.
struct MessageCodec<T> {
    name: String,
    phantom: PhantomData<T>,
}

impl<T> MessageCodec<T> {
    pub fn new(name: String) -> MessageCodec<T> {
        MessageCodec { name: name, phantom: PhantomData }
    }
}

// A u64 to Vec<u8> function to convert decimal to 256 hexadecimal.
pub fn number_to_four_vecu8(num: u64) -> Vec<u8> {
    assert!(num < (1 << 32));
    let mut result: Vec<u8> = vec![];
    let mut x = num;
    loop {
        if x / 256 > 0 {
            result.push((x % 256) as u8);
            x = x / 256;
        } else {
            result.push((x % 256) as u8);
            break;
        }
    }
    for _ in 0..(DATA_SIZE - result.len()) {
        result.push(0);
    }
    result.reverse();
    return result;
}

// A Vec<u8> to u64 function to convert 256 hexadecimal to decimal.
pub fn four_vecu8_to_number(vec: Vec<u8>) -> u64 {
    assert_eq!(vec.len(), DATA_SIZE);
    let num = vec[0] as u64 * 256 * 256 * 256 + vec[1] as u64 * 256 * 256
        + vec[2] as u64 * 256 + vec[3] as u64;
    return num;
}

impl<T> Encoder for MessageCodec<T> where T: serde::Serialize {
    type Item = Option<T>;
    type Error = io::Error;
    fn encode(&mut self, item: Self::Item, dst: &mut BytesMut) -> Result<(), Self::Error> {
        let mut data: Vec<u8> = vec![];
        match item {
            // If None, it is a register information, so state is 0 and data is register
            // information (i.e. name).
            None => {
                data.push(0 as u8);
                let mut name = self.name.clone().into_bytes();
                data.append(&mut name);
            }
            // If not None, it is user's message, so state is 1.
            Some(v) => {
                data.push(1 as u8);
                data.append(&mut serialize(&v).unwrap());
            }
        }
        let mut encoder: Vec<u8> = number_to_four_vecu8(data.len() as u64);
        encoder.append(&mut data);
        dst.reserve(encoder.len());
        dst.put(encoder);
        Ok(())
    }
}

impl<T> Decoder for MessageCodec<T> where T: serde::de::DeserializeOwned {
    type Item = (Option<String>, Option<T>);
    type Error = io::Error;
    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if src.len() < DATA_SIZE {
            Ok(None)
        } else {
            let mut vec: Vec<u8> = src.to_vec();
            let mut truth_data = vec.split_off(DATA_SIZE);
            let vec_length = four_vecu8_to_number(vec);
            if truth_data.len() == vec_length as usize {
                let msg_data = truth_data.split_off(1);
                src.clear();
                match truth_data[0] {
                    // Deserialize it is register information or user's message.
                    0 => {
                        Ok(Some((Some(String::from_utf8(msg_data).unwrap()), None)))
                    }
                    1 => {
                        let msg: T = deserialize(&msg_data).unwrap();
                        Ok(Some((None, Some(msg))))
                    }
                    _ => {
                        panic!("unexpected message");
                    }
                }
            } else {
                Ok(None)
            }
        }
    }
}

#[derive(Debug)]
pub struct MsgServer<T> {
    // addr is the socket address which server will bind and listen to.
    // connetions is used to map client's name to sender of channel.
    addr: SocketAddr,
    name: String,
    connections: Arc<Mutex<HashMap<String, mpsc::Sender<Option<T>>>>>,
}

impl<T> MsgServer<T>
    where T: serde::de::DeserializeOwned + serde::Serialize + Send + 'static + Clone
{
    pub fn new(addr: &str, server_name: &'static str) -> MsgServer<T> {
        let socket_addr = addr.parse::<SocketAddr>().unwrap();
        MsgServer {
            addr: socket_addr,
            name: String::from(server_name),
            connections: Arc::new(Mutex::new(HashMap::new())),
        }
    }
    pub fn start_server(&self, first_msg: T,
                        process_function: fn(T) -> Vec<(String, T)>)
    {
        let connections_outer = self.connections.clone();
        let listener = net::TcpListener::bind(&self.addr)
            .expect("unable to bind TCP listener");
        let server_name = self.name.clone();
        let done = listener.incoming()
            .for_each(move |tcp_stream| {
                let server_name = server_name.clone();
                let first_msg_inner = first_msg.clone();
                let connections = connections_outer.clone();

                // Create a mpsc::channel in order to build a bridge between sender task and receiver
                // task.
                let (tx, rx): (mpsc::Sender<Option<T>>, mpsc::Receiver<Option<T>>) = mpsc::channel(0);
                let rx: Box<Stream<Item=Option<T>, Error=io::Error> + Send> = Box::new(rx.map_err(|_| panic!()));

                // Split tcp_stream to sink and stream. Sink responses to send messages to this
                // client, stream responses to receive messages from this client.
                let (sink, stream) = MessageCodec::new(server_name).framed(tcp_stream).split();

                // Spawn a sender task.
                let send_to_client = rx.forward(sink).then(|result| {
                    if let Err(e) = result {
                        panic!("failed to write to socket: {}", e)
                    }
                    Ok(())
                });
                tokio::spawn(send_to_client);

                // Spawn a receiver task.
                let receive_and_process = stream.for_each(move |(name, msg): (Option<String>, Option<T>)| {
                    let connections_inner = connections.clone();
                    match name {
                        // If it is a register information, register to connections.
                        Some(register_name) => {
                            let mut tx_inner = tx.clone();
                            tx_inner.try_send(Some(first_msg_inner.clone())).unwrap();
                            connections_inner.lock().unwrap().insert(register_name, tx_inner);
                        }
                        // If it is a user's message, process it.
                        None => {
                            let msg = msg.unwrap();
                            let dest_and_msg = process_function(msg);
                            for (dest, msg) in dest_and_msg {
                                if connections_inner.lock().unwrap().contains_key(&dest) {
                                    connections_inner.lock().unwrap().get_mut(&dest).unwrap()
                                        .try_send(Some(msg)).unwrap();
                                } else {
                                    println!("{} doesn't register", dest);
                                }
                            }
                        }
                    }
                    Ok(())
                }).map_err(move |_| {
                    println!("closed connection");
                    //TODO remove the key and value of this socket from self.connections
                });

                tokio::spawn(receive_and_process);
                Ok(())
            }).map_err(|e| { println!("{:?}", e); });
        tokio::run(done);
    }
}

#[derive(Debug)]
pub struct MsgClient<T> {
    // addr is the socket address which client will connect to.
    // phantom is just used to avoid compile error.
    connect_addr: SocketAddr,
    name: String,
    phantom: PhantomData<T>,
}

impl<T> MsgClient<T>
    where T: serde::de::DeserializeOwned + serde::Serialize + Send + 'static + Clone
{
    pub fn new(addr: &str, client_name: &'static str) -> MsgClient<T> {
        let socket_addr = addr.parse::<SocketAddr>().unwrap();
        MsgClient {
            connect_addr: socket_addr,
            name: String::from(client_name),
            phantom: PhantomData,
        }
    }

    pub fn start_client<F>(&self, mut process_function: F)
        where F: FnMut(T) -> Vec<T> + Send + Sync + 'static
    {
        let client_name = self.name.clone();
        let tcp = net::TcpStream::connect(&self.connect_addr);
        let done = tcp.map(move |mut tcp_stream| {
            let mut message_codec: MessageCodec<T> = MessageCodec::new(client_name);

            // Send register information to server.
            let register_msg = None;
            let mut buf = BytesMut::new();
            let _ = message_codec.encode(register_msg, &mut buf);
            let _ = tcp_stream.write_all(&buf);

            // Create a mpsc::channel in order to build a bridge between sender task and receiver
            // task.
            let (mut tx, rx): (mpsc::Sender<Option<T>>, mpsc::Receiver<Option<T>>) = mpsc::channel(0);
            let rx: Box<Stream<Item=Option<T>, Error=io::Error> + Send> = Box::new(rx.map_err(|_| panic!()));
            let (sink, stream) = message_codec.framed(tcp_stream).split();

            // Spawn a sender task.
            let send_to_server = rx.forward(sink).then(|result| {
                if let Err(e) = result {
                    panic!("failed to write to socket: {}", e)
                }
                Ok(())
            });
            tokio::spawn(send_to_server);

            // Spawn a receiver task.
            let receive_and_process = stream.for_each(move |(name, msg): (Option<String>, Option<T>)| {
                match name {
                    Some(_) => {
                        panic!("client received unexpected message");
                    }
                    None => {
                        let msg = msg.unwrap();
                        let msgs = process_function(msg);
                        //let msgs = process_function(msg);
                        for msg in msgs {
                            tx.try_send(Some(msg)).unwrap();
                        }
                    }
                }
                Ok(())
            }).map_err(move |_| { println!("server closed"); });
            tokio::spawn(receive_and_process);
        }).map_err(|e| { println!("{:?}", e); });
        tokio::run(done);
    }
}