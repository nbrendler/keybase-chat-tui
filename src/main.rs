// # main.rs
//
// Contains the cli and high-level orchestration of other components.

use std::io::Error;
use std::sync::{atomic::AtomicBool, atomic::Ordering, Arc};

use crossbeam::select;
use env_logger;
use log::info;
use signal_hook::SIGTERM;

mod client;
mod state;
mod types;
mod ui;
mod views;

use crate::state::ApplicationState;
use crate::types::{ApiResponse, ClientMessage, ConversationData, ListenerEvent};
use crate::ui::Ui;

fn main() -> Result<(), Error> {
    // Only enable the logging when compiling in debug mode. This makes the difference between
    // `info!` and `debug!` somewhat moot, so I'm just using them to switch between a 'normal'
    // amount of logging and 'excessive'.
    //
    if cfg!(debug_assertions) {
        let mut builder = env_logger::Builder::from_default_env();
        builder.target(env_logger::Target::Stderr).init();
    }

    info!("Starting...");

    // Keep track of whether we should exit (i.e., we got a sigterm)
    let should_terminate = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(SIGTERM, Arc::clone(&should_terminate))?;

    // The UI object has all of the cursive (rust tui library) logic.
    let mut ui = Ui::new();
    // Application state is a middleman between the UI logic and the API client.
    let mut state = ApplicationState::new();

    // Some of the UI views need access to the state to make changes -- the easiest way to do that
    // seems to be adding it here.
    ui.cursive.set_user_data(state.clone());

    let r = {
        state.with_data(|state_mut| {
            // Tell the state to notify the UI observer about state changes
            state_mut.register_observer(Box::new(ui.observer.clone()));

            let client = state_mut.get_client_mut();
            let r = client.register();
            // Initial fetch of the conversation list
            client.fetch_conversations();

            r
        })
    };

    // ## main render loop
    //
    // After handling any signals, progress the UI one 'frame' (step). This allows the UI to handle
    // any messages it got from channels, and also for the TUI library to process events and render a frame.
    while !should_terminate.load(Ordering::Relaxed) && ui.step() {
        // The state is using an Arc<Mutex<_>> so that it can be modified freely by both the UI and
        // the api threads. I tried to hide most of the ugly Mutex details in the class itself and
        // added this `with_data` method that acquires a lock and returns the mutable state object
        // in the callback.

        state.with_data(|state_mut| {
            // I don't think the client really needs to be owned by the state, but it seemed to be
            // the best way to avoid getting into any funky lifetime requirements or having yet
            // another mutex and having to get multiple locks.
            let client = state_mut.get_client_mut();

            // We also step the client to let it handle messages related to API requests &
            // responses.
            client.step();
        // process any messages we got back from the client
        select! {
            recv(r) -> msg => {
                if let Ok(value) = msg {
                    match value {
                        // List of conversations (chatrooms)
                        ClientMessage::ApiResponse(ApiResponse::ConversationList {conversations}) => {
                            // fetch some messages from the first chat so we have something to draw
                            // on the screen. The rest of them can be lazily loaded.
                            client.fetch_messages(&conversations[0], 20);

                            // Set a pointer to the current conversation. This is used to add
                            // context in places where the API itself doesn't include the
                            // conversation info in the response.
                            state_mut.set_current_conversation(&conversations[0].id);

                            // Write the list of conversations we got back to the state (triggers a
                            // UI update via the observer).
                            state_mut.set_conversations(conversations);
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
                                let conversation_id = &messages[0].msg.conversation_id.clone();
                                
                                // Add this conversation data to the state.
                                // TODO: The naming could be better here. Randomly switching
                                // between Chat, Conversation, ConversationList, ConversationData,
                                // etc.
                                state_mut.add_chat(conversation_id, ConversationData::new(messages.into_iter().map(|m| m.msg).collect()));
                            }
                        },

                        // Fired when you send a message. Right now we don't need to do anything at
                        // this level, it's just listed for completeness of the match.
                        ClientMessage::ApiResponse(ApiResponse::MessageSent{..}) => {},

                        // We got a message from the API listener -- someone sent a chat message
                        // that we didn't request from the API!
                        ClientMessage::ListenerEvent(ListenerEvent::ChatMessage(msg)) => {
                            let conversation_id = &msg.msg.conversation_id;
                            state_mut.add_chat_message(conversation_id, msg.msg.clone());
                        }
                    }

                }
            },
            // if this is omitted, the above match will block until it gets a message (not what we
            // want inside the render loop).
            default => {},
        }
        });
    }

    // Cleanup the client after receiving a signal to stop -- this is important so we don't leave
    // zombie keybase processes.
    state.with_data(|state_mut| state_mut.get_client_mut().close())?;
    Ok(())
}
