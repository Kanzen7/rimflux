use super::{
    config::Config,
    desktop::{HideFor, ModeSelection, RuntimeVisibility, Visibility, VisibilityPolicy},
};
use crate::{
    modules::{
        clock::{ClockInfo, ClockProvider, LocalClock},
        monitoring::MonitoringInfo,
        music::{MediaCommand, MusicInfo},
        waveform::WaveformState,
    },
    ui::animations::Transition,
};
use iced::Size;
use serde::Deserialize;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum NotchMode {
    Clock,
    Music,
    Monitoring,
}

impl NotchMode {
    pub const ALL: [Self; 3] = [Self::Clock, Self::Music, Self::Monitoring];
}

impl std::fmt::Display for NotchMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Clock => "Clock",
            Self::Music => "Music",
            Self::Monitoring => "System Monitor",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotchExpansion {
    Compact,
    Expanded,
}

#[derive(Debug, Clone, Copy)]
pub enum Action {
    Select(NotchMode, NotchExpansion),
    SwitchMode(NotchMode),
    AutoMode,
    TogglePin,
    SetVisibility(VisibilityPolicy),
    ToggleFullscreenProtection,
    OpenPanel(Panel),
    ClosePanel,
    Hide(HideFor),
    Restore,
    ToggleVisibility,
    Escape,
    ToggleExpansion,
    Collapse,
    Media(MediaCommand),
    Hover(bool),
    SeekPreview(f32),
    SeekCommit,
    SeekCancel,
    WaveHover(Option<f32>, bool),
    Noop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Panel {
    Menu,
    Hide,
    Settings,
}

impl Panel {
    pub fn size(self) -> Size {
        match self {
            Self::Menu => Size::new(334.0, 356.0),
            Self::Hide => Size::new(304.0, 218.0),
            Self::Settings => Size::new(404.0, 494.0),
        }
    }
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
    pub selection: ModeSelection,
    pub visibility: Visibility,
    pub panel: Option<Panel>,
    pub display: DisplayInfo,
    pub clock: ClockInfo,
    pub music: MusicInfo,
    pub monitoring: MonitoringInfo,
    pub animation: Transition,
    pub config: Config,
    pub waveform: WaveformState,
    pub hovered: bool,
    pub now: Instant,
    last_tick: Instant,
    last_interaction: Instant,
    idle_since: Option<Instant>,
    pub seek_preview: Option<f32>,
    pub wave_hover: Option<f32>,
    seek_pending: Option<(f32, Instant)>,
}

impl AppState {
    pub fn new(config: Config, now: Instant) -> Self {
        let selection = config
            .navigation
            .mode_selection
            .unwrap_or(if config.music.auto_switch {
                ModeSelection::Auto
            } else {
                ModeSelection::Manual
            });
        let restore = config.navigation.restore_last_mode || selection == ModeSelection::Pinned;
        let mode = config
            .navigation
            .last_mode
            .filter(|_| restore)
            .or(config.navigation.startup_mode)
            .unwrap_or(config.notch.default_mode);
        let music = if config.development.demo_mode {
            MusicInfo::demo()
        } else {
            MusicInfo::default()
        };
        let waveform = WaveformState::new(&music);
        Self {
            mode,
            expansion: NotchExpansion::Compact,
            selection,
            visibility: Visibility::default(),
            panel: None,
            display: DisplayInfo::default(),
            clock: LocalClock.now(),
            music,
            monitoring: MonitoringInfo::default(),
            animation: Transition::new(dimensions(mode, NotchExpansion::Compact), now),
            config,
            waveform,
            hovered: false,
            now,
            last_tick: now,
            last_interaction: now,
            idle_since: None,
            seek_preview: None,
            wave_hover: None,
            seek_pending: None,
        }
    }

    pub fn action(&mut self, action: Action, now: Instant) {
        self.advance(now);
        self.last_interaction = now;
        match action {
            Action::Select(mode, expansion) => {
                self.choose(mode, expansion);
            }
            Action::SwitchMode(mode) => self.choose(mode, self.expansion),
            Action::AutoMode => {
                self.selection = ModeSelection::Auto;
                self.config.navigation.mode_selection = Some(self.selection);
                self.panel = None;
                let retain_paused_music =
                    self.mode == NotchMode::Music && self.music.available && !self.music.stopped;
                let next = if self.music.playing || retain_paused_music {
                    NotchMode::Music
                } else {
                    NotchMode::Clock
                };
                if next != self.mode {
                    self.set_mode(next, NotchExpansion::Compact);
                }
            }
            Action::TogglePin => {
                self.selection = if self.selection == ModeSelection::Pinned {
                    ModeSelection::Manual
                } else {
                    ModeSelection::Pinned
                };
                self.config.navigation.mode_selection = Some(self.selection);
                self.config.navigation.last_mode = Some(self.mode);
            }
            Action::SetVisibility(policy) => {
                self.config.visibility.policy = policy;
                self.panel = None;
                self.visibility.advance(now, &self.config.visibility, false);
            }
            Action::ToggleFullscreenProtection => {
                self.config.visibility.hide_fullscreen = !self.config.visibility.hide_fullscreen;
                self.visibility.advance(now, &self.config.visibility, false);
            }
            Action::OpenPanel(panel) => {
                self.cancel_seek();
                self.wave_hover = None;
                self.panel = Some(panel);
            }
            Action::ClosePanel => self.panel = None,
            Action::Hide(duration) => {
                self.panel = None;
                self.visibility.hide(duration, now);
            }
            Action::Restore => self.visibility.restore(now, &self.config.visibility),
            Action::ToggleVisibility => match self.visibility.runtime() {
                RuntimeVisibility::Peek => {
                    self.panel = None;
                    self.visibility.dismiss_peek();
                }
                RuntimeVisibility::Visible => {
                    if self.visibility.recovery_available {
                        self.panel = None;
                        self.visibility.hide(HideFor::UntilShown, now);
                    }
                }
                _ => self.visibility.restore(now, &self.config.visibility),
            },
            Action::Escape => {
                if self.panel.take().is_none() {
                    if self.visibility.runtime() == RuntimeVisibility::Peek {
                        self.visibility.dismiss_peek();
                    } else if self.config.interaction.escape_collapses {
                        self.expansion = NotchExpansion::Compact;
                    }
                }
            }
            Action::ToggleExpansion => {
                self.expansion = match self.expansion {
                    NotchExpansion::Compact => NotchExpansion::Expanded,
                    NotchExpansion::Expanded => NotchExpansion::Compact,
                }
            }
            Action::Collapse => self.expansion = NotchExpansion::Compact,
            Action::Media(command) => {
                if self.music.demo && self.music.supports(command) {
                    self.music.command(command);
                }
            }
            Action::SeekPreview(ratio) => {
                if ratio.is_finite() && self.can_seek_waveform() {
                    self.seek_preview = Some(ratio.clamp(0.0, 1.0));
                    self.waveform.lighting.drag(ratio, now);
                }
            }
            Action::SeekCommit => {
                self.waveform.lighting.release(now);
                if let Some(ratio) = self.seek_preview.take()
                    && !self.music.demo
                    && self.music.capabilities.seek
                {
                    self.seek_pending = Some((ratio, now));
                }
            }
            Action::SeekCancel => {
                self.waveform.lighting.clear();
                self.seek_preview = None;
                self.seek_pending = None;
                self.wave_hover = None;
            }
            Action::WaveHover(ratio, inside) => {
                self.hovered = inside;
                self.wave_hover = ratio.filter(|v| v.is_finite()).map(|v| v.clamp(0.0, 1.0));
            }
            Action::Noop => {}
            Action::Hover(hovered) => self.hovered = hovered,
        }
        self.waveform
            .select_track(&self.music, !self.config.appearance.reduced_motion);
        self.retarget(now);
    }

    fn choose(&mut self, mode: NotchMode, expansion: NotchExpansion) {
        if self.selection != ModeSelection::Pinned {
            self.selection = ModeSelection::Manual;
        }
        self.config.navigation.mode_selection = Some(self.selection);
        self.config.navigation.last_mode = Some(mode);
        self.panel = None;
        self.set_mode(mode, expansion);
    }

    fn set_mode(&mut self, mode: NotchMode, expansion: NotchExpansion) {
        self.mode = mode;
        self.expansion = expansion;
        self.seek_preview = None;
        self.wave_hover = None;
        self.waveform.lighting.clear();
    }

    fn retarget(&mut self, now: Instant) {
        let target = dimensions(self.mode, self.expansion);
        if target != self.animation.target {
            self.animation
                .retarget(target, now, self.config.appearance.reduced_motion);
        }
    }

    pub fn advance(&mut self, now: Instant) {
        let was_visible = self.visible();
        self.visibility.advance(
            now,
            &self.config.visibility,
            self.hovered
                || self.seek_preview.is_some()
                || (self.panel.is_some()
                    && now.saturating_duration_since(self.last_interaction)
                        < Duration::from_secs(2)),
        );
        if was_visible && !self.visible() {
            self.on_hidden();
        } else if !was_visible && self.visible() {
            self.last_interaction = now;
        }
        let elapsed = now.saturating_duration_since(self.last_tick);
        if let Some(timeline) = &self.music.timeline {
            self.music.position = timeline.elapsed(now, self.music.playing);
        } else {
            self.music.advance(elapsed);
        }
        if self.waveform_active() {
            self.advance_orb(now);
            self.waveform.advance(
                elapsed,
                self.music.playing,
                &self.config.music.waveform,
                now,
            );
        }
        if self
            .seek_pending
            .is_some_and(|(_, sent)| now.saturating_duration_since(sent) > Duration::from_secs(2))
        {
            self.seek_pending = None;
        }
        self.last_tick = now;
        self.now = now;
        self.animation.tick(now);
        if self.selection == ModeSelection::Auto
            && self.mode == NotchMode::Music
            && !self.music.demo
            && self.panel.is_none()
            && self.idle_since.is_some_and(|start| {
                now.saturating_duration_since(start).as_secs() >= self.config.music.idle_delay_secs
            })
        {
            self.mode = NotchMode::Clock;
            self.expansion = NotchExpansion::Compact;
            self.animation.retarget(
                dimensions(self.mode, self.expansion),
                now,
                self.config.appearance.reduced_motion,
            );
            self.idle_since = None;
        }
        if self.config.notch.auto_collapse
            && self.visible()
            && self.panel.is_none()
            && !self.hovered
            && self.expansion == NotchExpansion::Expanded
            && self.seek_preview.is_none()
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
        self.visible()
            && self.panel.is_none()
            && self.mode == NotchMode::Music
            && self.music.available
            && self.waveform_enabled()
            && !self.config.appearance.reduced_motion
            && (self.music.playing
                || self.waveform.morphing()
                || self.waveform.orb.is_animating()
                || self.waveform.audio.needs_redraw()
                || self.waveform.lighting.needs_redraw(self.now))
    }
    fn advance_orb(&mut self, now: Instant) {
        self.waveform.orb.update(
            now,
            self.waveform.audio.snapshot(),
            &self.config.music.waveform.beat_orb,
            self.music.playing,
            self.waveform.lighting.dragging
                || self.waveform.lighting.seek_energy(now, false) > 0.02,
            self.config.appearance.reduced_motion,
        );
    }
    pub fn receive_audio(&mut self, snapshot: crate::services::audio::AudioSnapshot, now: Instant) {
        if self.wants_audio_capture() {
            self.waveform.audio.receive(snapshot);
            self.advance_orb(now);
        }
    }
    pub fn frame_interval(&self) -> Option<Duration> {
        if !self.visible() || self.panel.is_some() {
            None
        } else if self.animation.active {
            Some(Duration::from_millis(16))
        } else if self.waveform_active() {
            Some(Duration::from_millis(
                if self.waveform.orb.is_animating()
                    || (self.config.music.waveform.reactivity.enabled
                        && self.config.music.waveform.reactivity.micro_ripples
                        && self.waveform.audio.reactivity.ripple_count() > 0)
                {
                    16
                } else {
                    33
                },
            ))
        } else {
            None
        }
    }
    pub fn size(&self) -> Size {
        self.panel
            .map_or_else(|| self.animation.size(self.now), Panel::size)
    }

    pub fn visible(&self) -> bool {
        if self.panel == Some(Panel::Settings) && self.visibility.recovery_available {
            self.visibility.allows_settings(&self.config.visibility)
        } else {
            self.visibility.runtime().visible()
        }
    }

    pub fn on_hidden(&mut self) {
        self.hovered = false;
        self.panel = None;
        self.wave_hover = None;
        self.cancel_seek();
    }

    pub fn apply_config(&mut self, config: Config, now: Instant) {
        let selection = config.navigation.mode_selection.unwrap_or(self.selection);
        if selection != self.selection && selection == ModeSelection::Auto {
            self.action(Action::AutoMode, now);
        }
        self.selection = selection;
        if config.appearance.reduced_motion {
            self.animation
                .retarget(dimensions(self.mode, self.expansion), now, true);
            self.waveform.audio.clear();
        }
        self.config = config;
        self.panel = None;
        self.advance(now);
    }
    pub fn receive_media(&mut self, music: MusicInfo, now: Instant) {
        let changed = self
            .waveform
            .select_track(&music, !self.config.appearance.reduced_motion);
        if changed || !music.playing {
            self.waveform.audio.clear();
        }
        if changed || !music.available || !music.capabilities.seek {
            self.waveform.lighting.clear();
            self.seek_preview = None;
            self.seek_pending = None;
            self.wave_hover = None;
        } else if self
            .seek_pending
            .is_some_and(|(ratio, _)| (music.progress() - ratio).abs() < 0.02)
        {
            self.seek_pending = None;
        }
        let starts_playing = music.playing && (!self.music.playing || !self.music.available);
        let idle = !music.available || music.stopped;
        let same_session = self.music.generation == music.generation;
        let busy = same_session && self.music.busy;
        let error = self.music.error.clone().filter(|_| same_session);
        self.music = music;
        self.music.busy = busy;
        if self.music.error.is_none() {
            self.music.error = error;
        }
        if starts_playing && self.selection == ModeSelection::Auto && self.mode != NotchMode::Music
        {
            self.set_mode(NotchMode::Music, NotchExpansion::Compact);
            self.retarget(now);
        }
        if idle {
            self.idle_since.get_or_insert(now);
        } else {
            self.idle_since = None;
        }
    }
    pub fn waveform_enabled(&self) -> bool {
        self.config.music.enable_waveform && self.config.music.waveform.enabled
    }
    pub fn wants_audio_capture(&self) -> bool {
        cfg!(target_os = "windows")
            && self.visible()
            && self.panel.is_none()
            && self.mode == NotchMode::Music
            && self.music.available
            && self.music.playing
            && !self.music.demo
            && self.waveform_enabled()
            && !self.config.appearance.reduced_motion
            && self.config.music.waveform.live_audio
            && self.config.music.waveform.mode != super::config::WaveformMode::Procedural
    }
    pub fn can_seek_waveform(&self) -> bool {
        self.waveform_enabled()
            && self.config.music.waveform.interactive_seek
            && self.music.supports(MediaCommand::Seek(0.0))
            && self.panel.is_none()
            && !self.animation.active
    }
    pub fn waveform_progress(&self) -> f32 {
        self.seek_preview
            .or(self.seek_pending.map(|(ratio, _)| ratio))
            .unwrap_or_else(|| self.music.progress())
    }
    pub fn cancel_seek(&mut self) {
        self.waveform.lighting.clear();
        self.seek_preview = None;
        self.seek_pending = None;
    }
}

pub fn dimensions(mode: NotchMode, expansion: NotchExpansion) -> Size {
    match (mode, expansion) {
        (NotchMode::Clock, NotchExpansion::Compact) => Size::new(116.0, 27.0),
        (NotchMode::Clock, NotchExpansion::Expanded) => Size::new(320.0, 86.0),
        (NotchMode::Music, NotchExpansion::Compact) => Size::new(345.0, 42.0),
        (NotchMode::Music, NotchExpansion::Expanded) => Size::new(456.0, 129.0),
        (NotchMode::Monitoring, NotchExpansion::Compact) => Size::new(278.0, 40.0),
        (NotchMode::Monitoring, NotchExpansion::Expanded) => Size::new(445.0, 122.0),
    }
}
