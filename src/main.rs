use cursive::align::HAlign;
use cursive::theme::{Color, PaletteColor};
use cursive::traits::*;
use cursive::view::SizeConstraint;
use cursive::views::{BoxView, Dialog, LinearLayout, ListView, Panel, ScrollView, TextView};
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

fn conversation_list(convos: Vec<client::Conversation>) -> LinearLayout {
    LinearLayout::vertical()
        //        .child(TextView::new("Conversations").h_align(HAlign::Center))
        .child(Panel::new(ListView::new().with(|list| {
            for convo in convos {
                list.add_child("", TextView::new(&convo.channel.name));
            }
        })))
}

fn chat_area(messages: Vec<client::Message>) -> LinearLayout {
    let mut layout = LinearLayout::vertical();
    //.child(TextView::new("Chat").h_align(HAlign::Center));

    for msg in messages.iter().rev() {
        match &msg.content {
            client::MessageType::Text { text } => layout.add_child(TextView::new(&format!(
                "{}: {}",
                msg.sender.username, text.body
            ))),
            _ => {}
        }
    }

    layout
}

fn main() {
    env_logger::init();

    let convos = client::list_conversations();
    let chat = client::read_conversation(&convos[0].channel, 50);

    let mut siv = Cursive::default();

    siv.load_theme_file("assets/default_theme.toml").unwrap();
    let mut theme = siv.current_theme().clone();
    theme.palette[PaletteColor::Background] = Color::TerminalDefault;
    theme.palette[PaletteColor::View] = Color::TerminalDefault;

    siv.set_theme(theme);

    siv.add_global_callback('q', Cursive::quit);

    siv.add_layer(
        Dialog::new().content(
            LinearLayout::horizontal()
                .child(BoxView::new(
                    SizeConstraint::Free,
                    SizeConstraint::Full,
                    conversation_list(convos),
                ))
                .child(
                    BoxView::new(SizeConstraint::Full, SizeConstraint::Full, chat_area(chat))
                        .scrollable()
                        .with_id("chat"),
                ),
        ),
    );

    siv.run();
}
