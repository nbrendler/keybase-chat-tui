use crossbeam::channel::{select, Receiver};

use crate::client::Client;
use crate::state::ApplicationState;
use crate::types::{ApiResponse, ClientMessage, Conversation, ListenerEvent, UiMessage};

pub struct Controller<S> {
    client: Client,
    state: S,
    client_receiver: Receiver<ClientMessage>,
    ui_receiver: Receiver<UiMessage>,
}

impl<S: ApplicationState> Controller<S> {
    pub fn new(mut client: Client, state: S, ui_receiver: Receiver<UiMessage>) -> Self {
        let r = client.register();
        Controller {
            client,
            state,
            client_receiver: r,
            ui_receiver,
        }
    }

    pub fn step(&mut self) {
        self.client.step();
        select! {
            recv(self.client_receiver) -> msg => {
                if let Ok(value) = msg {
                    match value {
                        // List of conversations (chatrooms)
                        ClientMessage::ApiResponse(ApiResponse::ConversationList {conversations}) => {
                            // fetch some messages from the first chat so we have something to draw
                            // on the screen. The rest of them can be lazily loaded.
                            self.client.fetch_messages(&conversations[0], 20);
                            let id = &conversations[0].id.clone();

                            // Write the list of conversations we got back to the state (triggers a
                            // UI update via the observer).
                            self.state.set_conversations(conversations.into_iter().map(|c| c.into()).collect::<Vec<Conversation>>());

                            // Set a pointer to the current conversation. This is used to add
                            // context in places where the API itself doesn't include the
                            // conversation info in the response.
                            self.state.set_current_conversation(id);

                        }
                        // List of messages (conversation-specific)
                        ClientMessage::ApiResponse(ApiResponse::MessageList{messages}) => {
                            if !messages.is_empty() {
                                // Figure out what conversation this is for. The API response
                                // unhelpfully doesn't include it, so we can figure it out by
                                // looking at the first message. If messages are empty, we won't be
                                // able to tell without keeping track of the requests more
                                // holistically (async/await in the client, maybe);
                                //
                                // This means that currently we don't cache requests for messages
                                // that return an empty list (a chat with no messages yet) which is
                                // a bug -- the statelessness of the client makes it impossible to
                                // know without us doing the extra bookkeeping.
                                let conversation_id = &messages[0].msg.conversation_id;

                                if let Some(convo) = self.state.get_conversation_mut(&conversation_id) {
                                    convo.insert_messages(messages.into_iter().map(|m| m.msg).collect())
                                }
                            }
                        },

                        // Fired when you send a message. Right now we don't need to do anything at
                        // this level, it's just listed for completeness of the match.
                        ClientMessage::ApiResponse(ApiResponse::MessageSent{..}) => {},

                        // We got a message from the API listener -- someone sent a chat message
                        // that we didn't request from the API!
                        ClientMessage::ListenerEvent(ListenerEvent::ChatMessage(msg)) => {
                            let conversation_id = &msg.msg.conversation_id;
                            self.state.insert_message(conversation_id, msg.msg.clone());
                        }
                    }

                }
            },
            recv(self.ui_receiver) -> msg => {
                if let Ok(value) = msg {
                    match value {
                        UiMessage::SendMessage(msg) => {
                            if let Some(convo) = self.state.get_current_conversation() {
                                let channel = &convo.data.channel;
                                self.client.send_message(channel, msg);
                            }
                        },
                        UiMessage::SwitchConversation(conversation_id) => {
                            if let Some(convo) = self.state.get_conversation(&conversation_id) {
                                if !convo.fetched {
                                    self.client.fetch_messages(&convo.data, 20);
                                }
                                self.state.set_current_conversation(&conversation_id);
                            }
                        }
                    }
                }
            },
            // if this is omitted, the above match will block until it gets a message (not what we
            // want inside the render loop).
            default => {},
        }
    }
}
