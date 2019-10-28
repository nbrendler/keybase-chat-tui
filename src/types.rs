use serde::{Deserialize, Serialize};

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
    ListenerEvent,
}
