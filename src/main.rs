use crate::tcp_node::TcpNode;
use std::{
    thread::{self, sleep},
    time::Duration,
};

mod connection;
mod tcp_node;

pub type Method<T> = Box<dyn FnMut(T) + Send + 'static>;

const ADDRESS: &str = "127.0.0.1:8080";

fn main() {
    let client = TcpNode::new_with_address(ADDRESS);
    match client {
        Ok(mut node) => {
            node.start_accepting_connections();
            node.start_sending();
            node.change_connection_handling_method(Some(Box::new(|c| {
                let sender = c.sender_to_connection();
                thread::spawn(move || {
                    let mut counter = 0;
                    loop {
                        println!("{}", counter);
                        _ = sender.send(counter.to_string());
                        counter += 1;
                        sleep(Duration::from_millis(100));
                    }
                });
            })));
            thread::park();

            // node.change_value_handling_method(Some(|value| {}));
            // TODO: CHANGE_HANDLING_CONNECTIONS

            // server.start_receiving();
            // server.change_handling_method(Some(|value| println!("{}", value)));
            // while let Ok(message) = server.receiver_of_messages_from_connections.recv() {
            //     println!("{}", message)
            // }
        }
        Err(_) => {
            let mut node = TcpNode::new().unwrap();
            _ = node.connect(ADDRESS, Some(|value| println!("{}", value)));
            thread::park();

            // let mut client = TcpNode::new().unwrap();
            // client.start_sending();
            // client
            //     .connect(ADDRESS, Some(|value| println!("{}", value)))
            //     .unwrap();
            // let sender_to_connections = client.sender_to_connections.unwrap().clone();
            // let mut counter = 0;
            // loop {
            //     // println!("{}", counter);
            //     _ = sender_to_connections.send(json!(counter));
            //     counter += 1;
            //     sleep(Duration::from_millis(100));
            // }
        }
    }
}
