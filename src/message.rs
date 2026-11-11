use crate::{
    core::state::{Action, DisplayInfo},
    modules::monitoring::SystemSnapshot,
};
use std::time::Instant;

#[derive(Debug, Clone)]
pub enum Message {
    Action(Action),
    Frame(Instant),
    Progress(Instant),
    Second(Instant),
    Sample,
    Metrics(Result<SystemSnapshot, String>),
    WindowReady(iced::window::Id),
    Display(DisplayInfo),
    WindowEvent(iced::window::Event),
    Keyboard(iced::keyboard::Event),
    Quit,
    SettingsEdit(crate::ui::desktop::SettingsEdit),
    SaveSettings,
    PanelHover(bool),
    AudioSnapshot(crate::services::audio::AudioSnapshot),
    MediaSnapshot(crate::modules::music::MusicInfo),
    MediaCommandResult(Result<(), String>),
    #[cfg(target_os = "windows")]
    MediaConnected(std::sync::mpsc::SyncSender<crate::platform::windows_media::WorkerRequest>),
    #[cfg(target_os = "windows")]
    DesktopConnected(crate::platform::windows_desktop::DesktopHandle),
    #[cfg(target_os = "windows")]
    DesktopStatus(crate::platform::windows_desktop::DesktopStatus),
    #[cfg(target_os = "windows")]
    DesktopAction(Action),
    #[cfg(target_os = "windows")]
    DesktopStopped,
}
