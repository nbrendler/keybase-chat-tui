use std::io::Error;
use std::sync::{atomic::AtomicBool, atomic::Ordering, Arc};

use crossbeam::select;
// use cursive::theme::{Color, PaletteColor};
use env_logger;
use log::{debug, info};
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
    let mut builder = env_logger::Builder::from_default_env();
    builder.target(env_logger::Target::Stderr).init();

    info!("Starting...");

    let should_terminate = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(SIGTERM, Arc::clone(&should_terminate))?;

    let mut ui = Ui::new();
    debug!("Created Ui");

    let mut state = ApplicationState::new();

    ui.cursive.set_user_data(state.clone());

    let r = {
        state.with_data(|state_mut| {
            state_mut.register_observer(Box::new(ui.observer.clone()));

            let client = state_mut.get_client_mut();
            let r = client.register();
            client.fetch_conversations();

            r
        })
    };

    while !should_terminate.load(Ordering::Relaxed) && ui.step() {
        state.with_data(|state_mut| {
            let client = state_mut.get_client_mut();
            client.step();
        select! {
            recv(r) -> msg => {
                if let Ok(value) = msg {
                    match value {
                        ClientMessage::ApiResponse(ApiResponse::ConversationList {conversations}) => {
                            client.fetch_messages(&conversations[0], 20);
                            state_mut.set_current_conversation(&conversations[0].id);
                            state_mut.set_conversations(conversations);
                        }
                        ClientMessage::ApiResponse(ApiResponse::MessageList{messages}) => {
                            if !messages.is_empty() {
                                let conversation_id = &messages[0].msg.conversation_id.clone();
                                state_mut.add_chat(conversation_id, ConversationData::new(messages.into_iter().map(|m| m.msg).collect()));
                            }
                        },
                        ClientMessage::ApiResponse(ApiResponse::MessageSent{..}) => {},
                        ClientMessage::ListenerEvent(ListenerEvent::ChatMessage(msg)) => {
                            let conversation_id = &msg.msg.conversation_id;
                            state_mut.add_chat_message(conversation_id, msg.msg.clone());
                        }
                    }

                }
            },
            default => {},
        }
        });
    }

    state.with_data(|state_mut| state_mut.get_client_mut().close())?;
    Ok(())
}
