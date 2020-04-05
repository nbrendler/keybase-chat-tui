use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::client::Client;
use crate::types::{Conversation, ConversationData, Message};

pub trait StateObserver {
    fn on_conversation_change(&mut self, title: &str, data: &ConversationData);
    fn on_conversations_added(&mut self, data: &[Conversation]);
    fn on_message(&mut self, data: &Message, conversation_id: &str, active: bool);
}

#[derive(Default)]
pub struct ApplicationStateInner {
    client: Client,
    // conversation id of the currently displayed conversation
    current_conversation: Option<String>,
    // list of available conversations displayed on the left
    conversations: Vec<Conversation>,
    // list of chat messages by conversation id
    chats: HashMap<String, ConversationData>,

    observers: Vec<Box<dyn StateObserver>>,
}

impl ApplicationStateInner {
    pub fn get_client_mut(&mut self) -> &mut Client {
        &mut self.client
    }

    pub fn add_chat(&mut self, conversation_id: &str, data: ConversationData) {
        let title = &self
            .conversations
            .iter()
            .find(|convo| convo.id == conversation_id)
            .unwrap()
            .channel
            .name;
        self.observers
            .iter_mut()
            .for_each(|o| o.on_conversation_change(&title, &data));
        self.chats.insert(conversation_id.to_owned(), data);
    }

    fn get_or_insert_entry(&mut self, conversation_id: &str) -> &mut ConversationData {
        self.chats
            .entry(conversation_id.to_owned())
            .or_insert_with(ConversationData::default)
    }

    pub fn add_chat_message(&mut self, conversation_id: &str, message: Message) {
        let is_active = {
            match self.current_conversation {
                Some(ref convo_id) => convo_id.as_str() == conversation_id,
                None => false,
            }
        };
        self.observers
            .iter_mut()
            .for_each(|o| o.on_message(&message, conversation_id, is_active));
        let e = { self.get_or_insert_entry(&conversation_id.to_owned()) };
        e.add_message(message);
    }

    pub fn set_current_conversation(&mut self, conversation_id: &String) {
        self.current_conversation = Some(conversation_id.clone());
    }

    pub fn set_conversations(&mut self, conversations: Vec<Conversation>) {
        self.observers
            .iter_mut()
            .for_each(|o| o.on_conversations_added(conversations.as_slice()));
        self.conversations = conversations;
    }

    pub fn switch_to_conversation(&mut self, conversation: &Conversation) {
        let e = self.get_or_insert_entry(&conversation.id);
        if !e.fetched {
            self.client.fetch_messages(conversation, 20);
        }
        let conversation_data = self.chats.get(&conversation.id).unwrap();
        self.observers
            .iter_mut()
            .for_each(|o| o.on_conversation_change(&conversation.channel.name, conversation_data));
        self.set_current_conversation(&conversation.id);
    }

    pub fn send_message(&mut self, message: String) {
        match &mut self.current_conversation {
            Some(convo_id) => {
                let data = self
                    .conversations
                    .iter()
                    .find(|c| c.id.eq(convo_id))
                    .unwrap();
                let channel = &data.channel;
                self.client.send_message(channel, message);
            }
            None => {
                panic!("tried to send a message without a current conversation");
            }
        }
    }

    pub fn register_observer(&mut self, observer: Box<dyn StateObserver>) {
        self.observers.push(observer);
    }
}

#[derive(Clone)]
pub struct ApplicationState {
    inner: Arc<Mutex<ApplicationStateInner>>,
}

impl ApplicationState {
    pub fn new() -> Self {
        ApplicationState {
            inner: Arc::new(Mutex::new(ApplicationStateInner::default())),
        }
    }

    pub fn with_data<R, F: FnOnce(&mut ApplicationStateInner) -> R>(&mut self, f: F) -> R {
        f(&mut self.inner.lock().unwrap())
    }
}
