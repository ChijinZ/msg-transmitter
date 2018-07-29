extern crate tokio_uds;
extern crate tokio;
extern crate futures;
extern crate tokio_codec;
extern crate serde;
extern crate bincode;
extern crate bytes;


use self::tokio::prelude::*;
use self::tokio::io;
use self::tokio_codec::*;
use self::bytes::BytesMut;
use self::futures::sync::mpsc;


use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use std::marker::PhantomData;

use super::codec::*;
use std::path::Path;
use self::tokio_uds::{UnixStream,UnixListener};

#[derive(Debug)]
pub struct UDSMsgServer<T> {
    // path is the Path bounded by UnixListener.
    // connetions is used to map client's name to sender of channel.
    path_name: String,
    server_name: String,
    connections: Arc<Mutex<HashMap<String, mpsc::Sender<Option<T>>>>>,
}

impl<T> UDSMsgServer<T>
    where T: serde::de::DeserializeOwned + serde::Serialize + Send + 'static + Clone
{
    pub fn new(path_name: &str, server_name: &str) -> UDSMsgServer<T> {
        UDSMsgServer {
            path_name: String::from(path_name),
            server_name: String::from(server_name),
            connections: Arc::new(Mutex::new(HashMap::new())),
        }
    }
    pub fn start_server(&self, first_msg: T,
                        process_function: fn(T) -> Vec<(String, T)>)
    {
        let path = Path::new(&self.path_name);
        let connections_outer = self.connections.clone();
        let listener = UnixListener::bind(path)
            .expect("unable to bind Unix listener");
        let server_name = self.server_name.clone();
        let done = listener.incoming()
            .for_each(move |unix_stream| {
                let server_name = server_name.clone();
                let first_msg_inner = first_msg.clone();
                let connections = connections_outer.clone();

                // Create a mpsc::channel in order to build a bridge between sender task and receiver
                // task.
                let (tx, rx): (mpsc::Sender<Option<T>>, mpsc::Receiver<Option<T>>) = mpsc::channel(0);
                let rx: Box<Stream<Item=Option<T>, Error=io::Error> + Send> = Box::new(rx.map_err(|_| panic!()));

                // Split tcp_stream to sink and stream. Sink responses to send messages to this
                // client, stream responses to receive messages from this client.
                let (sink, stream) = MessageCodec::new(server_name).framed(unix_stream).split();

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
pub struct UDSMsgClient<T> {
    // path_name is the Path connected by client.
    // phantom is just used to avoid compile error.
    path_name: String,
    name: String,
    phantom: PhantomData<T>,
}

impl<T> UDSMsgClient<T>
    where T: serde::de::DeserializeOwned + serde::Serialize + Send + 'static + Clone
{
    pub fn new(path_name: &str, client_name: &str) -> UDSMsgClient<T> {
        UDSMsgClient {
            path_name: String::from(path_name),
            name: String::from(client_name),
            phantom: PhantomData,
        }
    }
    pub fn start_client<F>(&self, mut process_function: F)
        where F: FnMut(T) -> Vec<T> + Send + Sync + 'static
    {
        let path = Path::new(&self.path_name);
        let client_name = self.name.clone();
        let uds = UnixStream::connect(path);
        let done = uds.map(move |mut unix_stream| {
            let mut message_codec: MessageCodec<T> = MessageCodec::new(client_name);

            // Send register information to server.
            let register_msg = None;
            let mut buf = BytesMut::new();
            let _ = message_codec.encode(register_msg, &mut buf);
            let _ = unix_stream.write_all(&buf);

            // Create a mpsc::channel in order to build a bridge between sender task and receiver
            // task.
            let (mut tx, rx): (mpsc::Sender<Option<T>>, mpsc::Receiver<Option<T>>) = mpsc::channel(0);
            let rx: Box<Stream<Item=Option<T>, Error=io::Error> + Send> = Box::new(rx.map_err(|_| panic!()));
            let (sink, stream) = message_codec.framed(unix_stream).split();

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