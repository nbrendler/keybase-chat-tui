use client::{list_conversations, Conversation};

fn main() {
    let convos: Vec<Conversation> = list_conversations();
    let channel_names: Vec<String> = convos.into_iter().map(|c| c.channel.name).collect();
    println!("{:?}", channel_names);
}
