use std::collections::HashMap;
use std::io::Error;
use std::sync::{atomic::AtomicBool, atomic::Ordering, Arc};

use crossbeam::channel::unbounded;
use crossbeam::{select, Receiver, Sender};
use cursive::theme::{Color, PaletteColor};
use cursive::traits::*;
use cursive::view::{IntoBoxedView, SizeConstraint};
use cursive::views::{BoxView, Dialog, EditView, LinearLayout, ListView, TextView, ViewBox};
use cursive::Cursive;
use env_logger;
use log::{debug, error, info};
use signal_hook::{iterator::Signals, SIGINT, SIGTERM};

mod client;
mod types;

use crate::client::Client;
use crate::types::{
    ApiResponse, Channel, ClientMessage, Conversation, MemberType, Message, MessageType,
};

fn send_chat_message(s: &mut Cursive, msg: &str) {
    if msg.is_empty() {
        return;
    }

    s.call_on_id("edit", |view: &mut EditView| view.set_content(""));
    s.with_user_data(|client: &mut Client| {
        client.send_message(
            &Channel {
                name: String::from("hyperyolo"),
                topic_name: String::from("bot-testing"),
                members_type: MemberType::Team,
            },
            msg,
        );
    });
}

fn conversation_list() -> ViewBox {
    let convo_list = LinearLayout::vertical().child(ListView::new().with_id("conversation_list"));
    ViewBox::new(
        BoxView::new(SizeConstraint::Free, SizeConstraint::Full, convo_list).as_boxed_view(),
    )
}

// TODO: Make this into an implementation of View with events
fn chat_area() -> ViewBox {
    let layout = LinearLayout::vertical().child(ListView::new().with_id("chat_container"));

    let chat_layout = LinearLayout::vertical()
        .child(layout.scrollable())
        .child(EditView::new().on_submit(send_chat_message).with_id("edit"));

    ViewBox::new(
        BoxView::new(SizeConstraint::Full, SizeConstraint::Full, chat_layout).as_boxed_view(),
    )
}

enum UiMessage {
    NewData,
}

struct Ui {
    cursive: Cursive,
    state: ApplicationState,
    ui_send: Sender<UiMessage>,
    ui_recv: Receiver<UiMessage>,
}

impl Ui {
    pub fn new() -> Self {
        let (ui_send, ui_recv) = unbounded();
        let mut siv = Cursive::default();

        siv.load_theme_file("assets/default_theme.toml").unwrap();
        let mut theme = siv.current_theme().clone();
        theme.palette[PaletteColor::Background] = Color::TerminalDefault;
        theme.palette[PaletteColor::View] = Color::TerminalDefault;

        siv.set_theme(theme);

        siv.add_layer(
            Dialog::new().content(
                LinearLayout::horizontal()
                    .child(conversation_list())
                    .child(chat_area()),
            ),
        );

        Ui {
            cursive: siv,
            state: ApplicationState::default(),
            ui_send,
            ui_recv,
        }
    }

    pub fn step(&mut self) -> bool {
        if !self.cursive.is_running() {
            return false;
        }

        loop {
            match self.ui_recv.try_recv() {
                Ok(msg) => match msg {
                    UiMessage::NewData => {
                        debug!("rendering!");
                        self.render();
                    }
                },
                Err(crossbeam::TryRecvError::Empty) => {
                    break;
                }
                Err(e) => {
                    error!("{}", e);
                    break;
                }
            }
        }

        self.cursive.step();

        true
    }

    fn render(&mut self) {
        let state = &self.state;
        self.cursive
            .call_on_id("conversation_list", |view: &mut ListView| {
                view.clear();
                for convo in state.conversations.iter() {
                    debug!("Adding child: {}", &convo.channel.name);
                    view.add_child(
                        "",
                        TextView::new(match &convo.channel.members_type {
                            MemberType::Team => {
                                format!("{}#{}", &convo.channel.name, &convo.channel.topic_name)
                            }
                            MemberType::User => format!("{}", convo.channel.name),
                        }),
                    )
                }
            });
        if let Some(ref convo_id) = state.current_conversation {
            if let Some(messages) = state.chats.get(convo_id) {
                self.cursive
                    .call_on_id("chat_container", |view: &mut ListView| {
                        view.clear();
                        for msg in messages.iter().rev() {
                            match &msg.content {
                                MessageType::Text { text } => view.add_child(
                                    "",
                                    TextView::new(&format!(
                                        "{}: {}",
                                        msg.sender.username, text.body
                                    )),
                                ),
                                _ => {}
                            }
                        }
                    });
            }
        }
        self.cursive.refresh();
    }
}

#[derive(Debug)]
struct ApplicationState {
    current_conversation: Option<String>,
    conversations: Vec<Conversation>,
    chats: HashMap<String, Vec<Message>>,
}

impl Default for ApplicationState {
    fn default() -> Self {
        ApplicationState {
            current_conversation: None,
            conversations: vec![],
            chats: HashMap::new(),
        }
    }
}

fn main() -> Result<(), Error> {
    let mut builder = env_logger::Builder::from_default_env();
    builder.target(env_logger::Target::Stderr).init();

    info!("Starting...");

    let should_terminate = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(SIGTERM, Arc::clone(&should_terminate))?;

    let mut ui = Ui::new();
    debug!("Created Ui");

    let mut client = Client::new();
    let r = client.register();
    client.fetch_conversations();

    ui.cursive.set_user_data(client);

    while !should_terminate.load(Ordering::Relaxed) && ui.step() {
        let _client = ui.cursive.user_data::<Client>().unwrap();
        _client.step();
        select! {
            recv(r) -> msg => {
                if let Ok(value) = msg {
                    match value {
                        ClientMessage::ApiResponse(ApiResponse::ConversationList {conversations}) => {
                            if ui.state.current_conversation.is_none() {
                                ui.state.current_conversation = Some((conversations[0]).id.clone());
                                _client.fetch_messages(&conversations[0], 20);
                            }
                            ui.state.conversations = conversations;
                            debug!("State: {:?}", &ui.state);
                            ui.ui_send.send(UiMessage::NewData).unwrap();
                        }
                        ClientMessage::ApiResponse(ApiResponse::MessageList{messages, ..}) => {
                            ui.state.chats.insert(messages[0].msg.conversation_id.clone(), messages.into_iter().map(|m| m.msg).collect());
                            debug!("State: {:?}", &ui.state);
                            ui.ui_send.send(UiMessage::NewData).unwrap();
                        },
                        ClientMessage::ApiResponse(_) => {
                            debug!("unhandled event");
                        },
                        ClientMessage::ListenerEvent => {
                            debug!("listener event");
                        }
                    }
                }
            },
            default => {},
        }
    }

    let _client = ui.cursive.user_data::<Client>().unwrap();
    _client.close();
    Ok(())
}
