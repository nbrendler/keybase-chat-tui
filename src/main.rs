use client::{list_conversations, read_conversation, MessageType};

fn main() {
    let convos = list_conversations();
    println!("{:?}", convos);

    let messages = read_conversation(&convos[0].channel.name, 20);
    for m in messages.iter().rev() {
        match &m.content {
            MessageType::text { text } => {
                println!("{}: {}", m.sender.username, text.body);
            }
            _ => {}
        }
    }
}
