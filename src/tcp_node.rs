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
    /// * The thread will be created only during the `start_receiving` method
    new_connections_handle: Option<JoinHandle<()>>,
    /// * Sender to send messages to connections
    /// * May be None if `start_receiving` hasn't been called yet
    pub sender_to_connections: Option<Sender<Value>>,
    /// thread sending messages of the client to all
    /// * May be None if `start_receiving` hasn't been called yet
    sender_to_connections_handle: Option<JoinHandle<()>>,
}

impl TcpNode {
    /// * Method to create a new client.
    /// * Will connect to random available address.
    /// * Said client needs to be "started" to accept connections using `start_receiving`
    pub fn new() -> Result<Self> {
        Self::new_with_address("127.0.0.1:0")
    }

    /// * Method to create a new client.
    /// * Said client needs to be "started" to accept connections using `start_receiving`
    pub fn new_with_address(address: &str) -> Result<Self> {
        let (sender, receiver) = channel();
        let tcp_listener = TcpListener::bind(address)?;
        #[cfg(debug_assertions)]
        println!(
            "NODE ADDRESS -> {}",
            tcp_listener.local_addr().unwrap().to_string()
        );
        Ok(Self {
            connections: Arc::new(Mutex::new(Vec::new())),
            tcp_listener: tcp_listener,
            receiver_of_messages_from_connections: receiver,
            sender_to_client: sender,
            senders_to_connections: Arc::new(Mutex::new(Vec::new())),
            new_connections_handle: None,
            sender_to_connections: None,
            sender_to_connections_handle: None,
        })
    }

    /// Method to use for the ability to connect the client to an outside address,\
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

    // pub fn connect_and_handle(&mut self, address: &str, handle: JoinHandle<()>) -> Result<()> {
    //     let stream = TcpStream::connect(address)?;
    //     let (s, r) = channel();
    //     let c = Connection::new(stream, self.sender_to_client.clone(), r);
    //     #[cfg(debug_assertions)]
    //     println!("CONNECTING -> {:?}", c);
    //     self.senders_to_connections.lock().unwrap().push(s);
    //     self.connections.lock().unwrap().push(c);
    //     Ok(())
    // }

    /// Let the client to recieve messages\
    /// This method will start a seperate thread to accept incoming tcp connections\
    /// and transform them into the 'Connection' made type and store them
    pub fn start_receiving(&mut self) {
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
    }

    /// Non destructive pause
    pub fn pause_receiving(&mut self) {
        todo!()
    }

    /// Destructive stop which will require calling `start_receiving` again
    pub fn stop_receiving(&mut self) {
        todo!()
    }

    /// Let the client to send messages\
    /// This method will start a thread to handle sending\
    /// messages to all connected at the moment Connections\
    /// `Its required to call this before wanting to send messages`
    pub fn start_sending(&mut self) {
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

    /// This method will remove the clients' handling of sending messages
    pub fn stop_sending(&mut self) {
        self.sender_to_connections = None;
        self.sender_to_connections_handle = None;
    }

    /// Changing TcpListener and appropriate handling TODO
    pub fn change_address(&mut self, address: &str) {
        todo!()
    }
}

#[derive(Debug)]
pub struct Connection {
    /// source of the client connected
    source: String,
    /// thread handling the connected client
    _read_handle: JoinHandle<()>,
    /// thread handling sending messages to a connection
    _write_handle: JoinHandle<()>,
    // /// bake inside the Connection its' handling
    // TODO: this doesnt work yet as we dont save content inside the handle
    // pub connection_handle: Option<JoinHandle<()>>,
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
        // //
        // handle_method: Option<JoinHandle<()>>,
    ) -> Self {
        let incoming_address = stream.peer_addr().unwrap().to_string();
        let reader = BufReader::new(stream.try_clone().unwrap());
        let mut writer = BufWriter::new(stream);
        Self {
            source: incoming_address,
            _read_handle: spawn(move || {
                for line in reader.lines() {
                    let json = line.expect("Client Disconnected");
                    let message = serde_json::from_str::<Value>(&json).unwrap();
                    sender_to_client.send(message).unwrap();
                }
            }),
            _write_handle: spawn(move || {
                while let Ok(message) = receiver_of_connection.recv() {
                    let message_json = serde_json::to_string(&message).unwrap() + "\n";
                    writer.write_all(message_json.as_bytes()).unwrap();
                    writer.flush().unwrap();
                }
            }),
            // connection_handle: handle_method,
        }
    }

    #[allow(dead_code)]
    pub fn source(&self) -> String {
        self.source.clone()
    }
}
