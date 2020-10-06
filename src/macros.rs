#[cfg(test)]
#[macro_use]
mod test {
    #[macro_export]
    macro_rules! conversation {
        ($id:expr) => {{
            KeybaseConversation {
                id: $id.to_string(),
                unread: false,
                channel: Channel {
                    name: "channel".to_string(),
                    topic_name: "".to_string(),
                    members_type: MemberType::User,
                },
            }
        }};
    }

    #[macro_export]
    macro_rules! message {
        ($convo_id: expr, $text: expr) => {{
            use crate::types::Sender;
            Message {
                conversation_id: $convo_id.to_string(),
                content: MessageType::Text {
                    text: MessageBody {
                        body: $text.to_string(),
                    },
                },
                channel: Channel {
                    name: "channel".to_string(),
                    topic_name: "".to_string(),
                    members_type: MemberType::User,
                },
                sender: Sender {
                    device_name: "My Device".to_string(),
                    username: "Some Guy".to_string(),
                },
            }
        }};
    }
}
