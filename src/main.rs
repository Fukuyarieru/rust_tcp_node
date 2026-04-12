use crate::client::Client;

mod client;
mod test;

fn main() {
    let mut client = Client::new();
    client.start_handling();
    while let Ok(message) = client.receiver_of_messages_from_connections.recv() {
        match message {
            client::Message::Command(command) => {}
            client::Message::Text(text) => println!("{}", text),
        }
    }
}
