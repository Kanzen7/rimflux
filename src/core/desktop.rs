use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

use super::state::NotchMode;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ModeSelection {
    #[default]
    Auto,
    Manual,
    Pinned,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Navigation {
    pub startup_mode: Option<NotchMode>,
    pub mode_selection: Option<ModeSelection>,
    pub restore_last_mode: bool,
    pub last_mode: Option<NotchMode>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VisibilityPolicy {
    #[default]
    Smart,
    AlwaysVisible,
    DesktopOnly,
}

impl VisibilityPolicy {
    pub const ALL: [Self; 3] = [Self::Smart, Self::AlwaysVisible, Self::DesktopOnly];
}

impl std::fmt::Display for VisibilityPolicy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Smart => "Smart",
            Self::AlwaysVisible => "Always Visible",
            Self::DesktopOnly => "Desktop Only",
        })
    }
}

impl std::fmt::Display for ModeSelection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Auto => "Auto",
            Self::Manual => "Manual",
            Self::Pinned => "Pinned",
        })
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct VisibilitySettings {
    pub policy: VisibilityPolicy,
    pub hide_maximized_browsers: bool,
    pub hide_fullscreen: bool,
    pub hide_other_maximized_apps: bool,
    pub allow_temporary_peek: bool,
    pub peek_seconds: u64,
    pub hide_debounce_ms: u64,
    pub show_debounce_ms: u64,
}

impl Default for VisibilitySettings {
    fn default() -> Self {
        Self {
            policy: VisibilityPolicy::Smart,
            hide_maximized_browsers: true,
            hide_fullscreen: true,
            hide_other_maximized_apps: false,
            allow_temporary_peek: true,
            peek_seconds: 8,
            hide_debounce_ms: 200,
            show_debounce_ms: 350,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct InteractionSettings {
    pub global_shortcut: String,
    pub compact_click_expands: bool,
    pub right_click_context_menu: bool,
    pub escape_collapses: bool,
}

impl Default for InteractionSettings {
    fn default() -> Self {
        Self {
            global_shortcut: "Ctrl+Alt+R".into(),
            compact_click_expands: true,
            right_click_context_menu: true,
            escape_collapses: true,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct General {
    pub start_with_windows: bool,
    pub keep_running_in_tray: bool,
}

impl Default for General {
    fn default() -> Self {
        Self {
            start_with_windows: false,
            keep_running_in_tray: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shortcut {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub key: u32,
}

impl Shortcut {
    pub fn parse(text: &str) -> Option<Self> {
        let mut shortcut = Self {
            ctrl: false,
            alt: false,
            shift: false,
            key: 0,
        };
        let mut parts = text.split('+').map(str::trim).peekable();
        while let Some(part) = parts.next() {
            let part = part.to_ascii_uppercase();
            if parts.peek().is_some() {
                let flag = match part.as_str() {
                    "CTRL" => &mut shortcut.ctrl,
                    "ALT" => &mut shortcut.alt,
                    "SHIFT" => &mut shortcut.shift,
                    _ => return None,
                };
                if *flag {
                    return None;
                }
                *flag = true;
            } else {
                shortcut.key = match part.as_bytes() {
                    [key @ (b'A'..=b'Z' | b'0'..=b'9')] => u32::from(*key),
                    _ => {
                        let function = part.strip_prefix('F')?.parse::<u32>().ok()?;
                        if !(1..=11).contains(&function) {
                            return None;
                        }
                        0x6F + function
                    }
                };
            }
        }
        (shortcut.key != 0 && (shortcut.ctrl || shortcut.alt)).then_some(shortcut)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Foreground {
    #[default]
    Desktop,
    Window,
    MaximizedBrowser,
    Maximized,
    Fullscreen,
}

pub fn classify_window(executable: &str, maximized: bool, fullscreen: bool) -> Foreground {
    if fullscreen {
        Foreground::Fullscreen
    } else if !maximized {
        Foreground::Window
    } else if [
        "chrome.exe",
        "msedge.exe",
        "firefox.exe",
        "brave.exe",
        "opera.exe",
        "opera_gx.exe",
        "vivaldi.exe",
    ]
    .iter()
    .any(|browser| executable.eq_ignore_ascii_case(browser))
    {
        Foreground::MaximizedBrowser
    } else {
        Foreground::Maximized
    }
}

pub fn covers_monitor(
    bounds: [i32; 4],
    monitor: [i32; 4],
    maximized: bool,
    decorated: bool,
) -> bool {
    (!maximized || !decorated)
        && bounds
            .iter()
            .zip(monitor)
            .all(|(edge, target)| edge.abs_diff(target) <= 3)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoHide {
    Fullscreen,
    MaximizedBrowser,
    MaximizedWindow,
    AwayFromDesktop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HideFor {
    Minutes15,
    Hour,
    UntilShown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeVisibility {
    Visible,
    Automatic(AutoHide),
    Manual { until: Option<Instant> },
    Peek,
}

impl RuntimeVisibility {
    pub fn visible(self) -> bool {
        matches!(self, Self::Visible | Self::Peek)
    }
}

#[derive(Debug, Default)]
pub struct Visibility {
    pub foreground: Foreground,
    pub recovery_available: bool,
    confirmed: Option<AutoHide>,
    pending: Option<(Option<AutoHide>, Instant)>,
    manual: Option<Option<Instant>>,
    peek_until: Option<Instant>,
}

impl Visibility {
    fn blocker(&self, settings: &VisibilitySettings) -> Option<AutoHide> {
        if settings.hide_fullscreen && self.foreground == Foreground::Fullscreen {
            return Some(AutoHide::Fullscreen);
        }
        match (settings.policy, self.foreground) {
            (VisibilityPolicy::DesktopOnly, foreground) if foreground != Foreground::Desktop => {
                Some(AutoHide::AwayFromDesktop)
            }
            (VisibilityPolicy::Smart, Foreground::MaximizedBrowser)
                if settings.hide_maximized_browsers =>
            {
                Some(AutoHide::MaximizedBrowser)
            }
            (VisibilityPolicy::Smart, Foreground::Maximized)
                if settings.hide_other_maximized_apps =>
            {
                Some(AutoHide::MaximizedWindow)
            }
            _ => None,
        }
    }

    pub fn seed(&mut self, settings: &VisibilitySettings) {
        self.confirmed = self.blocker(settings);
        self.pending = None;
    }

    pub fn advance(&mut self, now: Instant, settings: &VisibilitySettings, interacting: bool) {
        let candidate = self.blocker(settings);
        if candidate == self.confirmed {
            self.pending = None;
        } else {
            let delay = if candidate.is_some() {
                settings.hide_debounce_ms
            } else {
                settings.show_debounce_ms
            };
            let (_, since) = *self.pending.get_or_insert((candidate, now));
            if self
                .pending
                .is_some_and(|(previous, _)| previous != candidate)
            {
                self.pending = Some((candidate, now));
            } else if now.saturating_duration_since(since) >= Duration::from_millis(delay) {
                self.confirmed = candidate;
                self.pending = None;
            }
        }

        if self.manual.flatten().is_some_and(|until| now >= until) {
            self.manual = None;
            if candidate == Some(AutoHide::Fullscreen) {
                self.confirmed = candidate;
                self.pending = None;
            }
        }
        if !settings.allow_temporary_peek || candidate == Some(AutoHide::Fullscreen) {
            self.peek_until = None;
        } else if let Some(until) = self.peek_until {
            if interacting {
                self.peek_until = Some(now + Duration::from_secs(settings.peek_seconds));
            } else if now >= until {
                self.peek_until = None;
            }
        }
    }

    pub fn runtime(&self) -> RuntimeVisibility {
        if !self.recovery_available {
            return RuntimeVisibility::Visible;
        }
        if let Some(until) = self.manual {
            RuntimeVisibility::Manual { until }
        } else if self.confirmed == Some(AutoHide::Fullscreen) {
            RuntimeVisibility::Automatic(AutoHide::Fullscreen)
        } else if self.peek_until.is_some() {
            RuntimeVisibility::Peek
        } else if let Some(reason) = self.confirmed {
            RuntimeVisibility::Automatic(reason)
        } else {
            RuntimeVisibility::Visible
        }
    }

    pub fn needs_debounce_tick(&self) -> bool {
        self.pending.is_some()
    }

    pub fn allows_settings(&self, settings: &VisibilitySettings) -> bool {
        self.manual.is_none()
            && self.confirmed != Some(AutoHide::Fullscreen)
            && self.blocker(settings) != Some(AutoHide::Fullscreen)
    }

    pub fn hide(&mut self, duration: HideFor, now: Instant) {
        if self.recovery_available {
            let until = match duration {
                HideFor::Minutes15 => Some(now + Duration::from_secs(15 * 60)),
                HideFor::Hour => Some(now + Duration::from_secs(60 * 60)),
                HideFor::UntilShown => None,
            };
            self.manual = Some(until);
            self.peek_until = None;
        }
    }

    pub fn restore(&mut self, now: Instant, settings: &VisibilitySettings) {
        let blocker = self.blocker(settings);
        if self.manual.take().is_some() && blocker == Some(AutoHide::Fullscreen) {
            self.confirmed = blocker;
            self.pending = None;
        }
        if settings.allow_temporary_peek
            && blocker != Some(AutoHide::Fullscreen)
            && self.confirmed != Some(AutoHide::Fullscreen)
            && (blocker.is_some() || self.confirmed.is_some())
        {
            self.peek_until = Some(now + Duration::from_secs(settings.peek_seconds));
        }
    }

    pub fn dismiss_peek(&mut self) {
        self.peek_until = None;
    }
}
