use serde_json::Value;
use std::{
    io::{BufRead, BufReader, BufWriter, Result, Write},
    net::{TcpListener, TcpStream},
    sync::{
        Arc, Mutex,
        mpsc::{Receiver, Sender, channel},
    },
    thread::{JoinHandle, spawn},
};

pub struct TcpNode {
    pub connections: Arc<Mutex<Vec<Connection>>>,
    tcp_listener: TcpListener,
    /// connections send messages here
    pub receiver_of_messages_from_connections: Receiver<Value>,
    /// sender given to each conenction to send the client new messages
    sender_to_client: Sender<Value>,
    /// vec storing each sender of every connection
    senders_to_connections: Arc<Mutex<Vec<Sender<Value>>>>,
    /// thread accepting new connections
    /// * The thread will be created only during the `start_handling` method
    new_connections_handle: Option<JoinHandle<()>>,
    /// * Sender to send messages to connections
    /// * May be None if `start_handling` hasn't been called yet
    pub sender_to_connections: Option<Sender<Value>>,
    /// thread sending messages of the client to all
    /// * May be None if `start_handling` hasn't been called yet
    sender_to_connections_handle: Option<JoinHandle<()>>,
}

impl TcpNode {
    /// * Method to create a new client.
    /// * Said client needs to be "started" to accept connections using `start_handling`
    pub fn new() -> Self {
        let (sender, receiver) = channel();
        let senders_to_connections = Arc::new(Mutex::new(Vec::new()));
        let tcp_listener = TcpListener::bind("127.0.0.1:0").unwrap();
        #[cfg(debug_assertions)]
        println!(
            "NODE ADDRESS -> {}",
            tcp_listener.local_addr().unwrap().to_string()
        );
        Self {
            connections: Arc::new(Mutex::new(Vec::new())),
            tcp_listener: tcp_listener,
            receiver_of_messages_from_connections: receiver,
            sender_to_client: sender,
            senders_to_connections: senders_to_connections,
            new_connections_handle: None,
            sender_to_connections: None,
            sender_to_connections_handle: None,
        }
    }

    /// Method to use for the ability to connect the client to an outside address,
    /// handling it as same "Connection" as all receiving connections
    pub fn connect(&mut self, address: &str) -> Result<()> {
        let stream = TcpStream::connect(address)?;
        let (s, r) = channel();
        let c = Connection::new(stream, self.sender_to_client.clone(), r);
        #[cfg(debug_assertions)]
        println!("CONNECTING -> {:?}", c);
        self.senders_to_connections.lock().unwrap().push(s);
        self.connections.lock().unwrap().push(c);
        Ok(())
    }

    /// Let the client to recieve messages\
    /// This function is nonblocking\
    /// It will start a seperate thread to accept incoming tcp connections\
    /// and transform them into the 'Connection' made type and store them
    pub fn start_handling(&mut self) {
        let tcp_listener = self.tcp_listener.try_clone().unwrap();
        let connections = self.connections.clone();
        let sender_to_client = self.sender_to_client.clone();
        let senders_to_connections = self.senders_to_connections.clone();
        self.new_connections_handle = Some(spawn(move || {
            for stream in tcp_listener.incoming() {
                let (s, r) = channel::<Value>();
                let c = Connection::new(stream.unwrap(), sender_to_client.clone(), r);
                #[cfg(debug_assertions)]
                println!("CONNECTION -> {:?}", c);
                connections.lock().unwrap().push(c);
                senders_to_connections.lock().unwrap().push(s);
            }
        }));
        let senders_to_connections_clone = self.senders_to_connections.clone();
        let (sender_to_connections, receiver_of_sender_to_connections) = channel::<Value>();
        self.sender_to_connections = Some(sender_to_connections);
        self.sender_to_connections_handle = Some(spawn(move || {
            while let Ok(message) = receiver_of_sender_to_connections.recv() {
                for sender_to_connection in senders_to_connections_clone.lock().unwrap().iter() {
                    sender_to_connection.send(message.clone()).unwrap();
                }
            }
        }));
    }
}

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
        sender_to_client: Sender<Value>,
        // own client may send messages, which are recieved by the Connection through this receiver
        receiver_of_connection: Receiver<Value>,
    ) -> Connection {
        let incoming_address = stream.peer_addr().unwrap().to_string();
        let reader = BufReader::new(stream.try_clone().unwrap());
        let mut writer = BufWriter::new(stream);
        Connection {
            source: incoming_address,
            read_handle: spawn(move || {
                for line in reader.lines() {
                    let json = line.expect("Client Disconnected");
                    let message = serde_json::from_str::<Value>(&json).unwrap();
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
