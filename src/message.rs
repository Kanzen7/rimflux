use crate::{
    core::state::{Action, DisplayInfo},
    modules::monitoring::SystemSnapshot,
};
use std::time::Instant;

#[derive(Debug, Clone)]
pub enum Message {
    Action(Action),
    Frame(Instant),
    Second(Instant),
    Sample,
    Metrics(Result<SystemSnapshot, String>),
    WindowReady(iced::window::Id),
    Display(DisplayInfo),
    WindowEvent(iced::window::Event),
    Keyboard(iced::keyboard::Event),
    Quit,
}
