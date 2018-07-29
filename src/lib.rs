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

#[allow(dead_code)]
mod codec {
    extern crate tokio;
    extern crate futures;
    extern crate tokio_codec;
    extern crate serde;
    extern crate bincode;
    extern crate bytes;

    use bincode::{deserialize, serialize};
    use tokio::io;
    use tokio_codec::*;
    use bytes::{BufMut, BytesMut};
    use std::marker::PhantomData;

    // The number of bytes to represent data size.
    const DATA_SIZE: usize = 4;

    // This struct is to build a tokio framed to encode messages to bytes and decode bytes to messages.
// 'T' represents user-defined message type.

    pub struct MessageCodec<T> {
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
}

trait Server<T> {
    fn start_server(&self, first_msg: T,
                        process_function: fn(T) -> Vec<(String, T)>);
}

trait Client<T> {

}

#[allow(dead_code)]
mod tcp;

#[allow(dead_code)]
mod uds;

