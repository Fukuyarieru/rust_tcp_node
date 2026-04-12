use std::{
    io::{BufRead, BufReader, BufWriter, Result, Write},
    net::{TcpListener, TcpStream},
    sync::{
        Arc, Mutex,
        mpsc::{Receiver, Sender, TryRecvError, channel},
    },
    thread::{JoinHandle, spawn},
};

use serde::{Deserialize, Serialize};

pub struct Client {
    pub connections: Arc<Mutex<Vec<Connection>>>,
    tcp_listener: TcpListener,
    /// connections send messages here
    pub receiver_of_messages_from_connections: Receiver<Message>,
    /// sender given to each conenction to send the client new messages
    sender_to_client: Sender<Message>,
    /// vec storing each sender of every connection
    senders_to_connections: Arc<Mutex<Vec<Sender<Message>>>>,
    /// thread accepting new connections
    new_connections_handle: Option<JoinHandle<()>>,
    /// sender to send messages to connections
    pub sender_to_connections: Sender<Message>,
    /// thread sending messages of the client to all connections
    sender_to_connections_handle: Option<JoinHandle<()>>,
}

impl Client {
    /// method to create a new client.
    /// said client needs to be "started" to accept connections
    pub fn new() -> Self {
        let (sender, receiver) = channel();
        let (_replace_sender, _) = channel();
        Self {
            connections: Arc::new(Mutex::new(Vec::new())),
            tcp_listener: TcpListener::bind("127.0.0.1:8080").unwrap(),
            receiver_of_messages_from_connections: receiver,
            sender_to_client: sender,
            senders_to_connections: Arc::new(Mutex::new(Vec::new())),
            new_connections_handle: None,
            sender_to_connections: _replace_sender,
            sender_to_connections_handle: None,
        }
    }

    pub fn connect(&mut self, address: &str) -> Result<()> {
        let stream = TcpStream::connect(address)?;
        let (s, r) = channel();
        let c = Connection::new(
            stream,
            self.sender_to_client.clone(),
            // TODO: NEED TO FIND A WAY FOR RECEIVER
            r,
        );
        self.senders_to_connections.lock().unwrap().push(s);
        self.connections.lock().unwrap().push(c);
        Ok(())
    }

    // /// Messages which are `d from connections will be sent over through this method
    // pub fn receive(&self) -> Result<Message, TryRecvError> {
    //     self.receiver_of_messages_from_connections.try_recv()
    // }

    /// Let the client to recieve messages
    /// This function is nonblocking
    /// It will start a seperate thread to accept incoming tcp connections
    /// and transform them into the 'Connection' made type and store them
    pub fn start_handling(&mut self) {
        let tcp_listener = self.tcp_listener.try_clone().unwrap();
        let connections = self.connections.clone();
        let sender_to_client = self.sender_to_client.clone();
        let senders_to_connections = self.senders_to_connections.clone();
        self.new_connections_handle = Some(spawn(move || {
            for stream in tcp_listener.incoming() {
                let (s, r) = channel::<Message>();
                let c = Connection::new(stream.unwrap(), sender_to_client.clone(), r);
                #[cfg(debug_assertions)]
                println!("CONNECTION -> {:?}", c);
                connections.lock().unwrap().push(c);
                senders_to_connections.lock().unwrap().push(s);
            }
        }));
        let senders_to_connections = self.senders_to_connections.clone();
        let (main_sender_to_connections, receiver_of_sender_to_connections) = channel::<Message>();
        self.sender_to_connections = main_sender_to_connections;
        self.sender_to_connections_handle = Some(spawn(move || {
            while let Ok(message) = receiver_of_sender_to_connections.recv() {
                for sender_to_connection in senders_to_connections.lock().unwrap().iter() {
                    sender_to_connection.send(message.clone()).unwrap();
                }
            }
        }));
    }
}
// // private function used by "start_handling"
// fn handle_tcp_stream(
//     stream: TcpStream,
//     // own client may recieve message, these are sent to a receiver on the client through this sender the Connection got
//     sender_to_client: Sender<Message>,
//     // own client may send messages, which are recieved by the Connection through this receiver
//     receiver_of_connection: Receiver<Message>,
// ) -> Connection {
//     // let sender_clone = sender_to_client.clone();
//     let incoming_address = stream.peer_addr().unwrap().to_string();
//     let reader = BufReader::new(stream.try_clone().unwrap());
//     let mut writer = BufWriter::new(stream);
//     Connection {
//         source: incoming_address,
//         read_handle: spawn(move || {
//             for line in reader.lines() {
//                 let json = line.expect("Client Disconnected");
//                 let message = serde_json::from_str::<Message>(&json).unwrap();
//                 // sender_clone.send(message).unwrap();
//                 sender_to_client.send(message).unwrap();
//             }
//         }),
//         write_handle: spawn(move || {
//             while let Ok(message) = receiver_of_connection.recv() {
//                 let message_json = serde_json::to_string(&message).unwrap() + "\n";
//                 writer.write_all(message_json.as_bytes()).unwrap();
//                 writer.flush().unwrap();
//             }
//         }),
//         // sender_to_client,
//     }
// }

#[derive(Debug)]
pub struct Connection {
    /// source of the client connected
    source: String,
    /// thread handling the connected client
    read_handle: JoinHandle<()>,
    /// thread handling sending messages to a connection
    write_handle: JoinHandle<()>,
    // /// client recieves message through a receiver, which is appropriatly connected to this sender that can be cloned
}

impl Connection {
    /// This is the method to convert tcp stream into a "Connection"
    /// interactions with the Connection are made using a channel() made before calling this method (sender,receiver)
    /// we give this function the receiver and handle the sender to be however we want to send messages to this Connection
    pub fn new(
        stream: TcpStream,
        // own client may recieve message, these are sent to a receiver on the client through this sender the Connection got
        sender_to_client: Sender<Message>,
        // own client may send messages, which are recieved by the Connection through this receiver
        receiver_of_connection: Receiver<Message>,
    ) -> Connection {
        let incoming_address = stream.peer_addr().unwrap().to_string();
        let reader = BufReader::new(stream.try_clone().unwrap());
        let mut writer = BufWriter::new(stream);
        Connection {
            source: incoming_address,
            read_handle: spawn(move || {
                for line in reader.lines() {
                    let json = line.expect("Client Disconnected");
                    let message = serde_json::from_str::<Message>(&json).unwrap();
                    // sender_clone.send(message).unwrap();
                    sender_to_client.send(message).unwrap();
                }
            }),
            write_handle: spawn(move || {
                while let Ok(message) = receiver_of_connection.recv() {
                    let message_json = serde_json::to_string(&message).unwrap() + "\n";
                    writer.write_all(message_json.as_bytes()).unwrap();
                    writer.flush().unwrap();
                }
            }),
        }
    }

    pub fn source(&self) -> String {
        self.source.clone()
    }
}

fn read_handler(reader: BufReader<TcpStream>, sender_to_client: Sender<Message>) -> JoinHandle<()> {
    spawn(move || {
        for line in reader.lines() {
            let json = line.expect("Client Disconnected");
            let message = serde_json::from_str::<Message>(&json).unwrap();
            // sender_clone.send(message).unwrap();
            sender_to_client.send(message).unwrap();
        }
    })
}

///  Example:
/// * Command - starts with /
/// * Text - anything that isn't a command
/// ---
/// Can be whatever you want, handle this as you may please

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Message {
    Command(String),
    Text(String),
}

// transmitter
// receiver
