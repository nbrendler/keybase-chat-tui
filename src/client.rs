// # client.rs
//
// A client struct which talks to the Keybase API, handles serialization and deserialization of the
// messages and writing to the proper channels.

use std::process::{Stdio};
use std::error::Error;

use tokio::process::{Child, Command};
use tokio::io::{BufReader, AsyncReadExt, AsyncWriteExt, AsyncBufReadExt};
use tokio::sync::mpsc::{self, Sender, Receiver};
use serde_json::{from_str, from_value, json, to_string_pretty, Value};
use async_trait::async_trait;

use crate::types::{
    Message, ApiResponseWrapper, ApiResponse, Channel, KeybaseConversation, ListenerEvent,
};

#[async_trait]
pub trait KeybaseClient {
    fn get_receiver(&mut self) -> Receiver<ListenerEvent>;
    async fn fetch_conversations(&self) -> Result<Vec<KeybaseConversation>, Box<dyn Error>>;
    async fn fetch_messages(&self, conversation: &KeybaseConversation, count: u32) -> Result<Vec<Message>, Box<dyn Error>>;
    async fn send_message<T: Into<String> + Send>(&self, channel: &Channel, message: T) -> Result<(), Box<dyn Error>>;
}

pub struct Client {
    receiver: Option<Receiver<ListenerEvent>>,
    subscriber: Option<Sender<ListenerEvent>>,
    listener: Option<Child>
}

impl Default for Client {
    fn default() -> Self {
        Client::new()
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        if let Some(mut c) = self.listener.take() {
            c.kill().unwrap()
        }
    }
}

#[async_trait]
impl KeybaseClient for Client {

    fn get_receiver(&mut self) -> Receiver<ListenerEvent>{
        self.receiver.take().unwrap()
    }

    async fn fetch_conversations(&self) -> Result<Vec<KeybaseConversation>, Box<dyn Error>> {
        let value = run_api_command(
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
        let value = run_api_command(
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
        run_api_command(
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

impl Client {
    pub fn new() -> Self {
        let (s, r) = mpsc::channel(32);
        let mut c = Client {
            receiver: Some(r), 
            subscriber: Some(s),
            listener: None
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

// helper to start the oneoff keybase process that will run our command
async fn run_api_command(command: Value) -> Result<Value, Box<dyn Error>> {
    let mut child = Command::new("keybase")
        .arg("chat")
        .arg("api")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to start keybase api process");

    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = child.stdout.take().unwrap();

    info!("Sending Keybase Command");
    debug!("Keybase Command: {}", to_string_pretty(&command)?);
    stdin.write_all(serde_json::to_vec(&command)?.as_slice()).await.unwrap();

    // close the pipe so the api knows we're done.
    drop(stdin);

    let mut buf = vec![];

    tokio::spawn(async move {
        child.await.expect("The child process encountered an error");
    });

    stdout.read_to_end(&mut buf).await?;

    let parsed: Value = serde_json::from_slice(&buf)?;
    info!("Got Keybase Response");
    debug!("Keybase Response: {}", to_string_pretty(&parsed)?);
    Ok(parsed)
}

