// # client.rs
//
// A client struct which talks to the Keybase API, handles serialization and deserialization of the
// messages and writing to the proper channels.

use std::process::{Stdio};
use std::error::Error;

use tokio::process::{Child, Command};
use tokio::io::{BufReader, AsyncWriteExt, AsyncBufReadExt};
use tokio::sync::mpsc::{self, Sender, Receiver};
use serde_json::{from_str, from_value, json, to_string_pretty, Value};
use async_trait::async_trait;
#[cfg(test)]
use mockall::*;

use crate::types::{
    Message, ApiResponseWrapper, ApiResponse, Channel, KeybaseConversation, ListenerEvent,
};

#[cfg_attr(test, automock)]
#[async_trait]
pub trait KeybaseClient {
    fn get_receiver(&mut self) -> Receiver<ListenerEvent>;
    async fn fetch_conversations(&self) -> Result<Vec<KeybaseConversation>, Box<dyn Error>>;
    async fn fetch_messages(&self, conversation: &KeybaseConversation, count: u32) -> Result<Vec<Message>, Box<dyn Error>>;
    async fn send_message<T: Into<String> + Send + 'static>(&self, channel: &Channel, message: T) -> Result<(), Box<dyn Error>>;
}

pub struct Client<Executor: KeybaseExecutor> {
    receiver: Option<Receiver<ListenerEvent>>,
    subscriber: Option<Sender<ListenerEvent>>,
    listener: Option<Child>, 
    executor: Executor,
}

impl Default for Client<ClientExecutor> {
    fn default() -> Self {
        Client::new(ClientExecutor)
    }
}

impl<Executor: KeybaseExecutor> Drop for Client<Executor> {
    fn drop(&mut self) {
        if let Some(mut c) = self.listener.take() {
            c.kill().unwrap()
        }
    }
}

#[async_trait]
impl<Executor: KeybaseExecutor + Send + Sync + 'static> KeybaseClient for Client<Executor> {

    fn get_receiver(&mut self) -> Receiver<ListenerEvent>{
        self.receiver.take().unwrap()
    }

    async fn fetch_conversations(&self) -> Result<Vec<KeybaseConversation>, Box<dyn Error>> {
        let value = self.executor.run_api_command(
            json!({
                "method": "list"
            }),
        ).await?;
        let parsed = from_value::<ApiResponseWrapper>(value)?.result;
        if let ApiResponse::ConversationList { conversations: convos } = parsed {
            return Ok(convos);
        }
        // should be an Err
        Ok(vec![])
    }

    async fn fetch_messages(&self, conversation: &KeybaseConversation, count: u32) -> Result<Vec<Message>, Box<dyn Error>>{
        let value = self.executor.run_api_command(
            json!({
                "method": "read",
                "params": {
                    "options": {
                        "channel": &conversation.channel,
                        "pagination": {"num": count}
                    }
                }
            }),
        ).await?;
        let parsed = from_value::<ApiResponseWrapper>(value)?.result;
        if let ApiResponse::MessageList { messages: wrapper } = parsed {
            return Ok(wrapper.into_iter().map(|m| m.msg).collect::<Vec<Message>>());
        }
        // should be an Err
        Ok(vec![])
    }

    async fn send_message<T: Into<String> + Send>(&self, channel: &Channel, message: T) -> Result<(), Box<dyn Error>> {
        self.executor.run_api_command(
            json!({
                "method": "send",
                "params": {
                    "options": {
                        "channel": channel,
                        "message": {"body": message.into()}
                    }
                }
            }),
        ).await?;
        Ok(())
    }

}

impl<Executor: KeybaseExecutor> Client<Executor> {
    pub fn new(executor: Executor) -> Self {
        let (s, r) = mpsc::channel(32);
        let mut c = Client {
            receiver: Some(r), 
            subscriber: Some(s),
            listener: None, 
            executor
        };
        c.listener = Some(c.start_listener().unwrap());
        c
    }

    pub fn start_listener(&self) -> Result<Child, Box<dyn Error>> {
        let mut child = Command::new("keybase")
            .arg("chat")
            .arg("api-listen")
            .stdout(Stdio::piped())
            .spawn()?;

        debug!("Started listener process: {}", child.id());

        let stdout = child.stdout.take().unwrap();
        let mut subscriber = self.subscriber.clone().unwrap();

        tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Some(line) = lines.next_line().await.unwrap() {
                let parsed: Value = from_str(&line).unwrap();
                debug!("Listener Event: {}", to_string_pretty(&parsed).unwrap());
                let event = from_value::<ListenerEvent>(parsed).unwrap();
                subscriber.send(event).await.unwrap();
            }
        });

        Ok(child)
    }
}

pub struct ClientExecutor;

#[cfg_attr(test, automock)]
#[async_trait]
pub trait KeybaseExecutor {
    // helper to start the oneoff keybase process that will run our command
    async fn run_api_command(&self, command: Value) -> Result<Value, Box<dyn Error>>;
}

#[async_trait]
impl KeybaseExecutor for ClientExecutor {
    async fn run_api_command(&self, command: Value) -> Result<Value, Box<dyn Error>> {
        let mut child = Command::new("keybase")
            .arg("chat")
            .arg("api")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("Failed to start keybase api process");

        {
            // scoped so that the pipe is dropped
            let mut stdin = child.stdin.take().unwrap();

            info!("Sending Keybase Command");
            debug!("Keybase Command: {}", to_string_pretty(&command)?);
            stdin.write_all(serde_json::to_vec(&command)?.as_slice()).await.unwrap();
        }


        let output = child.wait_with_output().await?;

        let parsed: Value = serde_json::from_slice(&output.stdout)?;
        info!("Got Keybase Response");
        debug!("Keybase Response: {}", to_string_pretty(&parsed)?);
        Ok(parsed)
    }
}


#[cfg(test)]
mod test {
    use super::*;
    use crate::{message, conversation};
    use crate::types::*;

    #[tokio::test]
    async fn fetch_list() {
        let convos = vec![conversation!("test1"), conversation!("test2")];
        let mut executor = MockKeybaseExecutor::new();
        executor.expect_run_api_command()
            .times(1)
            .return_once(|_| {
                Ok(json!({
                    "result": {
                        "conversations": [
                        {
                            "id": "test1",
                            "active_at": 1,
                            "active_at_ms": 1000,
                            "channel": {
                                "members_type": "impteamnative",
                                "name": "channel",
                                "topic_type": "chat"
                            },
                            "creator_info": {
                                "ctime": 1,
                                "username": "test"
                            },
                            "unread": false,
                            "member_status": "active",
                            "is_default_conv": false

                        },
                        {
                            "id": "test2",
                            "active_at": 1,
                            "active_at_ms": 1000,
                            "channel": {
                                "members_type": "impteamnative",
                                "name": "channel",
                                "topic_type": "chat"
                            },
                            "creator_info": {
                                "ctime": 1,
                                "username": "test"
                            },
                            "unread": false,
                            "member_status": "active",
                            "is_default_conv": false

                        }
                        ]
                    }
                }))
            });

        let client = Client::new(executor);

        assert_eq!(convos, client.fetch_conversations().await.unwrap());
    }

    #[tokio::test]
    async fn fetch_messages() {
        let mut executor = MockKeybaseExecutor::new();
        executor.expect_run_api_command()
            .times(1)
            .return_once(|_| {
                Ok(json!({
                    "result": {
                        "messages": [
                        {
                            "msg": {
                                "id": "msg_id",
                                "conversation_id": "test1", 
                                "channel": {
                                    "members_type": "impteamnative",
                                    "name": "channel",
                                    "topic_type": "chat"
                                },
                                "content": {
                                    "text": {
                                        "body": "hi"
                                    },
                                    "type": "text"
                                },
                                "sender": {
                                    "device_id": "1",
                                    "device_name": "My Device",
                                    "uid": "1",
                                    "username": "Some Guy"
                                },
                                "unread": false
                            }
                        },
                        ],
                        "pagination": {
                            "next": "next",
                            "num": 1,
                            "previous": "prev"
                        }
                    }
                }))
            });

        let client = Client::new(executor);

        let convo = conversation!("test1");
        let messages = vec![message!("test1", "hi")];

        assert_eq!(messages, client.fetch_messages(&convo, 10).await.unwrap());
    }

    #[tokio::test]
    async fn send_message() {
        let convo = conversation!("test1");
        let my_value = json!({
            "method": "send",
            "params": {
                "options": {
                    "channel": convo.channel,
                    "message": {"body": "hi"}
                }
            }
        });
        let mut executor = MockKeybaseExecutor::new();
        executor.expect_run_api_command()
            .withf(move |value: &Value| *value == my_value)
            .times(1)
            .return_once(move |_| Ok(Value::Null));
        let client = Client::new(executor);

        client.send_message(&convo.channel, "hi").await.unwrap();
    }
}

