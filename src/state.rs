// # state.rs
//
// This struct owns most of the data for the app and exposes methods for reading and manipulating
// it, as well as registering simple observers that are notified on state changes.
//
// This went through a LOT of different iterations. I originally wanted the UI object to register
// callbacks on particular events, like this:
//
// ```rust
// let ui = Ui::new();
// let state = State::default();
//
// state.on_conversation_added(|conversation: &Conversation| {
//  ui.render_conversation(conversation);
// });
// ```
//
// But I ran into lots of lifetime issues with the callbacks being owned by the state, and needing
// the UI to also be in the closure so it can call render methods. I ended up ripping all that out
// and using this trait `StateObserver`, and composing a UiObserver that implements it as a child
// of the main UI struct. This is still more coupling than I wanted originally, but it seems to
// work OK. I'm sure a more experienced Rust developer could design this better!

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::client::Client;
use crate::types::{Conversation, ConversationData, Message};

type ConversationId = String;

// Trait that interested parties can implement (and register themselves below) to receive
// notifications when state changes. The APIs are all a little hodge-podge depending on what I
// needed to render in each case.
pub trait StateObserver {
    fn on_conversation_change(&mut self, title: &str, data: &ConversationData);
    fn on_conversations_added(&mut self, data: &[Conversation]);
    fn on_message(&mut self, data: &Message, conversation_id: &str, active: bool);
}

// This is the inner struct that lives inside the Arc<Mutex> which masquerades as the actual state.
#[derive(Default)]
pub struct ApplicationStateInner {
    client: Client,
    // conversation id of the currently displayed conversation
    current_conversation: Option<ConversationId>,
    // list of available conversations displayed on the left. I'm using a Vec to make rendering
    // easier, but I think this could probably be combined with the HashMap below and just index by
    // conversation id. We end up searching through this Vec using `find` to get the Conversation
    // by id multiple times, and only render once per app run.
    //
    // TODO: merge with the other hash map
    conversations: Vec<Conversation>,
    // map of chat messages by conversation id
    chats: HashMap<ConversationId, ConversationData>,

    // List of registered observers
    observers: Vec<Box<dyn StateObserver>>,
}

// There's a lot of string stuff going on here that is probably done wrong. I tried to stick with
// the rule of thumb that the state in this file should own the strings (using `String`), and data
// should go in or out as read-only slices (`&str`), but I think it's still doing some unnecessary
// allocations here. I can come back to it with a deeper understanding later.
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

    pub fn set_current_conversation(&mut self, conversation_id: &str) {
        self.current_conversation = Some(conversation_id.to_owned());
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

    // helper function for getting a defaultdict-like behavior on the chats HashMap
    fn get_or_insert_entry(&mut self, conversation_id: &str) -> &mut ConversationData {
        self.chats
            .entry(conversation_id.to_owned())
            .or_insert_with(ConversationData::default)
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

    // Hiding all the mutex ugliness with this callback-based interface
    pub fn with_data<R, F: FnOnce(&mut ApplicationStateInner) -> R>(&mut self, f: F) -> R {
        f(&mut self.inner.lock().unwrap())
    }
}
