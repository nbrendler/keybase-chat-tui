use cursive::align::Align;
use cursive::direction::Direction;
use cursive::view::View;
use cursive::{Printer, Vec2};

use crate::types::{Conversation, MemberType};

pub struct ConversationView {
    conversation: Conversation,
}

impl ConversationView {
    pub fn new(convo: Conversation) -> Self {
        ConversationView {
            conversation: convo,
        }
    }

    pub fn conversation(&self) -> Conversation {
        self.conversation.clone()
    }
}

impl View for ConversationView {
    fn draw(&self, printer: &Printer) {
        let name = match &self.conversation.channel.members_type {
            MemberType::Team => format!(
                "{}#{}",
                &self.conversation.channel.name, &self.conversation.channel.topic_name
            ),
            MemberType::User => self.conversation.channel.name.to_string(),
        };
        let offset = Align::top_left().v.get_offset(1, printer.size.y);
        let printer = &printer.offset((0, offset));

        printer.print((0, 0), &name);
    }

    fn take_focus(&mut self, _: Direction) -> bool {
        true
    }
    fn required_size(&mut self, _: Vec2) -> Vec2 {
        Vec2::new(self.conversation.channel.name.len() + 1, 1)
    }
}
