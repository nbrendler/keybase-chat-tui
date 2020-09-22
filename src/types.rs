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
    ConversationList {
        conversations: Vec<KeybaseConversation>,
    },
    MessageList {
        messages: Vec<MessageWrapper>,
    },
    MessageSent {
        message: String,
    },
}

#[derive(PartialOrd, Eq, PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum MemberType {
    #[serde(rename = "impteamnative")]
    User,
    #[serde(rename = "team")]
    Team,
}

#[derive(PartialOrd, Eq, PartialEq, Clone, Debug, Serialize, Deserialize)]
pub struct Channel {
    pub name: String,
    #[serde(default)]
    pub topic_name: String,
    pub members_type: MemberType,
}

#[derive(PartialOrd, PartialEq, Clone, Debug, Deserialize)]
pub struct KeybaseConversation {
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

pub enum UiMessage {
    SendMessage(String),
    SwitchConversation(String),
}

#[derive(Clone, Debug)]
pub struct Conversation {
    // id of the conversation (from Keybase)
    pub id: String,
    // keep track of whether we fetched this conversation
    pub fetched: bool,
    // messages we got from the API
    pub messages: Vec<Message>,

    pub data: KeybaseConversation,
}

impl Conversation {
    // put the message at the beginning (messages are in time-descending order)
    pub fn insert_message(&mut self, message: Message) {
        self.messages.insert(0, message);
    }

    pub fn insert_messages(&mut self, mut messages: Vec<Message>) {
        // assume these are already in time-descending order, so we swap them and then append the
        // older ones
        std::mem::swap(&mut self.messages, &mut messages);
        self.messages.extend(messages);
    }

    pub fn get_name(&self) -> String {
        match self.data.channel.members_type {
            MemberType::Team => format!(
                "{}#{}",
                &self.data.channel.name, &self.data.channel.topic_name
            ),
            // TODO: remove the username from the channel name for display
            MemberType::User => self.data.channel.name.to_string(),
        }
    }
}

impl From<KeybaseConversation> for Conversation {
    fn from(kb: KeybaseConversation) -> Conversation {
        Conversation {
            id: kb.id.clone(),
            fetched: false,
            messages: vec![],
            data: kb,
        }
    }
}
