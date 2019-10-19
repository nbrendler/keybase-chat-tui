use cursive::align::HAlign;
use cursive::theme::{Color, PaletteColor};
use cursive::traits::*;
use cursive::view::SizeConstraint;
use cursive::views::{BoxView, Dialog, DummyView, LinearLayout, TextView};
use cursive::Cursive;
use env_logger;

use client::{list_conversations, read_conversation, send_message, Channel, MemberType};

fn test() {
    let convos = list_conversations();

    let messages = read_conversation(&convos[0].channel, 20);

    send_message(
        &Channel {
            name: String::from("hyperyolo"),
            topic_name: String::from("bot-testing"),
            members_type: MemberType::Team,
        },
        "test!",
    );
}

fn main() {
    env_logger::init();

    let mut siv = Cursive::default();

    siv.load_theme_file("assets/default_theme.toml").unwrap();
    let mut theme = siv.current_theme().clone();
    theme.palette[PaletteColor::Background] = Color::TerminalDefault;

    siv.set_theme(theme);

    siv.add_global_callback('q', Cursive::quit);

    siv.add_layer(
        Dialog::new()
            .title("Keybase Chat")
            .padding((0, 0, 0, 0))
            .content(
                LinearLayout::horizontal()
                    .child(BoxView::new(
                        SizeConstraint::Free,
                        SizeConstraint::Full,
                        TextView::new("Conversations").h_align(HAlign::Center),
                    ))
                    .child(BoxView::new(
                        SizeConstraint::Full,
                        SizeConstraint::Full,
                        TextView::new("Chat").h_align(HAlign::Center),
                    )),
            ),
    );

    siv.run();
}
