use crate::{
    core::{
        config::Config,
        desktop::{HideFor, ModeSelection, RuntimeVisibility, VisibilityPolicy},
        state::{Action, AppState, NotchMode, Panel},
    },
    message::Message,
};
use iced::{
    Element, Length, Theme,
    widget::{
        button, column, container, pick_list, row, scrollable, svg, text, text_input, toggler,
    },
};
use std::sync::OnceLock;

use super::theme::{BASE, CYAN, ELEVATED, SECONDARY, TEXT};

#[derive(Debug, Clone)]
pub enum SettingsEdit {
    Startup(NotchMode),
    Selection(ModeSelection),
    RestoreLast(bool),
    StartupWithWindows(bool),
    Tray(bool),
    Policy(VisibilityPolicy),
    Browsers(bool),
    Fullscreen(bool),
    Maximized(bool),
    Peek(bool),
    Shortcut(String),
    ReducedMotion(bool),
}

impl SettingsEdit {
    pub fn apply(self, config: &mut Config) {
        match self {
            Self::Startup(mode) => config.navigation.startup_mode = Some(mode),
            Self::Selection(selection) => config.navigation.mode_selection = Some(selection),
            Self::RestoreLast(value) => config.navigation.restore_last_mode = value,
            Self::StartupWithWindows(value) => config.general.start_with_windows = value,
            Self::Tray(value) => config.general.keep_running_in_tray = value,
            Self::Policy(policy) => config.visibility.policy = policy,
            Self::Browsers(value) => config.visibility.hide_maximized_browsers = value,
            Self::Fullscreen(value) => config.visibility.hide_fullscreen = value,
            Self::Maximized(value) => config.visibility.hide_other_maximized_apps = value,
            Self::Peek(value) => config.visibility.allow_temporary_peek = value,
            Self::Shortcut(value) => config.interaction.global_shortcut = value,
            Self::ReducedMotion(value) => config.appearance.reduced_motion = value,
        }
    }
}

fn button_style(_: &Theme, status: button::Status) -> button::Style {
    let color = match status {
        button::Status::Hovered => ELEVATED,
        button::Status::Pressed => iced::Color::from_rgb8(45, 45, 56),
        _ => BASE,
    };
    button::Style {
        background: Some(color.into()),
        text_color: if status == button::Status::Disabled {
            SECONDARY
        } else {
            TEXT
        },
        border: iced::border::rounded(7),
        ..Default::default()
    }
}

fn item<'a>(
    label: impl Into<String>,
    message: impl Into<Option<Message>>,
) -> button::Button<'a, Message> {
    button(text(label.into()).size(13))
        .width(Length::Fill)
        .padding([5, 9])
        .style(button_style)
        .on_press_maybe(message.into())
}

fn mark(selected: bool, title: &str) -> String {
    format!("{}  {title}", if selected { "✓" } else { " " })
}

fn mode_icon(mode: NotchMode) -> svg::Handle {
    static ICONS: OnceLock<[svg::Handle; 3]> = OnceLock::new();
    let icons = ICONS.get_or_init(|| {
        [
            r#"<circle cx="12" cy="12" r="8"/><path d="M12 8v4l3 2"/>"#,
            r#"<path d="M10 17V6l8-2v11M10 9l8-2"/><ellipse cx="7" cy="17" rx="3" ry="2"/><ellipse cx="15" cy="15" rx="3" ry="2"/>"#,
            r#"<path d="M3 12h4l3-7 4 14 3-7h4"/>"#,
        ].map(|path| svg::Handle::from_memory(format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><g fill="none" stroke="white" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round">{path}</g></svg>"#
        ).into_bytes()))
    });
    icons[match mode {
        NotchMode::Clock => 0,
        NotchMode::Music => 1,
        NotchMode::Monitoring => 2,
    }]
    .clone()
}

fn menu(state: &AppState) -> Element<'_, Message> {
    let mut modes = column![].spacing(1);
    for (index, mode) in NotchMode::ALL.into_iter().enumerate() {
        let selected = mode == state.mode;
        let icon = svg(mode_icon(mode))
            .width(17)
            .height(17)
            .opacity(if selected { 1.0_f32 } else { 0.62 });
        modes = modes.push(
            button(
                row![
                    icon,
                    text(format!("{mode}"))
                        .size(13)
                        .color(if selected { TEXT } else { SECONDARY }),
                    iced::widget::space::horizontal(),
                    text(if selected {
                        "✓".to_owned()
                    } else {
                        (index + 1).to_string()
                    })
                    .size(11)
                    .color(if selected { CYAN } else { SECONDARY }),
                ]
                .spacing(9)
                .align_y(iced::Alignment::Center),
            )
            .padding([5, 9])
            .width(Length::Fill)
            .style(button_style)
            .on_press(Message::Action(Action::SwitchMode(mode))),
        );
    }
    let recovery = state.visibility.recovery_available;
    let hide = if state.visibility.runtime() == RuntimeVisibility::Peek {
        "Dismiss peek"
    } else {
        "Hide Rimflux"
    };
    column![
        modes,
        item(
            mark(state.selection == ModeSelection::Auto, "Auto Mode  ·  A"),
            Message::Action(Action::AutoMode)
        ),
        item(
            mark(
                state.selection == ModeSelection::Pinned,
                "Pin Current Mode  ·  P"
            ),
            Message::Action(Action::TogglePin)
        ),
        item(
            format!("{hide}  ·  H"),
            recovery.then_some(Message::Action(Action::ToggleVisibility))
        ),
        item(
            "Hide Temporarily…  ·  T",
            recovery.then_some(Message::Action(Action::OpenPanel(Panel::Hide)))
        ),
        item(
            mark(
                state.config.visibility.hide_fullscreen,
                "Auto-hide in Fullscreen  ·  F"
            ),
            recovery.then_some(Message::Action(Action::ToggleFullscreenProtection))
        ),
        item(
            "Settings…  ·  S",
            Message::Action(Action::OpenPanel(Panel::Settings))
        ),
        item("Quit Rimflux  ·  Q", Message::Quit),
    ]
    .spacing(1)
    .into()
}

fn hide_menu() -> Element<'static, Message> {
    column![
        item(
            "15 minutes  ·  1",
            Message::Action(Action::Hide(HideFor::Minutes15))
        ),
        item("1 hour  ·  2", Message::Action(Action::Hide(HideFor::Hour))),
        item(
            "Until manually shown  ·  3",
            Message::Action(Action::Hide(HideFor::UntilShown))
        ),
        item("Back", Message::Action(Action::OpenPanel(Panel::Menu))),
    ]
    .spacing(4)
    .into()
}

fn toggle(
    label: &'static str,
    value: bool,
    enabled: bool,
    edit: fn(bool) -> SettingsEdit,
) -> Element<'static, Message> {
    let toggle = toggler(value).label(label).size(18).text_size(13);
    if enabled {
        toggle
            .on_toggle(move |value| Message::SettingsEdit(edit(value)))
            .into()
    } else {
        toggle.into()
    }
}

fn settings<'a>(config: &'a Config, state: &AppState, native: bool) -> Element<'a, Message> {
    let general = &config.general;
    let navigation = &config.navigation;
    let visibility = &config.visibility;
    let title = |name| text(name).size(14).color(TEXT);
    let description = |label| text(label).size(12).color(SECONDARY);
    let form = column![
        title("General"),
        toggle(
            "Start with Windows",
            general.start_with_windows,
            native,
            SettingsEdit::StartupWithWindows
        ),
        toggle(
            "Keep running in tray",
            general.keep_running_in_tray,
            native,
            SettingsEdit::Tray
        ),
        description("Default startup mode"),
        pick_list(
            NotchMode::ALL,
            Some(navigation.startup_mode.unwrap_or(config.notch.default_mode)),
            |mode| Message::SettingsEdit(SettingsEdit::Startup(mode)),
        )
        .width(Length::Fill)
        .text_size(13),
        toggle(
            "Restore last selected mode",
            navigation.restore_last_mode,
            true,
            SettingsEdit::RestoreLast
        ),
        title("Navigation"),
        pick_list(
            [
                ModeSelection::Auto,
                ModeSelection::Manual,
                ModeSelection::Pinned
            ],
            Some(navigation.mode_selection.unwrap_or(state.selection)),
            |selection| Message::SettingsEdit(SettingsEdit::Selection(selection)),
        )
        .width(Length::Fill)
        .text_size(13),
        description("Global show / hide shortcut"),
        text_input("Ctrl+Alt+R", &config.interaction.global_shortcut)
            .on_input_maybe(
                native.then_some(|value| Message::SettingsEdit(SettingsEdit::Shortcut(value)))
            )
            .size(13),
        title("Visibility"),
        pick_list(VisibilityPolicy::ALL, Some(visibility.policy), |policy| {
            Message::SettingsEdit(SettingsEdit::Policy(policy))
        },)
        .width(Length::Fill)
        .text_size(13),
        toggle(
            "Hide over maximized browsers",
            visibility.hide_maximized_browsers,
            native,
            SettingsEdit::Browsers
        ),
        toggle(
            "Hide over other maximized apps",
            visibility.hide_other_maximized_apps,
            native,
            SettingsEdit::Maximized
        ),
        toggle(
            "Hide over fullscreen apps",
            visibility.hide_fullscreen,
            native,
            SettingsEdit::Fullscreen
        ),
        toggle(
            "Allow temporary peek",
            visibility.allow_temporary_peek,
            native,
            SettingsEdit::Peek
        ),
        title("Appearance"),
        toggle(
            "Reduced motion",
            config.appearance.reduced_motion,
            true,
            SettingsEdit::ReducedMotion
        ),
        description("Dark theme · Dynamic accent"),
    ]
    .spacing(9)
    .padding(iced::Padding {
        right: 11.0,
        ..iced::Padding::ZERO
    });
    scrollable(form).height(Length::Fill).into()
}

pub fn shortcut(
    panel: Panel,
    key: &iced::keyboard::Key,
    modifiers: iced::keyboard::Modifiers,
) -> Option<Message> {
    use iced::keyboard::{Key, key::Named};
    if matches!(key, Key::Named(Named::Escape)) {
        return Some(Message::Action(Action::Escape));
    }
    if modifiers.control()
        && matches!(key, Key::Character(value) if value.eq_ignore_ascii_case("q"))
    {
        return Some(Message::Quit);
    }
    if panel == Panel::Settings {
        return (modifiers.control() && matches!(key, Key::Named(Named::Enter)))
            .then_some(Message::SaveSettings);
    }
    if modifiers.control() || modifiers.alt() || modifiers.logo() {
        return None;
    }
    let Key::Character(key) = key else {
        return None;
    };
    let action = match (panel, key.to_ascii_lowercase().as_str()) {
        (Panel::Menu, "1") => Action::SwitchMode(NotchMode::Clock),
        (Panel::Menu, "2") => Action::SwitchMode(NotchMode::Music),
        (Panel::Menu, "3") => Action::SwitchMode(NotchMode::Monitoring),
        (Panel::Menu, "a") => Action::AutoMode,
        (Panel::Menu, "p") => Action::TogglePin,
        (Panel::Menu, "h") => Action::ToggleVisibility,
        (Panel::Menu, "t") => Action::OpenPanel(Panel::Hide),
        (Panel::Menu, "f") => Action::ToggleFullscreenProtection,
        (Panel::Menu, "s") => Action::OpenPanel(Panel::Settings),
        (Panel::Menu, "q") => return Some(Message::Quit),
        (Panel::Hide, "1") => Action::Hide(HideFor::Minutes15),
        (Panel::Hide, "2") => Action::Hide(HideFor::Hour),
        (Panel::Hide, "3") => Action::Hide(HideFor::UntilShown),
        _ => return None,
    };
    Some(Message::Action(action))
}

pub fn view<'a>(
    panel: Panel,
    state: &'a AppState,
    draft: Option<&'a Config>,
    notice: Option<&'a str>,
    native: bool,
) -> Element<'a, Message> {
    let label = match panel {
        Panel::Menu => "Rimflux",
        Panel::Hide => "Hide Rimflux",
        Panel::Settings => "Settings",
    };
    let header = row![
        text(label).size(17),
        iced::widget::space::horizontal(),
        button(text("Close · Esc").size(11))
            .padding([4, 6])
            .style(button_style)
            .on_press(Message::Action(Action::ClosePanel)),
    ]
    .align_y(iced::Alignment::Center);
    let mut contents = column![header].spacing(8);
    contents = contents.push(match panel {
        Panel::Menu => menu(state),
        Panel::Hide => hide_menu(),
        Panel::Settings => settings(draft.unwrap_or(&state.config), state, native),
    });
    if let Some(notice) = notice {
        contents = contents.push(text(notice).size(11).color(SECONDARY));
    }
    if panel == Panel::Settings {
        contents = contents.push(
            button(text("Save settings").size(13))
                .padding([6, 10])
                .on_press(Message::SaveSettings),
        );
    }
    container(contents)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding([13, 23])
        .style(|_| container::Style {
            background: Some(BASE.into()),
            ..Default::default()
        })
        .into()
}
