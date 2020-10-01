// # ui.rs
//
// Contains the main UI struct and all the views that don't exist in their own module.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use cursive::{event::*, view::*, views::*, Cursive};
use dirs::config_dir;
use log::debug;
use tokio::sync::mpsc::{self, Receiver, Sender};

use crate::state::StateObserver;
use crate::types::{Conversation, Message, MessageType, UiMessage};
use crate::views::conversation::{ConversationName, ConversationView};

pub struct UiBuilder {
    cursive: Cursive,
}

impl UiBuilder {
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

        UiBuilder { cursive: siv }
    }

    pub fn build(mut self) -> (Rc<RefCell<Ui>>, Receiver<UiMessage>) {
        let (ui_send, ui_recv) = mpsc::channel(32);
        let executor = UiExecutor {
            sender: ui_send,
        };

        self.cursive.set_user_data(executor.clone());

        (
            Rc::new(RefCell::new(Ui {
                cursive: self.cursive,
                executor,
            })),
            ui_recv,
        )
    }
}

pub struct Ui {
    // Cursive (Rust TUI library object)
    cursive: Cursive,
    executor: UiExecutor,
}

impl Ui {
    // render one 'frame'
    pub fn step(&mut self) -> bool {
        if !self.cursive.is_running() {
            return false;
        }

        self.cursive.step();

        true
    }

    fn render_conversation_list(&mut self, data: &[Conversation]) {
        self.cursive
            .call_on_id("conversation_list", |view: &mut ListView| {
                view.clear();
                for convo in data.iter() {
                    debug!("Adding child: {}", &convo.get_name());
                    view.add_child("", conversation_view(convo.clone()))
                }
            });
        self.cursive.refresh();
    }

    fn render_conversation(&mut self, data: &Conversation) {
        self.cursive
            .call_on_id("chat_container", |view: &mut TextView| {
                view.set_content("");
                for msg in data.messages.iter().rev() {
                    render_message(view, msg);
                }
            });
        self.cursive
            .call_on_id("chat_panel", |view: &mut Panel<LinearLayout>| {
                view.set_title(data.get_name());
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

impl StateObserver for Ui {
    fn on_conversation_change(&mut self, data: &Conversation) {
        self.render_conversation(data);
        self.cursive.focus_id("edit").unwrap();
    }

    fn on_conversations_added(&mut self, conversations: &[Conversation]) {
        self.render_conversation_list(conversations);
    }

    fn on_message(&mut self, message: &Message, conversation_id: &str, active: bool) {
        if active {
            // write the message in the chat box
            self.new_message(&message);
        } else {
            // highlight the conversation with unread messages
            self.unread_message(conversation_id);
        }
    }
}

impl StateObserver for Rc<RefCell<Ui>> {
    fn on_conversation_change(&mut self, data: &Conversation) {
        self.borrow_mut().on_conversation_change(data)
    }

    fn on_conversations_added(&mut self, conversations: &[Conversation]) {
        self.borrow_mut().on_conversations_added(conversations)
    }

    fn on_message(&mut self, message: &Message, conversation_id: &str, active: bool) {
        self.borrow_mut()
            .on_message(message, conversation_id, active)
    }
}

#[derive(Clone)]
struct UiExecutor {
    sender: Sender<UiMessage>,
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
            handle_switch
        )
        // handle pressing enter when a conversation name has focus
        .on_event_inner(
            cursive::event::Key::Enter,
            handle_switch
        )
}

fn handle_switch(v: &mut IdView<ConversationView>, e: &Event) -> Option<EventResult> {
                if let Event::Mouse {
                    event: MouseEvent::Release(MouseButton::Left),
                    ..
                } = *e
                {
                    let convo = v.conversation_id();

                    Some(EventResult::with_cb(move |s| {
                        s.with_user_data(|executor: &mut UiExecutor| {
                            let mut exec = executor.clone();
                            let c = convo.clone();
                            tokio::spawn(async move {
                                exec.sender.send(UiMessage::SwitchConversation(c)).await;
                            });
                        });
                    }))
                } else {
                    None
                }
}

fn send_chat_message(s: &mut Cursive, msg: &str) {
    if msg.is_empty() {
        return;
    }

    s.call_on_id("edit", |view: &mut EditView| view.set_content(""));
    s.with_user_data(|executor: &mut UiExecutor| {
        let mut exec = executor.clone();
        let c = msg.to_owned();
        tokio::spawn(async move {
            exec.sender.send(UiMessage::SendMessage(c)).await;
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
