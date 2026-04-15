use std::{
    thread::{self, sleep},
    time::Duration,
};

use serde_json::json;

use crate::tcp_node::TcpNode;

mod playground;
mod tcp_node;

const ADDRESS: &str = "127.0.0.1:8080";

fn main() {
    let client = TcpNode::new_with_address(ADDRESS);
    match client {
        Ok(mut node) => {
            node.start_receiving();
            node.change_connection_handling_method(Some(|c| {
                let sender = c.sender_to_connection();
                thread::spawn(move || {
                    let mut counter = 0;
                    loop {
                        // println!("{}", counter);
                        _ = sender.send(json!(counter));
                        counter += 1;
                    }
                });
            }));
            loop {}

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
            node.start_receiving();
            _ = node.connect(ADDRESS, None);
            node.change_value_handling_method(Some(|value| println!("{}", value)));
            loop {}

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
