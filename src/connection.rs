use std::{
    fmt::Debug,
    io::{BufRead, BufReader, BufWriter, Write},
    net::TcpStream,
    sync::{
        Arc, Mutex,
        mpsc::{Receiver, Sender},
    },
    thread::{JoinHandle, spawn},
};

type Method = Box<dyn FnMut(String) + Send + 'static>;

// TODO: Combine handles into a single _worker_handle, which does both jobs
pub struct Connection {
    /// source of the client connected to the connection
    source: String,
    /// thread handling reading from the connected client
    _read_handle: JoinHandle<()>,
    /// thread handling sending messages to the connected client
    _write_handle: JoinHandle<()>,
    /// local method stored with the connection,
    /// applies on incoming messages from the `_read_handle`,
    connection_handle_method: Arc<Mutex<Option<Method>>>,
    /// the connection receives messages through a sender,
    /// said sender is given during construction of the
    /// connection, and is cloned over here
    sender_to_connection: Sender<String>,
}

impl Connection {
    /// This is the method to convert tcp stream into a "Connection"\
    /// interactions with the Connection are made using a channel() made before calling this method (sender,receiver)\
    /// we give this function the receiver and handle the sender to be however we want to send messages to this Connection
    pub fn new<F: FnMut(String) + Debug + Send + 'static>(
        stream: TcpStream,
        // own client may receive message, these are sent to a receiver on the client through this sender the Connection got
        sender_of_messages_to_the_client: Sender<String>,
        // own client may send messages, this is a copy of the sender
        sender_of_messages_to_the_connection: Sender<String>,
        // own client may send messages, which are recieved by the Connection through this receiver
        recevier_of_messages_for_the_connection: Receiver<String>,
        // local method stored with the connection,
        // applies on incoming messages from the `_read_handle`,
        handle_method: Option<F>,
    ) -> Self {
        let handle_method: Option<Method> = match handle_method {
            Some(method) => Some(Box::new(method)),
            None => None,
        };
        let incoming_address = stream.peer_addr().unwrap().to_string();
        let reader = BufReader::new(stream.try_clone().unwrap());
        let mut writer = BufWriter::new(stream);
        let handling_method = Arc::new(Mutex::new(handle_method));
        let handling_method_clone = handling_method.clone();
        Self {
            source: incoming_address,
            _read_handle: spawn(move || {
                for message in reader.lines() {
                    match message {
                        Ok(message) => {
                            if let Some(method) = handling_method_clone.lock().unwrap().as_mut() {
                                // #[cfg(debug_assertions)]
                                // println!("{:?}", method);
                                method(message.clone())
                            }
                            sender_of_messages_to_the_client.send(message).unwrap();
                        }
                        Err(e) => eprintln!("{}", e),
                    }
                }
            }),
            _write_handle: spawn(move || {
                while let Ok(message) = recevier_of_messages_for_the_connection.recv() {
                    // TODO: buffer may get full, handle erroring
                    writer.write_all((message + "\n").as_bytes()).unwrap();
                    writer.flush().unwrap();
                }
            }),
            connection_handle_method: handling_method,
            sender_to_connection: sender_of_messages_to_the_connection,
        }
    }

    pub fn sender_to_connection(&self) -> Sender<String> {
        self.sender_to_connection.clone()
    }

    pub fn change_method<F: FnMut(String) + Send + 'static>(&self, method: Option<F>) {
        let method: Option<Method> = match method {
            Some(method) => Some(Box::new(method)),
            None => None,
        };
        *self.connection_handle_method.lock().unwrap() = method;
    }

    pub fn source(&self) -> String {
        self.source.clone()
    }
}

impl Debug for Connection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Blocking inside a Debug impl is dangerous and can cause deadlocks.
        let method_status = match self.connection_handle_method.try_lock() {
            Ok(guard) => {
                if guard.is_some() {
                    "Some(<closure>)"
                } else {
                    "None"
                }
            }
            Err(_) => "<locked>",
        };

        f.debug_struct("Connection")
            .field("source", &self.source)
            .field("_read_handle", &self._read_handle)
            .field("_write_handle", &self._write_handle)
            // Format the closure status as a plain string
            .field(
                "connection_handle_method",
                &format_args!("{}", method_status),
            )
            .field("sender_to_connection", &self.sender_to_connection)
            .finish()
    }
}
