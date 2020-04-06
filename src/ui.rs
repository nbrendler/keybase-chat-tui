// # ui.rs
//
// Contains the main UI struct and all the views that don't exist in their own module.

use std::path::PathBuf;

use crossbeam::{select, unbounded, Receiver, Sender};
use cursive::{event::*, view::*, views::*, Cursive};
use dirs::config_dir;
use log::debug;

use crate::state::{ApplicationState, StateObserver};
use crate::types::{Conversation, ConversationData, Message, MessageType};
use crate::views::conversation::{ConversationView, HasConversation};

pub struct Ui {
    // Cursive (Rust TUI library object)
    pub cursive: Cursive,
    // Observer to handle state changes
    pub observer: UiObserver,
}

impl Ui {
    pub fn new() -> Self {
        let mut siv = Cursive::default();

        // load a theme from `$HOME/.config/keybase-chat-tui/theme.toml` (on linux)
        if let Some(dir) = config_dir() {
            let theme_path = PathBuf::new().join(dir).join("keybase-chat-tui/theme.toml");
            siv.load_theme_file(theme_path)
                .expect("Failed to load theme");
        }

        siv.add_layer(
            Dialog::around(
                LinearLayout::horizontal()
                    .child(conversation_list())
                    .child(chat_area()),
            )
            .title("keybase-chat-tui"),
        );

        // focus the edit view (where you type) on the initial render
        siv.focus_id("edit").unwrap();

        Ui {
            cursive: siv,
            observer: UiObserver::new(),
        }
    }

    // render one 'frame'
    pub fn step(&mut self) -> bool {
        if !self.cursive.is_running() {
            return false;
        }

        select! {
            recv(self.observer.receiver) -> msg => {
                if let Ok(value) = msg {
                    match value {
                        StateChangeEvent::ConversationsAdded(conversations) => {
                            self.render_conversation_list(conversations.as_slice());
                        }
                        StateChangeEvent::ConversationChange(title, conversation) => {
                            self.render_conversation(title.as_str(), &conversation);
                            self.cursive.focus_id("edit").unwrap();
                        }
                        StateChangeEvent::NewMessage(message, conversation_id, active) => {
                            if active {
                                // write the message in the chat box
                                self.new_message(&message);
                            } else {
                                // highlight the conversation with unread messages
                                self.unread_message(conversation_id.as_str());
                            }

                        }
                    }
                }
            },
            default => {}
        }

        self.cursive.step();

        true
    }

    fn render_conversation_list(&mut self, data: &[Conversation]) {
        self.cursive
            .call_on_id("conversation_list", |view: &mut ListView| {
                view.clear();
                for convo in data.iter() {
                    debug!("Adding child: {}", &convo.channel.name);
                    view.add_child("", conversation_view(convo.clone()))
                }
            });
        self.cursive.refresh();
    }

    fn render_conversation(&mut self, title: &str, data: &ConversationData) {
        self.cursive
            .call_on_id("chat_container", |view: &mut TextView| {
                view.set_content("");
                for msg in data.messages.iter().rev() {
                    render_message(view, msg);
                }
            });
        self.cursive
            .call_on_id("chat_panel", |view: &mut Panel<LinearLayout>| {
                view.set_title(title);
            });
        self.cursive.refresh();
    }

    fn new_message(&mut self, message: &Message) {
        self.cursive
            .call_on_id("chat_container", |view: &mut TextView| {
                render_message(view, message);
            });
        self.cursive.refresh();
    }

    fn unread_message(&mut self, conversation_id: &str) {
        self.cursive
            .call_on_id(conversation_id, |view: &mut ConversationView| {
                view.unread = true;
            });
        self.cursive.refresh();
    }
}

// TODO: move this into a new view that inherits from TextView so we can color the username.
fn render_message(view: &mut TextView, message: &Message) {
    match &message.content {
        MessageType::Text { text } => {
            view.append(&format!("{}: {}\n", message.sender.username, text.body));
        }
        MessageType::Unfurl {} => {
            view.append(&format!(
                "{} sent an Unfurl and I don't know how to render it\n",
                message.sender.username
            ));
        }
        _ => {}
    }
}

pub enum StateChangeEvent {
    ConversationChange(String, ConversationData),
    ConversationsAdded(Vec<Conversation>),
    NewMessage(Message, String, bool),
}

#[derive(Clone)]
pub struct UiObserver {
    pub sender: Sender<StateChangeEvent>,
    receiver: Receiver<StateChangeEvent>,
}

impl UiObserver {
    fn new() -> Self {
        let (send, recv) = unbounded::<StateChangeEvent>();
        UiObserver {
            sender: send,
            receiver: recv,
        }
    }
}

impl StateObserver for UiObserver {
    fn on_conversation_change(&mut self, title: &str, data: &ConversationData) {
        self.sender
            .send(StateChangeEvent::ConversationChange(
                title.to_owned(),
                data.clone(),
            ))
            .unwrap();
    }

    fn on_conversations_added(&mut self, conversations: &[Conversation]) {
        self.sender
            .send(StateChangeEvent::ConversationsAdded(
                // is this allocating?
                conversations.to_owned(),
            ))
            .unwrap();
    }

    fn on_message(&mut self, message: &Message, conversation_id: &str, active: bool) {
        self.sender
            .send(StateChangeEvent::NewMessage(
                message.clone(),
                conversation_id.to_owned(),
                active,
            ))
            .unwrap();
    }
}

// helper to create the view of available conversations on the left. Should probably go to its own
// module.
fn conversation_view(convo: Conversation) -> impl View {
    let id = convo.id.clone();
    let view = ConversationView::new(convo).with_id(id);
    OnEventView::new(view)
        // handle left clicking on a conversation name
        .on_event_inner(
            EventTrigger::mouse(),
            |v: &mut IdView<ConversationView>, e: &Event| {
                if let Event::Mouse {
                    event: MouseEvent::Release(MouseButton::Left),
                    ..
                } = *e
                {
                    let convo = v.conversation();

                    Some(EventResult::with_cb(move |s| {
                        s.with_user_data(|state: &mut ApplicationState| {
                            state.with_data(|state_mut| {
                                state_mut.switch_to_conversation(&convo);
                            });
                        });
                    }))
                } else {
                    None
                }
            },
        )
        // handle pressing enter when a conversation name has focus
        .on_event_inner(
            cursive::event::Key::Enter,
            |v: &mut IdView<ConversationView>, _e: &Event| {
                let convo = v.conversation();

                Some(EventResult::with_cb(move |s| {
                    s.with_user_data(|state: &mut ApplicationState| {
                        state.with_data(|state_mut| {
                            state_mut.switch_to_conversation(&convo);
                        });
                    });
                }))
            },
        )
}

fn send_chat_message(s: &mut Cursive, msg: &str) {
    if msg.is_empty() {
        return;
    }

    s.call_on_id("edit", |view: &mut EditView| view.set_content(""));
    s.with_user_data(|state: &mut ApplicationState| {
        state.with_data(|state_mut| {
            state_mut.send_message(msg.to_owned());
        });
    });
}

fn conversation_list() -> ViewBox {
    let convo_list =
        Panel::new(ListView::new().with_id("conversation_list")).title("Conversations");
    ViewBox::new(
        BoxView::new(SizeConstraint::Free, SizeConstraint::Full, convo_list).as_boxed_view(),
    )
}

fn chat_area() -> ViewBox {
    let mut text = TextView::new("").with_id("chat_container").scrollable();
    text.set_scroll_strategy(cursive::view::ScrollStrategy::StickToBottom);

    let chat_layout = LinearLayout::vertical()
        .child(BoxView::new(
            SizeConstraint::Full,
            SizeConstraint::Full,
            text,
        ))
        .child(EditView::new().on_submit(send_chat_message).with_id("edit"));
    let chat = Panel::new(chat_layout).with_id("chat_panel");

    ViewBox::new(BoxView::new(SizeConstraint::Full, SizeConstraint::Full, chat).as_boxed_view())
}
