use serde_json::Value;
use std::{
    io::Result,
    net::{TcpListener, TcpStream},
    sync::{
        Arc, Mutex,
        mpsc::{Receiver, Sender, channel},
    },
    thread::{JoinHandle, spawn},
};

use crate::connection::Connection;

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
    /// * `TODO: TO CLOSE THREAD WE JOIN IT AND TAKE BACK THE RECEIVER TO NOT BREAK CONNECTION WITH THE SENDER
    sender_to_connections_handle: Option<JoinHandle<()>>,
    ///
    value_handling_method: Arc<Mutex<Option<fn(Value)>>>,
    ///
    connection_handling_method: Arc<Mutex<Option<fn(&mut Connection)>>>,
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
            value_handling_method: Arc::new(Mutex::new(None)),
            connection_handling_method: Arc::new(Mutex::new(None)),
        })
    }

    /// TODO: START_RECEIVING_CONNECTIONS AND START_RECEVING_MESSAGES?

    /// Method to use for the ability to connect the client to an outside address,\
    /// handling it as same "Connection" as all receiving connections\
    /// `handling_method` may be supplied to bake message handling locally within the connection
    pub fn connect(&mut self, address: &str, handling_method: Option<fn(Value)>) -> Result<()> {
        let stream = TcpStream::connect(address)?;
        let (s, r) = channel();
        let c = Connection::new(
            stream,
            self.sender_to_client.clone(),
            s.clone(),
            r,
            handling_method,
        );
        #[cfg(debug_assertions)]
        println!("CONNECTING -> {:?}", c);
        self.senders_to_connections.lock().unwrap().push(s);
        self.connections.lock().unwrap().push(c);
        Ok(())
    }

    /// Let the client to receive messages\
    /// This method will start a seperate thread to accept incoming tcp connections\
    /// and transform them into the 'Connection' made type and store them
    pub fn start_receiving(&mut self) {
        let tcp_listener = self.tcp_listener.try_clone().unwrap();
        let connections = self.connections.clone();
        let sender_to_client = self.sender_to_client.clone();
        let senders_to_connections = self.senders_to_connections.clone();
        let value_handling_method = self.value_handling_method.clone();
        let connection_handling_method = self.connection_handling_method.clone();
        self.new_connections_handle = Some(spawn(move || {
            for stream in tcp_listener.incoming() {
                let (s, r) = channel::<Value>();
                let mut c = Connection::new(
                    stream.unwrap(),
                    sender_to_client.clone(),
                    s.clone(),
                    r,
                    value_handling_method.lock().unwrap().clone(),
                );
                if let Some(method) = *connection_handling_method.lock().unwrap() {
                    #[cfg(debug_assertions)]
                    println!("{:?}", method);
                    method(&mut c);
                }
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

    pub fn pause_sending(&mut self) {
        // self.sender_to_connections_handle.unwrap().join();
        todo!()
    }

    /// This method will remove the clients' handling of sending messages
    pub fn stop_sending(&mut self) {
        self.sender_to_connections = None;
        self.sender_to_connections_handle = None;
    }

    pub fn change_value_handling_method(&self, handling_method: Option<fn(Value)>) {
        *self.value_handling_method.lock().unwrap() = handling_method;
    }

    pub fn change_connection_handling_method(&self, handling_method: Option<fn(&mut Connection)>) {
        *self.connection_handling_method.lock().unwrap() = handling_method;
    }
}
