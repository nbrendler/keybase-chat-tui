use cursive::align::Align;
use cursive::direction::Direction;
use cursive::theme::ColorStyle;
use cursive::view::{View, ViewWrapper};
use cursive::{Printer, Vec2};

use crate::types::{Conversation, MemberType};

const MAX_NAME_LENGTH: usize = 20;

pub trait HasConversation: View {
    fn conversation(&self) -> Conversation;
}

pub struct ConversationView {
    conversation: Conversation,
    pub unread: bool,
}

impl ConversationView {
    pub fn new(convo: Conversation) -> Self {
        ConversationView {
            conversation: convo,
            unread: false,
        }
    }
}

impl HasConversation for ConversationView {
    fn conversation(&self) -> Conversation {
        self.conversation.clone()
    }
}

impl<T> HasConversation for T
where
    T: ViewWrapper<V = ConversationView>,
{
    fn conversation(&self) -> Conversation {
        self.with_view(|v| v.conversation()).unwrap()
    }
}

impl View for ConversationView {
    fn draw(&self, printer: &Printer) {
        let name = match &self.conversation.channel.members_type {
            MemberType::Team => format!(
                "{}#{}",
                &self.conversation.channel.name, &self.conversation.channel.topic_name
            ),
            // TODO: remove the username from the channel name for display
            MemberType::User => self.conversation.channel.name.to_string(),
        };
        let offset = Align::top_left().v.get_offset(1, printer.size.y);
        let printer = &printer.offset((0, offset));

        let style = if self.unread && !printer.focused {
            ColorStyle::highlight_inactive()
        } else if printer.focused {
            ColorStyle::highlight()
        } else {
            ColorStyle::primary()
        };
        printer.with_color(style, |printer| {
            if name.len() > MAX_NAME_LENGTH {
                printer.print((0, 0), &name[0..MAX_NAME_LENGTH - 4]);
                printer.print((MAX_NAME_LENGTH - 4, 0), "...");
            } else {
                printer.print((0, 0), &name);
            }
        })
    }

    fn take_focus(&mut self, _: Direction) -> bool {
        self.unread = false;
        true
    }

    fn required_size(&mut self, _: Vec2) -> Vec2 {
        Vec2::new(
            (self.conversation.channel.name.len() + 1).min(MAX_NAME_LENGTH),
            1,
        )
    }
}
