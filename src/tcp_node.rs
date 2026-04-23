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
    tcp_listener: TcpListener,
    /// stored connections
    pub connections: Arc<Mutex<Vec<Connection>>>,
    /// connections send messages here
    pub receiver_of_messages_from_connections: Receiver<String>,
    /// sender given to each conenction to send the client new messages
    sender_to_client: Sender<String>,
    /// senders to every connection
    senders_to_connections: Arc<Mutex<Vec<Sender<String>>>>,
    /// thread accepting new connections
    /// * The thread will be created only during the `start_receiving` method
    new_connections_handle: Option<JoinHandle<()>>,
    /// * Sender to send messages to connections
    /// * May be None if `start_receiving` hasn't been called yet
    sender_to_connections: Option<Sender<String>>,
    ///
    receiver_of_sender_to_connections: Arc<Mutex<Option<Receiver<String>>>>,
    /// thread sending messages of the client to all connections
    /// * May be None if `start_receiving` hasn't been called yet
    sender_to_connections_handle: Option<JoinHandle<()>>,
    /// method given to every connection on every message received by the node
    message_handling_method: Arc<Mutex<Option<fn(String)>>>,
    /// method applied on every connection after construction
    connection_handling_method: Arc<Mutex<Option<fn(&mut Connection)>>>,
}

impl TcpNode {
    /// Method to create a new client.\
    /// Will connect to random available address.\
    /// * use `start_receiving` to let the node accept connection
    /// * use `start_sending` to let the node send messages
    pub fn new() -> Result<Self> {
        Self::new_with_address("127.0.0.1:0")
    }

    /// Method to create a new client.\
    /// * use `start_receiving` to let the node accept connection
    /// * use `start_sending` to let the node send messages
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
            receiver_of_sender_to_connections: Arc::new(Mutex::new(None)),
            sender_to_connections_handle: None,
            message_handling_method: Arc::new(Mutex::new(None)),
            connection_handling_method: Arc::new(Mutex::new(None)),
        })
    }

    /// TODO: START_ACCEPTING_CONNECTIONS AND START_RECEVING_MESSAGES?

    /// Method to use for the ability to connect the client to an outside address,\
    /// handling it as same "Connection" as all receiving connections\
    /// `handling_method` may be supplied to bake message handling locally within the connection
    pub fn connect(&mut self, address: &str, handling_method: Option<fn(String)>) -> Result<()> {
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
    pub fn start_accepting_connections(&mut self) {
        let tcp_listener = self.tcp_listener.try_clone().unwrap();
        let connections = self.connections.clone();
        let sender_to_client = self.sender_to_client.clone();
        let senders_to_connections = self.senders_to_connections.clone();
        let value_handling_method = self.message_handling_method.clone();
        let connection_handling_method = self.connection_handling_method.clone();
        self.new_connections_handle = Some(spawn(move || {
            for stream in tcp_listener.incoming() {
                let (s, r) = channel::<String>();
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

    /// Let the client send messages\
    /// * A thread will be started to handle sending messages\
    /// * The node will create appropriate sender and receiver for this task\
    /// * This destroys existing sender and receiver
    ///
    /// `Its required to call this before wanting to send messages`\
    pub fn start_sending(&mut self) {
        let (sender_to_connections, receiver_of_sender_to_connections) = channel::<String>();
        self.sender_to_connections = Some(sender_to_connections);
        self.receiver_of_sender_to_connections =
            Arc::new(Mutex::new(Some(receiver_of_sender_to_connections)));
        self.sender_to_connections_handle = Some(start_sending(
            self.receiver_of_sender_to_connections.clone(),
            self.senders_to_connections.clone(),
        ));
    }

    /// * Node's thread handling sending messages will be removed\
    /// * This perserves existing sender and receiver**
    pub fn pause_sending(&mut self) {
        self.sender_to_connections_handle = None;
    }

    /// Let the client to send messages\
    /// * This method will start a thread to handle sending messages to all connections\
    /// * This perserves existing sender and receiver
    pub fn resume_sending(&mut self) {
        self.sender_to_connections_handle = Some(start_sending(
            self.receiver_of_sender_to_connections.clone(),
            self.senders_to_connections.clone(),
        ))
    }

    /// This method will remove the clients' handling of sending messages
    pub fn stop_sending(&mut self) {
        self.sender_to_connections = None;
        self.sender_to_connections_handle = None;
        self.receiver_of_sender_to_connections = Arc::new(Mutex::new(None));
    }

    pub fn change_value_handling_method(&self, handling_method: Option<fn(String)>) {
        *self.message_handling_method.lock().unwrap() = handling_method;
    }

    pub fn change_connection_handling_method(&self, handling_method: Option<fn(&mut Connection)>) {
        *self.connection_handling_method.lock().unwrap() = handling_method;
    }
}

fn start_sending(
    receiver_of_sender_to_connections: Arc<Mutex<Option<Receiver<String>>>>,
    senders_to_connections: Arc<Mutex<Vec<Sender<String>>>>,
) -> JoinHandle<()> {
    spawn(move || {
        while let Ok(message) = receiver_of_sender_to_connections
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .recv()
        {
            for sender_to_connection in senders_to_connections.lock().unwrap().iter() {
                sender_to_connection.send(message.clone()).unwrap();
            }
        }
    })
}

fn start_receiving() {
    todo!("Copy `start_sending` logic over to `start_receiving`")
}
