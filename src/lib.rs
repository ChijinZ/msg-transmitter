//! # msg-transmitter
//!
//! ## Overview
//! It is a library of single server multiple clients model. The main purpose of this library
//! is helping users more focus on communication logic instead of low-level networking design.
//! User can transmit any structs between server and client.
//!
//! User is able to choose either tcp-based or uds-based connection. Note that tcp-based connection
//! can support both Windows and *nux, but uds-based connection only can support *nux.
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
//! Design can be found [here](https://github.com/ChijinZ/msg-transmitter/blob/dev/readme.md)
//!
//! This crate is created by ChijinZ(tlock.chijin@gmail.com).
// #![feature(nll)]
//#![deny(warnings, missing_debug_implementations)]


extern crate tokio;
extern crate futures;
extern crate tokio_codec;
extern crate serde;
extern crate bincode;
extern crate bytes;

use tokio::net;
use tokio::prelude::*;
use bincode::{deserialize, serialize};
use tokio::io;
use tokio_codec::*;
use bytes::{BufMut, BytesMut};
use std::marker::PhantomData;
use std::sync::{Arc, Mutex, RwLock};
use std::cell::RefCell;
use std::collections::HashMap;
use futures::sync::mpsc;

// The number of bytes to represent data size.
const DATA_SIZE: usize = 4;

// This struct is to build a tokio framed to encode messages to bytes and decode bytes to messages.
// 'T' represents user-defined message type.
#[derive(Debug)]
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

//T is user message type
//F is the closure of control logic
//U is to abstract tcp and uds
fn start_server<T, F, U>(incoming: U, first_msg: T,
                         process_function: F,
                         server_name: String,
                         connections_outer: Arc<Mutex<HashMap<String, mpsc::Sender<Option<T>>>>>)
                         -> Box<Future<Item=(), Error=()> + Send + 'static>
    where T: serde::de::DeserializeOwned + serde::Serialize + Send + 'static + Clone,
          F: FnMut(String, T) -> Vec<(String, T)> + Send + Sync + 'static + Clone,
          U: Stream + Send + Sync + 'static,
          <U as futures::Stream>::Item: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Sync,
          <U as futures::Stream>::Error: std::fmt::Debug + 'static + Send + Sync
{
    Box::new(
        incoming
            .for_each(move |stream| {
                let process_function_outer = process_function.clone();
                let server_name = server_name.clone();
                let first_msg_inner = first_msg.clone();

                // Create a mpsc::channel in order to build a bridge between sender task and receiver
                // task.
                let (tx, rx): (mpsc::Sender<Option<T>>, mpsc::Receiver<Option<T>>) = mpsc::channel(0);
                let rx: Box<Stream<Item=Option<T>, Error=io::Error> + Send> = Box::new(rx.map_err(|_| panic!()));

                // Split tcp_stream to sink and stream. Sink responses to send messages to this
                // client, stream responses to receive messages from this client.
                let (sink, stream) = MessageCodec::new(server_name).framed(stream).split();

                // Spawn a sender task.
                let send_to_client = rx.forward(sink).then(|result| {
                    if let Err(e) = result {
                        println!("failed to write to socket: {}", e)
                    }
                    Ok(())
                });
                //tokio::spawn(send_to_client);

                // To record the client_name
                let client_name = Arc::new(RwLock::new(String::new()));
                // let client_name_ref = &mut client_name;
                // Spawn a receiver task.
                let receive_and_process =
                    {
                        let connections = connections_outer.clone();
                        let mut client_name = client_name.clone();
                        stream.for_each(move |(name, msg): (Option<String>, Option<T>)| {
                            let connections_inner = connections.clone();
                            match name {
                                // If it is a register information, register to connections.
                                Some(register_name) => {
                                    client_name.write().unwrap().push_str(&register_name);
                                    let mut tx_inner = tx.clone();
                                    tx_inner.try_send(Some(first_msg_inner.clone())).unwrap();
                                    connections_inner.lock().unwrap().insert(register_name, tx_inner);
                                }
                                // If it is a user's message, process it.
                                None => {
                                    let msg = msg.unwrap();
                                    let mut process_function_inner = process_function_outer.clone();
                                    let dest_and_msg = process_function_inner(client_name.read().unwrap().to_string(), msg);
                                    for (dest, msg) in dest_and_msg {
                                        if dest == "" {
                                            let mut tx_inner = tx.clone();
                                            tx_inner.try_send(Some(msg)).unwrap();
                                        } else {
                                            if connections_inner.lock().unwrap().contains_key(&dest) {
                                                connections_inner.lock().unwrap().get_mut(&dest).unwrap()
                                                    .try_send(Some(msg)).unwrap();
                                            } else {
                                                println!("{} doesn't register", dest);
                                            }
                                        }
                                    }
                                }
                            }
                            Ok(())
                        })
                    };
                let connections = connections_outer.clone();
                tokio::spawn(
                    send_to_client.select(receive_and_process)
                        .and_then(move |_| {
                            println!("{} disconnect",*client_name.read().unwrap());
                            connections.lock().unwrap().remove(&*client_name.read().unwrap());
                            Ok(())
                        }).map_err(|_| {})
                );
                Ok(())
            }).map_err(|e| { println!("{:?}", e); })
    )
}

//T is user message type
//F is the closure of control logic
//U is to abstract tcp and uds
fn start_client<T, F, U>(connect: U,
                         client_name: String,
                         mut process_function: F)
                         -> Box<Future<Item=(), Error=()> + Send + 'static>
    where T: serde::de::DeserializeOwned + serde::Serialize + Send + 'static + Clone,
          F: FnMut(T) -> Vec<T> + Send + Sync + 'static,
          U: Future + Send + Sync + 'static,
          <U as futures::Future>::Item: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Sync,
          <U as futures::Future>::Error: std::fmt::Debug + 'static + Send + Sync
{
    // Create a mpsc::channel in order to build a bridge between sender task and receiver
    // task.
    let (mut tx, rx): (mpsc::Sender<Option<T>>, mpsc::Receiver<Option<T>>) = mpsc::channel(0);
    let rx: Box<Stream<Item=Option<T>, Error=io::Error> + Send> = Box::new(rx.map_err(|_| panic!()));

    Box::new(
        connect.and_then(move |mut tcp_stream| {
            let mut message_codec: MessageCodec<T> = MessageCodec::new(client_name);

            // Send register information to server.
            let mut buf = BytesMut::new();
            let _ = message_codec.encode(None, &mut buf);
            let _ = tcp_stream.write_all(&buf);


            let (sink, stream) = message_codec.framed(tcp_stream).split();

            // Spawn a sender task.
            let send_to_server = rx.forward(sink).then(|result| {
                if let Err(e) = result {
                    println!("failed to write to socket: {}", e)
                }
                Ok(())
            });
            // tokio::spawn(send_to_server);

            // Spawn a receiver task.
            let receive_and_process = stream.for_each(move |(name, msg): (Option<String>, Option<T>)| {
                match name {
                    Some(_) => {
                        println!("client received unexpected message");
                    }
                    None => {
                        let msg = msg.unwrap();
                        let msgs = process_function(msg);
                        for msg in msgs {
                            tx.try_send(Some(msg)).unwrap();
                        }
                    }
                }
                Ok(())
            });
            tokio::spawn(
                send_to_server.select(receive_and_process)
                    .and_then(|_| {
                        println!("server closed");
                        Ok(())
                    }).map_err(|_| {})
            );
            Ok(())
        }).map_err(|e| { println!("faild to connect. err:{:?}", e); })
    )
}

#[allow(dead_code)]
pub mod tcp;

#[allow(dead_code)]
pub mod uds;

pub fn create_tcp_server<T>(addr: &str, server_name: &str) -> tcp::TCPMsgServer<T>
    where T: serde::de::DeserializeOwned + serde::Serialize + Send + 'static + Clone
{
    tcp::TCPMsgServer::new(addr, server_name)
}

pub fn create_tcp_client<T>(addr: &str, client_name: &str) -> tcp::TCPMsgClient<T>
    where T: serde::de::DeserializeOwned + serde::Serialize + Send + 'static + Clone
{
    tcp::TCPMsgClient::new(addr, client_name)
}

pub fn create_uds_server<T>(addr: &str, server_name: &str) -> uds::UDSMsgServer<T>
    where T: serde::de::DeserializeOwned + serde::Serialize + Send + 'static + Clone
{
    uds::UDSMsgServer::new(addr, server_name)
}

pub fn create_uds_client<T>(addr: &str, client_name: &str) -> uds::UDSMsgClient<T>
    where T: serde::de::DeserializeOwned + serde::Serialize + Send + 'static + Clone
{
    uds::UDSMsgClient::new(addr, client_name)
}