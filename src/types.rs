// # types.rs
//
// Various type definitions used by all parts of the app. Mostly dumb structs with data inside
// them, and lots of Serde annotations for serialization/deserialization.
//
// A lot of these were just trial and error while using the Keybase API and fixing serialization
// errors.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "type")]
pub enum ListenerEvent {
    #[serde(rename = "chat")]
    ChatMessage(MessageWrapper),
}

#[derive(Clone, Debug, Deserialize)]
pub struct ApiResponseWrapper {
    pub result: ApiResponse,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub enum ApiResponse {
    ConversationList { conversations: Vec<Conversation> },
    MessageList { messages: Vec<MessageWrapper> },
    MessageSent { message: String },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum MemberType {
    #[serde(rename = "impteamnative")]
    User,
    #[serde(rename = "team")]
    Team,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Channel {
    pub name: String,
    #[serde(default)]
    pub topic_name: String,
    pub members_type: MemberType,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Conversation {
    pub id: String,
    pub channel: Channel,
    pub unread: bool,
}

#[derive(Clone, Debug, Deserialize)]
pub struct MessageBody {
    pub body: String,
}

#[derive(Clone, Debug, Deserialize)]
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
    #[serde(rename = "reaction")]
    Reaction {},
}

#[derive(Clone, Debug, Deserialize)]
pub struct MessageWrapper {
    pub msg: Message,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Message {
    pub channel: Channel,
    pub content: MessageType,
    pub sender: Sender,
    pub conversation_id: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Sender {
    pub username: String,
    pub device_name: String,
}

pub enum ClientMessage {
    ApiResponse(ApiResponse),
    ListenerEvent(ListenerEvent),
}

#[derive(Clone, Debug)]
pub struct ConversationData {
    // keep track of whether we fetched this conversation
    pub fetched: bool,
    // messages we got from the API
    pub messages: Vec<Message>,
}

impl Default for ConversationData {
    fn default() -> Self {
        ConversationData {
            fetched: false,
            messages: Vec::new(),
        }
    }
}

impl ConversationData {
    pub fn new(messages: Vec<Message>) -> Self {
        ConversationData {
            fetched: true,
            messages,
        }
    }
    // put the message at the beginning (messages are in time-descending order)
    pub fn add_message(&mut self, message: Message) {
        self.messages.insert(0, message);
    }
}
