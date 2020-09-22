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

use std::collections::hash_map::Values;
use std::collections::HashMap;

use crate::types::{Conversation, Message};

type ConversationId = String;

// Trait that interested parties can implement (and register themselves below) to receive
// notifications when state changes. The APIs are all a little hodge-podge depending on what I
// needed to render in each case.
pub trait StateObserver {
    fn on_conversation_change(&mut self, data: &Conversation);
    fn on_conversations_added(&mut self, data: &[Conversation]);
    fn on_message(&mut self, data: &Message, conversation_id: &str, active: bool);
}

// This is the inner struct that lives inside the Arc<Mutex> which masquerades as the actual state.
#[derive(Default)]
pub struct ApplicationStateInner {
    // conversation id of the currently displayed conversation
    current_conversation: Option<ConversationId>,

    // map of chat messages by conversation id
    conversations: HashMap<ConversationId, Conversation>,

    // List of registered observers
    observers: Vec<Box<dyn StateObserver>>,
}

pub struct Conversations<'a, I: Iterator<Item = &'a Conversation>> {
    inner: I,
}

impl<'a, I: Iterator<Item = &'a Conversation>> Iterator for Conversations<'a, I> {
    type Item = &'a Conversation;

    #[inline]
    fn next(&mut self) -> Option<&'a Conversation> {
        self.inner.next()
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<'a, I: ExactSizeIterator + Iterator<Item = &'a Conversation>> ExactSizeIterator
    for Conversations<'a, I>
{
    #[inline]
    fn len(&self) -> usize {
        self.inner.len()
    }
}

pub trait ApplicationState {
    fn insert_conversation(&mut self, conversation: Conversation);
    fn insert_message(&mut self, conversation_id: &str, message: Message);
    fn set_current_conversation(&mut self, conversation_id: &str);
    fn get_current_conversation(&self) -> Option<&Conversation>;
    fn set_conversations(&mut self, conversations: Vec<Conversation>);
    fn get_conversations(&self) -> Conversations<Values<'_, String, Conversation>>;
    fn register_observer(&mut self, observer: Box<dyn StateObserver>);
    fn get_conversation(&self, conversation_id: &str) -> Option<&Conversation>;
    fn get_conversation_mut(&mut self, conversation_id: &str) -> Option<&mut Conversation>;
}

impl ApplicationState for ApplicationStateInner {
    fn insert_conversation(&mut self, conversation: Conversation) {
        self.conversations
            .insert(conversation.id.clone(), conversation);
    }

    // should return a result
    fn insert_message(&mut self, conversation_id: &str, message: Message) {
        let is_active = {
            if let Some(convo) = self.get_current_conversation() {
                convo.id == conversation_id
            } else {
                false
            }
        };
        if let Some(convo) = self.conversations.get_mut(conversation_id) {
            self.observers
                .iter_mut()
                .for_each(|o| o.on_message(&message, conversation_id, is_active));
            convo.insert_message(message);
        }
    }

    // should return a result
    fn set_current_conversation(&mut self, conversation_id: &str) {
        if let Some(convo) = self.conversations.get(conversation_id) {
            self.current_conversation = Some(conversation_id.to_string());
            self.observers
                .iter_mut()
                .for_each(|o| o.on_conversation_change(convo));
        }
    }

    fn get_current_conversation(&self) -> Option<&Conversation> {
        if let Some(id) = &self.current_conversation {
            if let Some(convo) = self.conversations.get(id) {
                return Some(convo);
            }
        }
        None
    }

    fn set_conversations(&mut self, conversations: Vec<Conversation>) {
        self.observers
            .iter_mut()
            .for_each(|o| o.on_conversations_added(conversations.as_slice()));

        for convo in conversations.into_iter() {
            self.conversations.insert(convo.id.clone(), convo);
        }
    }

    fn get_conversations(&self) -> Conversations<Values<'_, String, Conversation>> {
        Conversations {
            inner: self.conversations.values(),
        }
    }

    fn register_observer(&mut self, observer: Box<dyn StateObserver>) {
        self.observers.push(observer)
    }

    fn get_conversation(&self, conversation_id: &str) -> Option<&Conversation> {
        self.conversations.get(conversation_id)
    }

    fn get_conversation_mut(&mut self, conversation_id: &str) -> Option<&mut Conversation> {
        self.conversations.get_mut(conversation_id)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::types::{Channel, KeybaseConversation, MemberType};
    use std::cell::RefCell;
    use std::rc::Rc;

    struct TestObserver {
        conversation_change_called: bool,
        conversations_added_called: bool,
        message_called: bool,
    }

    impl StateObserver for Rc<RefCell<TestObserver>> {
        fn on_conversation_change(&mut self, _: &Conversation) {
            self.borrow_mut().conversation_change_called = true;
        }

        fn on_conversations_added(&mut self, _: &[Conversation]) {
            self.borrow_mut().conversations_added_called = true;
        }

        fn on_message(&mut self, _: &Message, _: &str, _: bool) {
            self.borrow_mut().message_called = true;
        }
    }

    impl Default for TestObserver {
        fn default() -> Self {
            TestObserver {
                conversation_change_called: false,
                conversations_added_called: false,
                message_called: false,
            }
        }
    }

    #[test]
    fn initial_state() {
        let state = ApplicationStateInner::default();

        assert!(state.get_conversations().is_empty());
        assert!(state.get_current_conversation().is_none());
        assert!(state.observers.is_empty());
    }

    #[test]
    fn set_current_convo() {
        let mut state = ApplicationStateInner::default();

        let obs = Rc::new(RefCell::new(TestObserver::default()));
        state.register_observer(Box::new(obs.clone()));

        state.insert_conversation(
            KeybaseConversation {
                id: "test".to_string(),
                unread: false,
                channel: Channel {
                    name: "My Channel".to_string(),
                    topic_name: "".to_string(),
                    members_type: MemberType::User,
                },
            }
            .into(),
        );
        state.set_current_conversation("test");

        assert_eq!(state.get_current_conversation().unwrap().id, "test");
        assert!(obs.borrow().conversation_change_called);
    }
}
