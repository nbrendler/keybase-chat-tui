use std::process::{Command, Stdio};

use serde;
use serde::Deserialize;
use serde_json;
use serde_json::{from_slice, from_value, json, to_writer, Value};

#[derive(Debug, Deserialize)]
pub struct Channel {
    pub name: String,
    pub topic_type: String,
}

#[derive(Debug, Deserialize)]
pub struct Conversation {
    pub id: String,
    pub channel: Channel,
    pub unread: bool,
}

#[derive(Debug, Deserialize)]
pub struct MessageBody {
    pub body: String,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
#[allow(non_camel_case_types)]
pub enum MessageType {
    text { text: MessageBody },
    attachment {},
    unfurl {},
}

#[derive(Debug, Deserialize)]
pub struct MessageWrapper {
    pub msg: Message,
}

#[derive(Debug, Deserialize)]
pub struct Message {
    pub channel: Channel,
    pub content: MessageType,
    pub sender: Sender,
}

#[derive(Debug, Deserialize)]
pub struct Sender {
    pub username: String,
    pub device_name: String,
}

pub fn list_conversations() -> Vec<Conversation> {
    let result = keybase_exec(json!({
        "method": "list"
    }))
    .unwrap();

    let mut parsed: Value = from_slice(result.as_slice()).unwrap();
    from_value(parsed["result"]["conversations"].take())
        .expect("Failed to deserialize conversation list")
}

pub fn read_conversation<T: Into<String>>(name: T, count: u32) -> Vec<Message> {
    let result = keybase_exec(serde_json::json!({
        "method": "read",
        "params": {
            "options": {
                "channel": {"name": name.into()},
                "pagination": {"num": count}
            }
        }
    }))
    .unwrap();
    let mut parsed: Value = from_slice(result.as_slice()).unwrap();
    from_value::<Vec<MessageWrapper>>(parsed["result"]["messages"].take())
        .expect("Failed to deserialize messages")
        .into_iter()
        .map(|wrapper| wrapper.msg)
        .collect()
}

fn keybase_exec(command: Value) -> serde_json::Result<Vec<u8>> {
    let mut child = Command::new("keybase")
        .arg("chat")
        .arg("api")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to spawn Keybase");

    {
        let stdin = child.stdin.as_mut().expect("Failed to get child stdin");
        to_writer(stdin, &command)?;
    }

    let output = child.wait_with_output().expect("No Keybase output");
    Ok(output.stdout)
}
