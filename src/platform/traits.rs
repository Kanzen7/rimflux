use crate::message::Message;
use iced::{Size, Task, window};

/// Lifecycle and logical-size boundary. Native adapters own physical DPI conversion.
pub trait WindowAdapter {
    fn settings(&self, size: Size) -> window::Settings;
    fn synchronize(&self, id: window::Id, size: Size, reveal: bool) -> Task<Message>;
    fn visibility(&self, id: window::Id, visible: bool) -> Task<Message> {
        window::set_mode(
            id,
            if visible {
                window::Mode::Windowed
            } else {
                window::Mode::Hidden
            },
        )
    }
    fn shutdown(&self) -> Task<Message> {
        iced::exit()
    }
}

pub struct PortableWindow;

impl WindowAdapter for PortableWindow {
    fn settings(&self, size: Size) -> window::Settings {
        window::Settings {
            size,
            decorations: true,
            resizable: false,
            position: window::Position::Centered,
            ..Default::default()
        }
    }
    fn synchronize(&self, id: window::Id, size: Size, _: bool) -> Task<Message> {
        window::resize(id, size)
    }
}
