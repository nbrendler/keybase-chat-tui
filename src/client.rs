use std::io::{BufRead, BufReader, Error};
use std::process::{Child, ChildStdout, Command, Stdio};
use std::thread;

use crossbeam::channel::{select, unbounded, Receiver, Sender};
use log::{debug, info};
use serde_json::{from_slice, from_str, from_value, json, to_string_pretty, to_writer, Value};

use crate::types::{ApiResponseWrapper, Channel, ClientMessage, Conversation, ListenerEvent};

pub struct Client {
    api_sender: Sender<Value>,
    api_receiver: Receiver<Value>,
    listener_receiver: Receiver<Value>,
    subscriber: Option<Sender<ClientMessage>>,
    listener_handle: Child,
}

impl Default for Client {
    fn default() -> Self {
        Client::new()
    }
}

impl Client {
    pub fn new() -> Self {
        let (s1, r1) = unbounded::<Value>();
        let (s2, r2) = unbounded::<Value>();
        let mut child = Command::new("keybase")
            .arg("chat")
            .arg("api-listen")
            .stdout(Stdio::piped())
            .spawn()
            .expect("Failed to start keybase listener process");

        debug!("Started listener process: {}", child.id());
        start_listener(child.stdout.take().unwrap(), s2);

        Client {
            api_sender: s1,
            api_receiver: r1,
            listener_receiver: r2,
            subscriber: None,
            listener_handle: child,
        }
    }

    pub fn step(&self) -> bool {
        select! {
            recv(self.api_receiver) -> msg => {
                if let Ok(value) = msg {
                    if let Some(s) = &self.subscriber {
                        let deserialized = ClientMessage::ApiResponse(from_value::<ApiResponseWrapper>(value).expect("Failed to deserialize API response").result);
                        s.send(deserialized).unwrap();
                    }
                }
            },
            recv(self.listener_receiver) -> msg => {
                if let Ok(value) = msg {
                    if let Some(s) = &self.subscriber {
                        let deserialized = ClientMessage::ListenerEvent(from_value::<ListenerEvent>(value).expect("Failed to deserialize listener event"));
                        s.send(deserialized).unwrap();
                    }
                }
            },
            default => {}
        };

        true
    }

    pub fn close(&mut self) -> Result<(), Error> {
        self.listener_handle.kill()?;
        Ok(())
    }

    pub fn register(&mut self) -> Receiver<ClientMessage> {
        let (s, r) = unbounded::<ClientMessage>();
        self.subscriber = Some(s);
        r
    }

    pub fn fetch_conversations(&self) {
        run_api_command(
            self.api_sender.clone(),
            json!({
                "method": "list"
            }),
        );
    }

    pub fn fetch_messages(&self, conversation: &Conversation, count: u32) {
        run_api_command(
            self.api_sender.clone(),
            json!({
                "method": "read",
                "params": {
                    "options": {
                        "channel": &conversation.channel,
                        "pagination": {"num": count}
                    }
                }
            }),
        );
    }

    pub fn send_message<T: Into<String>>(&self, channel: &Channel, message: T) {
        run_api_command(
            self.api_sender.clone(),
            json!({
                "method": "send",
                "params": {
                    "options": {
                        "channel": channel,
                        "message": {"body": message.into()}
                    }
                }
            }),
        );
    }
}

fn run_api_command(sender: Sender<Value>, command: Value) {
    let mut child = Command::new("keybase")
        .arg("chat")
        .arg("api")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to start keybase api process");

    debug!("Started process: {}", child.id());
    let stdin = child.stdin.take().unwrap();

    thread::spawn(move || {
        info!("Sending Keybase Command");
        debug!("Keybase Command: {}", to_string_pretty(&command).unwrap());
        to_writer(stdin, &command).unwrap();

        let output = child.wait_with_output().unwrap();

        let parsed: Value = from_slice(output.stdout.as_slice()).unwrap();
        info!("Got Keybase Response");
        debug!("Keybase Response: {}", to_string_pretty(&parsed).unwrap());
        sender.send(parsed).unwrap();
    });
}

fn start_listener(stdout: ChildStdout, sender: Sender<Value>) {
    thread::spawn(move || {
        let mut f = BufReader::new(stdout);
        loop {
            let mut buf = String::new();
            f.read_line(&mut buf).unwrap();
            let parsed: Value = from_str(buf.as_str()).unwrap();
            debug!("Listener Event: {}", to_string_pretty(&parsed).unwrap());
            sender.send(parsed).unwrap();
        }
    });
}
