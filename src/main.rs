use crossbeam::channel::unbounded;
use crossbeam::{Receiver, Sender};
use cursive::align::HAlign;
use cursive::theme::{Color, PaletteColor};
use cursive::traits::*;
use cursive::view::{IntoBoxedView, SizeConstraint};
use cursive::views::{
    BoxView, Dialog, EditView, LinearLayout, ListView, Panel, ScrollView, TextView, ViewBox,
};
use cursive::Cursive;
use std::thread;
#[macro_use]
use log::{info, error};
use env_logger;

use client;

fn send_chat_message(s: &mut Cursive, msg: &str) {
    if msg.is_empty() {
        return;
    }

    s.call_on_id("edit", |view: &mut EditView| view.set_content(""));

    client::send_message(
        &client::Channel {
            name: String::from("hyperyolo"),
            topic_name: String::from("bot-testing"),
            members_type: client::MemberType::Team,
        },
        msg,
    );
}

fn conversation_list() -> LinearLayout {
    LinearLayout::vertical().child(ListView::new().with_id("conversation_list"))
}

fn set_messages(s: &mut Cursive, messages: Vec<client::Message>) {
    s.call_on_id("chat_container", |view: &mut ListView| {
        view.clear();
        for msg in messages.iter().rev() {
            match &msg.content {
                client::MessageType::Text { text } => view.add_child(
                    &text.body,
                    TextView::new(&format!("{}: {}", msg.sender.username, text.body)),
                ),
                _ => {}
            }
        }
    });
}

// TODO: Make this into an implementation of View with events
fn chat_area() -> ViewBox {
    let mut layout = LinearLayout::vertical().child(ListView::new().with_id("chat_container"));

    let chat_layout = LinearLayout::vertical()
        .child(layout.scrollable())
        .child(EditView::new().on_submit(send_chat_message).with_id("edit"));

    ViewBox::new(
        BoxView::new(SizeConstraint::Full, SizeConstraint::Full, chat_layout).as_boxed_view(),
    )
}

enum ClientMessage {
    FetchConversations,
    FetchMessages(client::Channel),
    ReceiveConversations(Vec<client::Conversation>),
    ReceiveMessages(client::Channel, Vec<client::Message>),
}

enum UiMessage {
    NewData,
}

struct Ui {
    cursive: Cursive,
    ui_send: Sender<UiMessage>,
    ui_recv: Receiver<UiMessage>,
    client_send: Sender<ClientMessage>,
}

impl Ui {
    pub fn new(client_send: Sender<ClientMessage>) -> Self {
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
                    .child(BoxView::new(
                        SizeConstraint::Free,
                        SizeConstraint::Full,
                        conversation_list(),
                    ))
                    .child(chat_area()),
            ),
        );
        Ui {
            cursive: siv,
            ui_send,
            ui_recv,
            client_send,
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
                        info!("A thing happened");
                        info!("{:?}", self.cursive.user_data::<ApplicationState>());
                        // re-render
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
}

fn fetch_conversations(sender: Sender<ClientMessage>) {
    thread::spawn(move || {
        let convos = client::list_conversations();
        sender
            .send(ClientMessage::ReceiveConversations(convos))
            .unwrap()
    });
}

#[derive(Debug)]
struct ApplicationState {
    conversations: Vec<client::Conversation>,
}

fn main() {
    let mut builder = env_logger::Builder::from_default_env();
    builder.target(env_logger::Target::Stderr).init();

    let (client_send, client_recv) = unbounded::<ClientMessage>();

    fetch_conversations(client_send.clone());

    let mut ui = Ui::new(client_send);

    while ui.step() {
        loop {
            match client_recv.try_recv() {
                Ok(msg) => match msg {
                    ClientMessage::ReceiveConversations(convos) => {
                        ui.cursive
                            .set_user_data::<ApplicationState>(ApplicationState {
                                conversations: convos,
                            });
                        ui.ui_send.send(UiMessage::NewData).unwrap();
                    }
                    _ => {}
                },
                Err(crossbeam::TryRecvError::Empty) => {
                    break;
                }
                Err(e) => {
                    error!("{}", e);
                }
            }
        }
    }
}
