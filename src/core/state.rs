use super::config::Config;
use crate::{
    modules::{
        clock::{ClockInfo, ClockProvider, LocalClock},
        monitoring::MonitoringInfo,
        music::{MediaCommand, MusicInfo},
    },
    services::media::{MediaProvider, MockMedia},
    ui::animations::Transition,
};
use iced::Size;
use serde::Deserialize;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NotchMode {
    Clock,
    Music,
    Monitoring,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotchExpansion {
    Compact,
    Expanded,
}

#[derive(Debug, Clone, Copy)]
pub enum Action {
    Select(NotchMode, NotchExpansion),
    ToggleExpansion,
    Collapse,
    Media(MediaCommand),
    More,
    Hover(bool),
}

#[derive(Debug, Clone, Copy, Default)]
pub struct DisplayInfo {
    pub width: f32,
    pub height: f32,
    pub scale: f32,
}

pub struct AppState {
    pub mode: NotchMode,
    pub expansion: NotchExpansion,
    pub display: DisplayInfo,
    pub clock: ClockInfo,
    pub music: MusicInfo,
    pub monitoring: MonitoringInfo,
    pub animation: Transition,
    pub config: Config,
    pub phase: f32,
    pub more: bool,
    pub hovered: bool,
    pub now: Instant,
    last_tick: Instant,
    last_interaction: Instant,
}

impl AppState {
    pub fn new(config: Config, now: Instant) -> Self {
        let mode = config.notch.default_mode;
        let mut music = MusicInfo::default();
        if !config.development.demo_mode {
            music.available = false;
            music.playing = false;
        }
        Self {
            mode,
            expansion: NotchExpansion::Compact,
            display: DisplayInfo::default(),
            clock: LocalClock.now(),
            music,
            monitoring: MonitoringInfo::default(),
            animation: Transition::new(dimensions(mode, NotchExpansion::Compact), now),
            config,
            phase: 0.0,
            more: false,
            hovered: false,
            now,
            last_tick: now,
            last_interaction: now,
        }
    }

    pub fn action(&mut self, action: Action, now: Instant) {
        self.advance(now);
        self.last_interaction = now;
        match action {
            Action::Select(mode, expansion) => {
                self.mode = mode;
                self.expansion = expansion;
                self.more = false;
            }
            Action::ToggleExpansion => {
                self.expansion = match self.expansion {
                    NotchExpansion::Compact => NotchExpansion::Expanded,
                    NotchExpansion::Expanded => NotchExpansion::Compact,
                }
            }
            Action::Collapse => self.expansion = NotchExpansion::Compact,
            Action::Media(command) => MockMedia.command(&mut self.music, command),
            Action::More => self.more = !self.more,
            Action::Hover(hovered) => self.hovered = hovered,
        }
        let target = dimensions(self.mode, self.expansion);
        if target != self.animation.target {
            self.animation
                .retarget(target, now, self.config.appearance.reduced_motion);
        }
    }

    pub fn advance(&mut self, now: Instant) {
        let elapsed = now.saturating_duration_since(self.last_tick);
        MockMedia.advance(&mut self.music, elapsed);
        if self.waveform_active() {
            self.phase =
                (self.phase + elapsed.as_secs_f32() * 1.6) % (std::f32::consts::TAU * 100.0);
        }
        self.last_tick = now;
        self.now = now;
        self.animation.tick(now);
        if self.config.notch.auto_collapse
            && !self.hovered
            && self.expansion == NotchExpansion::Expanded
            && now.saturating_duration_since(self.last_interaction) >= Duration::from_secs(8)
        {
            self.expansion = NotchExpansion::Compact;
            self.animation.retarget(
                dimensions(self.mode, self.expansion),
                now,
                self.config.appearance.reduced_motion,
            );
        }
    }

    pub fn waveform_active(&self) -> bool {
        self.mode == NotchMode::Music
            && self.music.available
            && self.music.playing
            && self.config.music.enable_waveform
            && !self.config.appearance.reduced_motion
    }
    pub fn frame_interval(&self) -> Option<Duration> {
        if self.animation.active || self.waveform_active() {
            Some(Duration::from_millis(33))
        } else {
            None
        }
    }
    pub fn size(&self) -> Size {
        self.animation.size(self.now)
    }
}

pub fn dimensions(mode: NotchMode, expansion: NotchExpansion) -> Size {
    match (mode, expansion) {
        (NotchMode::Clock, NotchExpansion::Compact) => Size::new(116.0, 27.0),
        (NotchMode::Clock, NotchExpansion::Expanded) => Size::new(320.0, 86.0),
        (NotchMode::Music, NotchExpansion::Compact) => Size::new(345.0, 42.0),
        (NotchMode::Music, NotchExpansion::Expanded) => Size::new(456.0, 113.0),
        (NotchMode::Monitoring, NotchExpansion::Compact) => Size::new(278.0, 40.0),
        (NotchMode::Monitoring, NotchExpansion::Expanded) => Size::new(445.0, 122.0),
    }
}
