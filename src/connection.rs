use std::{
    io::{BufRead, BufReader, BufWriter, Write},
    net::TcpStream,
    sync::{
        Arc, Mutex,
        mpsc::{Receiver, Sender},
    },
    thread::{JoinHandle, spawn},
};

use serde_json::Value;

#[derive(Debug)]
pub struct Connection {
    /// source of the client connected
    source: String,
    /// thread handling the connected client
    _read_handle: JoinHandle<()>,
    /// thread handling sending messages to a connection
    _write_handle: JoinHandle<()>,
    /// bake inside the Connection its' handling,
    /// local method stored with the connection,
    /// applies on incoming messages from the `_read_handle`,
    connection_handle_method: Arc<Mutex<Option<fn(Value)>>>,
    ///
    sender_to_connection: Sender<Value>,
    ///
    receiver_of_connection: Arc<Mutex<Receiver<Value>>>,
}

impl Connection {
    /// This is the method to convert tcp stream into a "Connection"\
    /// interactions with the Connection are made using a channel() made before calling this method (sender,receiver)\
    /// we give this function the receiver and handle the sender to be however we want to send messages to this Connection
    pub fn new(
        stream: TcpStream,
        // own client may recieve message, these are sent to a receiver on the client through this sender the Connection got
        output_sender: Sender<Value>,
        //
        input_sender: Sender<Value>,
        // own client may send messages, which are recieved by the Connection through this receiver
        input_receiver: Receiver<Value>,
        // local method stored with the connection,
        // applies on incoming messages from the `_read_handle`,
        handle_method: Option<fn(Value)>,
    ) -> Self {
        let incoming_address = stream.peer_addr().unwrap().to_string();
        let reader = BufReader::new(stream.try_clone().unwrap());
        let mut writer = BufWriter::new(stream);
        let handling_method = Arc::new(Mutex::new(handle_method));
        let handling_method_clone = handling_method.clone();
        let receiver_of_connection = Arc::new(Mutex::new(input_receiver));
        let receiver_of_connection_clone = receiver_of_connection.clone();
        Self {
            source: incoming_address,
            _read_handle: spawn(move || {
                for line in reader.lines() {
                    let json = line.expect("Client Disconnected");
                    let message = serde_json::from_str::<Value>(&json).unwrap();
                    if let Some(method) = *handling_method_clone.lock().unwrap() {
                        #[cfg(debug_assertions)]
                        println!("{:?}", method);
                        method(message.clone())
                    }
                    output_sender.send(message).unwrap();
                }
            }),
            _write_handle: spawn(move || {
                // any warning or problem?
                while let Ok(message) = receiver_of_connection_clone.lock().unwrap().recv() {
                    let message_json = serde_json::to_string(&message).unwrap() + "\n";
                    writer.write_all(message_json.as_bytes()).unwrap();
                    writer.flush().unwrap();
                }
            }),
            connection_handle_method: handling_method,
            sender_to_connection: input_sender,
            receiver_of_connection: receiver_of_connection,
        }
    }

    pub fn sender_to_connection(&self) -> Sender<Value> {
        self.sender_to_connection.clone()
    }

    pub fn receiver_of_connection(&self) -> Arc<Mutex<Receiver<Value>>> {
        self.receiver_of_connection.clone()
    }

    pub fn change_method(&self, method: Option<fn(Value)>) {
        *self.connection_handle_method.lock().unwrap() = method;
    }

    #[allow(dead_code)]
    pub fn source(&self) -> String {
        self.source.clone()
    }
}
