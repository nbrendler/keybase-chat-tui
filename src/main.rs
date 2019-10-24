use std::collections::HashMap;
use std::thread;

use crossbeam::channel::unbounded;
use crossbeam::{Receiver, Sender};
use cursive::theme::{Color, PaletteColor};
use cursive::traits::*;
use cursive::view::{IntoBoxedView, SizeConstraint};
use cursive::views::{BoxView, Dialog, EditView, LinearLayout, ListView, TextView, ViewBox};
use cursive::Cursive;
use env_logger;
use log::{debug, error, info};

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

enum ClientMessage {
    FetchConversations,
    FetchMessages(client::Conversation),
    ReceiveConversations(Vec<client::Conversation>),
    ReceiveMessages(client::Conversation, Vec<client::Message>),
}

enum UiMessage {
    NewData,
}

struct Ui {
    cursive: Cursive,
    state: ApplicationState,
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
                    .child(conversation_list())
                    .child(chat_area()),
            ),
        );

        Ui {
            cursive: siv,
            state: ApplicationState::default(),
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
                            client::MemberType::Team => {
                                format!("{}#{}", &convo.channel.name, &convo.channel.topic_name)
                            }
                            client::MemberType::User => format!("{}", convo.channel.name),
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
                                client::MessageType::Text { text } => view.add_child(
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
    conversations: Vec<client::Conversation>,
    chats: HashMap<String, Vec<client::Message>>,
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

struct AsyncClient {
    sender: Sender<ClientMessage>,
}

impl AsyncClient {
    pub fn new(sender: Sender<ClientMessage>) -> Self {
        AsyncClient { sender }
    }

    pub fn fetch_conversations(&self) {
        fetch_conversations(self.sender.clone());
    }

    pub fn fetch_messages(&self, conversation: client::Conversation) {
        fetch_messages(self.sender.clone(), conversation);
    }

    pub fn listen(&self) {
        client::listen(|value| {});
    }
}

fn fetch_conversations(sender: Sender<ClientMessage>) {
    debug!("Fetching conversations");
    thread::spawn(move || {
        let convos = client::list_conversations();
        debug!("Fetched conversations");
        sender
            .send(ClientMessage::ReceiveConversations(convos))
            .unwrap()
    });
}

fn fetch_messages(sender: Sender<ClientMessage>, conversation: client::Conversation) {
    debug!("Fetching channel messages");
    thread::spawn(move || {
        let messages = client::read_conversation(&conversation, 50);
        debug!("Fetched messages for {}", &conversation.id);
        sender
            .send(ClientMessage::ReceiveMessages(conversation, messages))
            .unwrap()
    });
}

fn main() {
    let mut builder = env_logger::Builder::from_default_env();
    builder.target(env_logger::Target::Stderr).init();

    info!("Starting...");

    let (client_send, client_recv) = unbounded::<ClientMessage>();

    {
        // kick off initial data request
        client_send.send(ClientMessage::FetchConversations).unwrap();
    }

    let mut ui = Ui::new(client_send.clone());
    debug!("Created Ui");

    let client = AsyncClient::new(client_send.clone());

    client.listen();

    while ui.step() {
        loop {
            match client_recv.try_recv() {
                Ok(msg) => match msg {
                    ClientMessage::ReceiveConversations(convos) => {
                        if ui.state.current_conversation.is_none() {
                            ui.state.current_conversation = Some((convos[0]).id.clone());
                            client_send
                                .send(ClientMessage::FetchMessages(convos[0].clone()))
                                .unwrap();
                        }
                        ui.state.conversations = convos;
                        debug!("State: {:?}", &ui.state);
                        ui.ui_send.send(UiMessage::NewData).unwrap();
                    }
                    ClientMessage::ReceiveMessages(convo, messages) => {
                        ui.state.chats.insert(convo.id, messages);
                        debug!("State: {:?}", &ui.state);
                        ui.ui_send.send(UiMessage::NewData).unwrap();
                    }
                    ClientMessage::FetchConversations => client.fetch_conversations(),
                    ClientMessage::FetchMessages(conversation_id) => {
                        client.fetch_messages(conversation_id)
                    }
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
