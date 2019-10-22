use std::ops::Deref;

use cursive::align::HAlign;
use cursive::theme::{Color, PaletteColor};
use cursive::traits::*;
use cursive::view::{IntoBoxedView, SizeConstraint};
use cursive::views::{
    BoxView, Dialog, EditView, LinearLayout, ListView, Panel, ScrollView, TextView, ViewBox,
};
use cursive::Cursive;
use env_logger;

use client;

fn test() {
    let convos = client::list_conversations();

    let messages = client::read_conversation(&convos[0].channel, 20);

    client::send_message(
        &client::Channel {
            name: String::from("hyperyolo"),
            topic_name: String::from("bot-testing"),
            members_type: client::MemberType::Team,
        },
        "test!",
    );
}

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

fn conversation_list(convos: Vec<client::Conversation>) -> LinearLayout {
    LinearLayout::vertical().child(ListView::new().with(|list| {
        for convo in convos {
            list.add_child("", TextView::new(&convo.channel.name));
        }
    }))
}

// TODO: Make this into an implementation of View with events
fn chat_area(messages: Vec<client::Message>) -> ViewBox {
    let mut layout = LinearLayout::vertical();

    for msg in messages.iter().rev() {
        match &msg.content {
            client::MessageType::Text { text } => layout.add_child(TextView::new(&format!(
                "{}: {}",
                msg.sender.username, text.body
            ))),
            _ => {}
        }
    }
    let chat_layout = LinearLayout::vertical()
        .child(layout.scrollable())
        .child(EditView::new().on_submit(send_chat_message).with_id("edit"));

    ViewBox::new(
        BoxView::new(SizeConstraint::Full, SizeConstraint::Full, chat_layout).as_boxed_view(),
    )
}

fn main() {
    let mut builder = env_logger::Builder::from_default_env();
    builder.target(env_logger::Target::Stderr).init();

    let convos = client::list_conversations();
    let chat = client::read_conversation(&convos[0].channel, 50);

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
                    conversation_list(convos),
                ))
                .child(chat_area(chat)),
        ),
    );

    siv.run();
}
