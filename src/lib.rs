use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};
use serde_json;

#[derive(Debug, Deserialize)]
pub struct Channel {
    pub name: String,
    pub members_type: String,
    pub topic_type: String,
}

// TODO: date fields
#[derive(Debug, Deserialize)]
pub struct Conversation {
    pub id: String,
    pub channel: Channel,
    pub unread: bool,
    pub member_status: String,
}

#[derive(Deserialize)]
struct ConversationResultInner {
    conversations: Vec<Conversation>,
}

#[derive(Deserialize)]
struct ConversationResult {
    result: ConversationResultInner,
}

#[derive(Serialize)]
struct KBCommand {
    pub method: &'static str,
}

pub fn list_conversations() -> Vec<Conversation> {
    let result = keybase_exec(KBCommand { method: "list" }).unwrap();
    let parsed_result: ConversationResult =
        serde_json::from_slice(result.as_slice()).expect("Failed to parse conversation result");
    parsed_result.result.conversations
}

fn keybase_exec(command: KBCommand) -> serde_json::Result<Vec<u8>> {
    let mut child = Command::new("keybase")
        .arg("chat")
        .arg("api")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to spawn Keybase");

    {
        let stdin = child.stdin.as_mut().expect("Failed to get child stdin");
        serde_json::to_writer(stdin, &command)?;
    }

    let output = child.wait_with_output().expect("No Keybase output");
    Ok(output.stdout)
}
