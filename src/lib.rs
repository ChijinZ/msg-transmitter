//#![doc(html_root_url = "11")]
//#![deny(missing_docs, warnings, missing_debug_implementations)]

extern crate tokio;
extern crate futures;
extern crate tokio_codec;
extern crate serde;
//#[macro_use]
//extern crate serde_derive;
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

const MAX_CODEC_SYMBOL_SIZE: usize = 4;

struct MessageCodec<T> {
    name: String,
    phantom: PhantomData<T>,
}

impl<T> MessageCodec<T> {
    pub fn new(name: &str) -> MessageCodec<T> {
        let name_string = name.to_string();
        MessageCodec { name: name_string, phantom: PhantomData }
    }
}

fn number_to_four_vecu8(num: u64) -> Vec<u8> {
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
    for _ in 0..(4 - result.len()) {
        result.push(0);
    }
    result.reverse();
    return result;
}


fn four_vecu8_to_number(vec: Vec<u8>) -> u64 {
    assert_eq!(vec.len(), 4);
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
            None => {
                data.push(0 as u8);
                let mut name = self.name.clone().into_bytes();
                data.append(&mut name);
            }
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
        if src.len() < MAX_CODEC_SYMBOL_SIZE {
            Ok(None)
        } else {
            let mut vec: Vec<u8> = src.to_vec();
            let mut truth_data = vec.split_off(MAX_CODEC_SYMBOL_SIZE);
            let vec_length = four_vecu8_to_number(vec);
            if truth_data.len() == vec_length as usize {
                let msg_data = truth_data.split_off(1);
                src.clear();
                match truth_data[0] {
                    0 => {
                        Ok(Some((Some(String::from_utf8(truth_data).unwrap()), None)))
                    }
                    1 => {
                        let msg: T = deserialize(&msg_data).unwrap();
                        Ok(Some((None, Some(msg))))
                    }
                    _ => {
                        panic!("unexpected message");
                    }
                }
                // Ok(Some((String::from_utf8(truth_data).unwrap(), msg)))
            } else {
                Ok(None)
            }
        }
    }
}

pub struct MsgServer<T> {
    pub addr: SocketAddr,
    connections: Arc<Mutex<HashMap<String, mpsc::Sender<Option<T>>>>>,
}

impl<T> MsgServer<T>
    where T: serde::de::DeserializeOwned + serde::Serialize + Send + 'static
{
    pub fn new(addr: &str) -> MsgServer<T> {
        let socket_addr = addr.parse::<SocketAddr>().unwrap();
        MsgServer {
            addr: socket_addr,
            connections: Arc::new(Mutex::new(HashMap::new())),
        }
    }
    pub fn start_server(&self, name: &'static str, process_function: fn(T) -> Vec<(String, T)>)
    {
        let connections_outer = self.connections.clone();
        let listener = net::TcpListener::bind(&self.addr).unwrap();
        let done = listener.incoming().for_each(move |tcp_stream| {
            let connections = connections_outer.clone();
            let (mut tx, rx): (mpsc::Sender<Option<T>>, mpsc::Receiver<Option<T>>) = mpsc::channel(0);
            let rx = rx.map_err(|_| panic!());
            let rx: Box<Stream<Item=Option<T>, Error=io::Error> + Send> = Box::new(rx);
            let (sink, stream) = MessageCodec::new(name).framed(tcp_stream).split();
            let send_to_client = rx.forward(sink).then(|result| {
                if let Err(e) = result {
                    panic!("failed to write to socket: {}", e)
                }
                Ok(())
            });
            tokio::spawn(send_to_client);
            let receive_and_process = stream.for_each(move |(name, msg): (Option<String>, Option<T>)| {
                // process
//                let dest_and_msg = process_function(msg);
//                for (dest, msg) in dest_and_msg {
                //}

                Ok(())
            }).map_err(move |e| { println!("{} closed connection", name); });

            tokio::spawn(receive_and_process);
            Ok(())
        }).map_err(|e| { println!("{:?}", e); });
        tokio::run(done);
    }
}