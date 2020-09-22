use cursive::align::Align;
use cursive::direction::Direction;
use cursive::theme::ColorStyle;
use cursive::view::{View, ViewWrapper};
use cursive::{Printer, Vec2};

use crate::types::Conversation;

const MAX_NAME_LENGTH: usize = 20;

pub trait ConversationName: View {
    fn name(&self) -> String;
    fn conversation_id(&self) -> String;
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

impl ConversationName for ConversationView {
    fn name(&self) -> String {
        self.conversation.get_name()
    }

    fn conversation_id(&self) -> String {
        self.conversation.id.to_owned()
    }
}

impl<T> ConversationName for T
where
    T: ViewWrapper<V = ConversationView>,
{
    fn name(&self) -> String {
        self.with_view(|v| v.name()).unwrap()
    }

    fn conversation_id(&self) -> String {
        self.with_view(|v| v.conversation_id()).unwrap()
    }
}

impl View for ConversationView {
    fn draw(&self, printer: &Printer) {
        let name = self.name();
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
        Vec2::new((self.name().len() + 1).min(MAX_NAME_LENGTH), 1)
    }
}
