use std::collections::HashMap;
use std::io::Error;
use std::sync::{atomic::AtomicBool, atomic::Ordering, Arc, Mutex};

use crossbeam::channel::unbounded;
use crossbeam::{select, Receiver, Sender};
use cursive::event::{Event, EventResult, EventTrigger, MouseButton, MouseEvent};
use cursive::theme::{Color, PaletteColor};
use cursive::traits::*;
use cursive::view::{IntoBoxedView, SizeConstraint};
use cursive::views::{
    BoxView, Dialog, EditView, LinearLayout, ListView, OnEventView, TextView, ViewBox,
};
use cursive::Cursive;
use env_logger;
use log::{debug, error, info};
use signal_hook::SIGTERM;

mod client;
mod types;
mod views;

use crate::client::Client;
use crate::types::{ApiResponse, ClientMessage, Conversation, ListenerEvent, Message, MessageType};
use crate::views::conversation::ConversationView;

pub fn conversation_view(convo: Conversation) -> OnEventView<ConversationView> {
    let view = ConversationView::new(convo);
    OnEventView::new(view).on_event_inner(
        EventTrigger::mouse(),
        |v: &mut ConversationView, e: &Event| {
            if let &Event::Mouse {
                event: MouseEvent::Release(MouseButton::Left),
                ..
            } = e
            {
                let convo = v.conversation();

                Some(EventResult::with_cb(move |s| {
                    s.with_user_data(|_state: &mut ApplicationState| {
                        let mut state = _state.lock().unwrap();
                        let convo = convo.clone();
                        state.client.fetch_messages(&convo, 20);
                        state.data.current_conversation = Some(convo.id);
                    });
                }))
            } else {
                None
            }
        },
    )
}

fn send_chat_message(s: &mut Cursive, msg: &str) {
    if msg.is_empty() {
        return;
    }

    s.call_on_id("edit", |view: &mut EditView| view.set_content(""));
    s.with_user_data(|state: &mut ApplicationState| {
        debug!("acquiring lock");
        let s = state.lock().unwrap();
        let convo = s
            .data
            .conversations
            .iter()
            .find(|&i| &i.id == s.data.current_conversation.as_ref().unwrap())
            .expect("No current conversation");
        debug!("sending to conversation: {:?}", convo);
        s.client.send_message(&convo.channel, msg);
    });
}

fn conversation_list() -> ViewBox {
    let convo_list = LinearLayout::vertical().child(ListView::new().with_id("conversation_list"));
    ViewBox::new(
        BoxView::new(SizeConstraint::Free, SizeConstraint::Full, convo_list).as_boxed_view(),
    )
}

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
            ui_send,
            ui_recv,
        }
    }

    pub fn step(&mut self, state: ApplicationState) -> bool {
        if !self.cursive.is_running() {
            return false;
        }

        loop {
            match self.ui_recv.try_recv() {
                Ok(msg) => match msg {
                    UiMessage::NewData => {
                        debug!("rendering!");
                        let _state = state.lock().unwrap();
                        self.render(&_state.data);
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

    fn render(&mut self, data: &ApplicationData) {
        self.cursive
            .call_on_id("conversation_list", |view: &mut ListView| {
                view.clear();
                for convo in data.conversations.iter() {
                    debug!("Adding child: {}", &convo.channel.name);
                    view.add_child("", conversation_view(convo.clone()))
                }
            });
        if let Some(ref convo_id) = data.current_conversation {
            if let Some(messages) = data.chats.get(convo_id) {
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
struct ApplicationData {
    current_conversation: Option<String>,
    conversations: Vec<Conversation>,
    chats: HashMap<String, Vec<Message>>,
}

impl Default for ApplicationData {
    fn default() -> Self {
        ApplicationData {
            current_conversation: None,
            conversations: vec![],
            chats: HashMap::new(),
        }
    }
}

struct ApplicationStateInner {
    client: Client,
    data: ApplicationData,
}

type ApplicationState = Arc<Mutex<ApplicationStateInner>>;

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

    let state = Arc::new(Mutex::new(ApplicationStateInner {
        client,
        data: ApplicationData::default(),
    }));

    ui.cursive.set_user_data(Arc::clone(&state));

    while !should_terminate.load(Ordering::Relaxed) && ui.step(Arc::clone(&state)) {
        let mut state = state.lock().unwrap();
        state.client.step();
        select! {
            recv(r) -> msg => {
                if let Ok(value) = msg {
                    match value {
                        ClientMessage::ApiResponse(ApiResponse::ConversationList {conversations}) => {
                            if state.data.current_conversation.is_none() {
                                state.data.current_conversation = Some((conversations[0]).id.clone());
                                state.client.fetch_messages(&conversations[0], 20);
                            }
                            state.data.conversations = conversations;
                            ui.ui_send.send(UiMessage::NewData).unwrap();
                        }
                        ClientMessage::ApiResponse(ApiResponse::MessageList{messages, ..}) => {
                            if !messages.is_empty() {
                                state.data.chats.insert(messages[0].msg.conversation_id.clone(), messages.into_iter().map(|m| m.msg).collect());
                            }
                            ui.ui_send.send(UiMessage::NewData).unwrap();
                        },
                        ClientMessage::ApiResponse(_) => {
                            debug!("unhandled event");
                        },
                        ClientMessage::ListenerEvent(ListenerEvent::ChatMessage(msg)) => {
                            let messages = state.data.chats.get_mut(&msg.msg.conversation_id).unwrap();
                            messages.insert(0, msg.msg);
                            ui.ui_send.send(UiMessage::NewData).unwrap();
                        }
                    }
                }
            },
            default => {},
        }
    }

    state.lock().unwrap().client.close()?;
    Ok(())
}
