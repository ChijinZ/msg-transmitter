extern crate tokio_uds;
extern crate tokio;
extern crate futures;
extern crate tokio_codec;
extern crate serde;
extern crate bincode;
extern crate bytes;

use super::*;
use std::path::Path;
use self::tokio_uds::{UnixStream, UnixListener};

#[derive(Debug)]
pub struct UDSMsgServer<T> {
    // path is the Path bounded by UnixListener.
    // connetions is used to map client's name to sender of channel.
    pub path_name: String,
    pub name: String,
    pub connections: Arc<Mutex<HashMap<String, mpsc::Sender<Option<T>>>>>,
}

impl<T> UDSMsgServer<T>
    where T: serde::de::DeserializeOwned + serde::Serialize + Send + 'static + Clone
{
    /// *addr* is socket address. like: 127.0.0.1:6666.
    /// *name* is the server's name, to identity which server it is.
    pub fn new(path_name: &str, server_name: &str) -> UDSMsgServer<T> {
        UDSMsgServer {
            path_name: String::from(path_name),
            name: String::from(server_name),
            connections: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// *first_msg* is the first message that server send to the client which just
    /// connect to server.
    /// *process_fuction* receive a tuple of <client_name, message>, and return
    /// a series of tuple of (client_name,message) indicating which message will be
    /// sent to which client. Note that if you want to send a message to current
    /// client, you should set client_name as a string with 0 length, i.e. ""
    pub fn start_server<F>(&self, first_msg: T,
                           process_function: F)
                           -> Box<Future<Item=(), Error=()> + Send + 'static>
        where F: FnMut(String, T) -> Vec<(String, T)> + Send + Sync + 'static + Clone
    {
        let path = Path::new(&self.path_name);
        let listener = UnixListener::bind(path)
            .expect("unable to bind Unix listener");
        start_server(listener.incoming(),
                     first_msg,
                     process_function,
                     self.name.clone(),
                     self.connections.clone())
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
    /// *addr* is socket address. like: 127.0.0.1:6666.
    /// *name* is the client's name, to identity which client it is.
    pub fn new(path_name: &str, client_name: &str) -> UDSMsgClient<T> {
        UDSMsgClient {
            path_name: String::from(path_name),
            name: String::from(client_name),
            phantom: PhantomData,
        }
    }

    /// process_function receive a message from server and send a series
    /// of messages to server
    pub fn start_client<F>(&self, process_function: F) -> Box<Future<Item=(), Error=()> + Send + 'static>
        where F: FnMut(T) -> Vec<T> + Send + Sync + 'static
    {
        start_client(UnixStream::connect(Path::new(&self.path_name)),
                     self.name.clone(),
                     process_function)
    }
}