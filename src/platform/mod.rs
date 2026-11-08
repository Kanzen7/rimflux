#[cfg(target_os = "linux")]
pub mod linux;
pub mod traits;
#[cfg(target_os = "windows")]
pub mod windows;

use crate::{
    app::Rimflux,
    core::config::Config,
    core::state::{AppState, NotchMode},
    message::Message,
};
use traits::{PortableWindow, WindowAdapter};

pub fn adapter(preview: bool) -> Box<dyn WindowAdapter> {
    #[cfg(target_os = "windows")]
    if !preview {
        return Box::new(windows::WindowsWindow {
            inspect: std::env::var_os("RIMFLUX_INSPECT").is_some(),
        });
    }
    #[cfg(target_os = "linux")]
    if !preview {
        return Box::new(linux::LayerWindow);
    }
    let _ = preview;
    Box::new(PortableWindow)
}

pub fn run(config: Config, preview: bool) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_os = "linux")]
    if !preview {
        match linux::layer_shell_supported() {
            Ok(true) => return linux::run(config),
            Ok(false) => tracing::warn!(
                "Compositor does not advertise zwlr_layer_shell_v1; using a decorated preview. Try KDE, Sway, or Hyprland for native top anchoring."
            ),
            Err(error) => {
                tracing::warn!(%error, "Native Wayland connection unavailable; using decorated preview")
            }
        }
        return run_portable(config, true);
    }
    run_portable(config, preview || !cfg!(target_os = "windows"))
}

fn run_portable(config: Config, preview: bool) -> Result<(), Box<dyn std::error::Error>> {
    let initial = AppState::new(config.clone(), std::time::Instant::now()).size();
    let settings = adapter(preview).settings(initial);
    tracing::info!(
        preview,
        "Starting Rimflux; focus and press 1–5 to inspect states, Ctrl+Q to quit"
    );
    iced::application(
        move || Rimflux::boot(config.clone(), preview, false),
        Rimflux::update,
        Rimflux::view,
    )
    .title(if preview {
        "Rimflux · Preview · 1–5 switch states"
    } else {
        "Rimflux"
    })
    .window(settings)
    .theme(iced::Theme::Dark)
    .style(move |_: &Rimflux, _: &iced::Theme| iced::theme::Style {
        background_color: if preview {
            crate::ui::theme::SURFACE
        } else {
            iced::Color::TRANSPARENT
        },
        text_color: crate::ui::theme::TEXT,
    })
    .subscription(Rimflux::subscription)
    .antialiasing(true)
    .run()?;
    Ok(())
}

pub fn shortcut(
    key: &iced::keyboard::Key,
    modifiers: iced::keyboard::Modifiers,
    demo: bool,
) -> Option<Message> {
    use crate::core::state::{Action, NotchExpansion::*};
    use iced::keyboard::{Key, key::Named};
    match key {
        Key::Character(value) if modifiers.control() && value.eq_ignore_ascii_case("q") => {
            Some(Message::Quit)
        }
        Key::Named(Named::Escape) => Some(Message::Action(Action::Collapse)),
        Key::Named(Named::Enter) => Some(Message::Action(Action::ToggleExpansion)),
        Key::Named(Named::Space) => Some(Message::Action(Action::Media(
            crate::modules::music::MediaCommand::PlayPause,
        ))),
        Key::Character(value) if demo => {
            let (mode, expansion) = match value.as_str() {
                "1" => (NotchMode::Clock, Compact),
                "2" => (NotchMode::Music, Compact),
                "3" => (NotchMode::Music, Expanded),
                "4" => (NotchMode::Monitoring, Compact),
                "5" => (NotchMode::Monitoring, Expanded),
                _ => return None,
            };
            Some(Message::Action(Action::Select(mode, expansion)))
        }
        _ => None,
    }
}
