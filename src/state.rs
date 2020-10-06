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

#[cfg(test)]
use mockall::*;

use crate::types::{Conversation, Message};

type ConversationId = String;

// Trait that interested parties can implement (and register themselves below) to receive
// notifications when state changes. The APIs are all a little hodge-podge depending on what I
// needed to render in each case.
#[cfg_attr(test, automock)]
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
    use crate::types::*;
    use crate::types::{
        Channel, KeybaseConversation, MemberType, MessageBody, MessageType, Sender,
    };
    use crate::{conversation, message};
    use std::collections::HashSet;

    // State Tests

    #[test]
    fn initial_state() {
        let state = ApplicationStateInner::default();

        assert!(state.get_conversations().is_empty());
        assert!(state.get_current_conversation().is_none());
        assert!(state.observers.is_empty());
    }

    #[test]
    fn get_or_set_conversation() {
        let mut state = ApplicationStateInner::default();

        let test_convo: Conversation = conversation!("test").into();
        let data = test_convo.data.clone();

        state.insert_conversation(test_convo);
        let actual = state.get_conversation("test").unwrap();
        assert_eq!(actual.id, "test");
        assert_eq!(actual.data, data);

        let mut_actual = state.get_conversation_mut("test").unwrap();
        assert_eq!(mut_actual.id, "test");
        assert_eq!(mut_actual.data, data);
    }

    #[test]
    fn current_conversation() {
        let mut state = ApplicationStateInner::default();

        state.set_current_conversation("test");
        assert!(state.get_current_conversation().is_none());

        let convo: Conversation = conversation!("test").into();
        let data_copy = convo.data.clone();

        state.insert_conversation(convo);
        state.set_current_conversation("test");
        let current = state.get_current_conversation().unwrap();

        assert_eq!(current.id, "test");
        assert_eq!(current.data, data_copy);
    }

    #[test]
    fn get_or_set_whole_vec() {
        let mut state = ApplicationStateInner::default();
        let conversations = vec![conversation!("test1").into(), conversation!("test2").into()];
        let set: HashSet<Conversation> = conversations.iter().cloned().collect();

        state.set_conversations(conversations);

        for c in state.get_conversations() {
            assert!(set.contains(c));
        }
    }

    #[test]
    fn insert_message() {
        let mut state = ApplicationStateInner::default();

        state.insert_conversation(conversation!("test").into());
        state.insert_message("test", message!("test", "hey"));

        let convo = state.get_conversation("test").unwrap();

        if let MessageType::Text { text } = &convo.messages[0].content {
            assert_eq!(text.body, "hey");
        } else {
            panic!("Wrong message type");
        }

        state.insert_message("test", message!("test", "there"));
        let convo = state.get_conversation("test").unwrap();

        // message should be prepended
        if let MessageType::Text { text } = &convo.messages[0].content {
            assert_eq!(text.body, "there");
        } else {
            panic!("Wrong message type");
        }
    }

    // Observer Tests

    #[test]
    fn obs_set_current_convo() {
        let mut state = ApplicationStateInner::default();

        let test_convo: Conversation = conversation!("test").into();

        let mut obs = MockStateObserver::new();

        obs.expect_on_conversation_change()
            .withf(|convo: &Conversation| &*convo.id == "test")
            .times(1)
            .return_const(());

        state.register_observer(Box::new(obs));

        state.insert_conversation(test_convo);
        state.set_current_conversation("test");

        assert_eq!(state.get_current_conversation().unwrap().id, "test");
    }

    #[test]
    fn obs_set_conversations() {
        let mut state = ApplicationStateInner::default();
        let conversations: Vec<Conversation> =
            vec![conversation!("test1").into(), conversation!("test2").into()];

        let c = conversations.clone();

        let mut obs = MockStateObserver::new();

        obs.expect_on_conversations_added()
            .withf(move |convos: &[Conversation]| {
                convos.iter().zip(c.iter()).all(|(c1, c2)| c1 == c2)
            })
            .times(1)
            .return_const(());

        state.register_observer(Box::new(obs));
        state.set_conversations(conversations);

        assert!(state.get_current_conversation().is_none())
    }

    #[test]
    fn obs_send_message() {
        let mut state = ApplicationStateInner::default();

        let test_convo1: Conversation = conversation!("test1").into();
        let test_convo2: Conversation = conversation!("test2").into();

        let message = Message {
            conversation_id: "test1".to_string(),
            content: MessageType::Text {
                text: MessageBody {
                    body: "My Message".to_string(),
                },
            },
            channel: Channel {
                name: "My Channel".to_string(),
                topic_name: "".to_string(),
                members_type: MemberType::User,
            },
            sender: Sender {
                device_name: "My Device".to_string(),
                username: "Some Guy".to_string(),
            },
        };

        let message2 = Message {
            conversation_id: "test2".to_string(),
            content: MessageType::Text {
                text: MessageBody {
                    body: "My Message 2".to_string(),
                },
            },
            channel: Channel {
                name: "My Channel".to_string(),
                topic_name: "".to_string(),
                members_type: MemberType::User,
            },
            sender: Sender {
                device_name: "My Device".to_string(),
                username: "Some Guy".to_string(),
            },
        };

        let m1 = message.clone();

        let mut inactive_obs = MockStateObserver::new();
        inactive_obs
            .expect_on_message()
            .withf(move |msg: &Message, id: &str, active: &bool| {
                *msg == m1 && id == "test1" && !*active
            })
            .times(1)
            .return_const(());

        state.insert_conversation(test_convo1);
        state.insert_conversation(test_convo2);
        state.set_current_conversation("test2");

        state.register_observer(Box::new(inactive_obs));
        state.insert_message("test1", message);

        state.observers.clear();

        let mut active_obs = MockStateObserver::new();
        let m2 = message2.clone();
        active_obs
            .expect_on_message()
            .withf(move |msg: &Message, id: &str, active: &bool| {
                *msg == m2 && id == "test2" && *active
            })
            .times(1)
            .return_const(());

        state.register_observer(Box::new(active_obs));
        state.insert_message("test2", message2);
    }
}
