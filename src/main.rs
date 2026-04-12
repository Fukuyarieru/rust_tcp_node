use crate::tcp_node::TcpNode;

mod tcp_node;
mod test;

fn main() {
    let mut client = TcpNode::new();
    client.start_handling();
    while let Ok(message) = client.receiver_of_messages_from_connections.recv() {}
}
