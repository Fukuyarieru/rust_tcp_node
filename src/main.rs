use std::{thread::sleep, time::Duration};

use serde_json::json;

use crate::tcp_node::TcpNode;

mod playground;
mod tcp_node;

const ADDRESS: &str = "127.0.0.1:8080";

fn main() {
    let client = TcpNode::new_with_address(ADDRESS);
    match client {
        Ok(mut server) => {
            server.start_receiving();
            while let Ok(message) = server.receiver_of_messages_from_connections.recv() {
                println!("{}", message)
            }
        }
        Err(_) => {
            let mut client = TcpNode::new().unwrap();
            client.start_sending();
            client
                .connect(ADDRESS, Some(|value| println!("{}", value)))
                .unwrap();
            let sender_to_connections = client.sender_to_connections.unwrap().clone();
            let mut counter = 0;
            loop {
                // println!("{}", counter);
                _ = sender_to_connections.send(json!(counter));
                counter += 1;
                sleep(Duration::from_millis(100));
            }
        }
    }
}
