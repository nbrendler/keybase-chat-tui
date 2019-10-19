use std::process::{Command, Stdio};

#[macro_use]
use log::debug;
use serde;
use serde::{Deserialize, Serialize};
use serde_json;
use serde_json::{from_slice, from_value, json, to_string_pretty, to_writer, Value};

#[derive(Debug, Serialize, Deserialize)]
pub enum MemberType {
    #[serde(rename = "impteamnative")]
    User,
    #[serde(rename = "team")]
    Team,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Channel {
    pub name: String,
    #[serde(default)]
    pub topic_name: String,
    pub members_type: MemberType,
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
    #[serde(rename = "join")]
    Join,
    #[serde(rename = "attachment")]
    Attachment {},
    #[serde(rename = "metadata")]
    Metadata {},
    #[serde(rename = "system")]
    System {},
    #[serde(rename = "text")]
    Text { text: MessageBody },
    #[serde(rename = "unfurl")]
    Unfurl {},
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
    let mut result = keybase_exec(json!({
        "method": "list"
    }))
    .unwrap();

    from_value(result["result"]["conversations"].take())
        .expect("Failed to deserialize conversation list")
}

pub fn read_conversation(channel: &Channel, count: u32) -> Vec<Message> {
    let mut result = keybase_exec(json!({
        "method": "read",
        "params": {
            "options": {
                "channel": channel,
                "pagination": {"num": count}
            }
        }
    }))
    .unwrap();
    from_value::<Vec<MessageWrapper>>(result["result"]["messages"].take())
        .expect("Failed to deserialize messages")
        .into_iter()
        .map(|wrapper| wrapper.msg)
        .collect()
}

pub fn send_message<T: Into<String>>(channel: &Channel, message: T) {
    keybase_exec(json!({
        "method": "send",
        "params": {
            "options": {
                "channel": channel,
                "message": {"body": message.into()}
            }
        }
    }))
    .expect("Failed to send message");
}

fn keybase_exec(command: Value) -> serde_json::Result<Value> {
    let mut child = Command::new("keybase")
        .arg("chat")
        .arg("api")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to spawn Keybase");

    {
        let stdin = child.stdin.as_mut().expect("Failed to get child stdin");
        debug!("{}", to_string_pretty(&command).unwrap());
        to_writer(stdin, &command)?;
    }

    let output = child.wait_with_output().expect("No Keybase output");
    let parsed: Value = from_slice(output.stdout.as_slice()).unwrap();

    debug!("{}", to_string_pretty(&parsed).unwrap());
    Ok(parsed)
}
