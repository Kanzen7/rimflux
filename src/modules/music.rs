use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, Default)]
pub struct Capabilities {
    pub play: bool,
    pub pause: bool,
    pub toggle: bool,
    pub previous: bool,
    pub next: bool,
    pub seek: bool,
    pub shuffle: bool,
    pub repeat: bool,
}

#[derive(Debug, Clone)]
pub struct PlaybackTimeline {
    pub start: f64,
    pub end: f64,
    pub position: f64,
    pub min_seek: f64,
    pub max_seek: f64,
    pub rate: f64,
    pub captured_at: Instant,
}
pub type Timeline = PlaybackTimeline;
impl PlaybackTimeline {
    pub fn duration(&self) -> f32 {
        (self.end - self.start).max(0.0) as f32
    }
    pub fn elapsed(&self, now: Instant, playing: bool) -> f32 {
        let delta = if playing {
            now.saturating_duration_since(self.captured_at)
                .as_secs_f64()
                * self.rate
        } else {
            0.0
        };
        (self.position + delta - self.start).clamp(0.0, self.duration() as f64) as f32
    }
    pub fn seek_ticks(&self, ratio: f32) -> Option<i64> {
        if !ratio.is_finite() || self.end <= self.start || self.max_seek <= self.min_seek {
            return None;
        }
        let low = self.min_seek.max(self.start);
        let high = self.max_seek.min(self.end);
        if !low.is_finite() || !high.is_finite() || high < low {
            return None;
        }
        let position =
            (self.start + (self.end - self.start) * ratio.clamp(0.0, 1.0) as f64).clamp(low, high);
        Some((position * 10_000_000.0).round() as i64)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Repeat {
    Off,
    All,
    One,
}

#[derive(Debug, Clone)]
pub struct MusicInfo {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub source: String,
    pub artwork: Option<iced::widget::image::Handle>,
    pub capabilities: Capabilities,
    pub timeline: Option<Timeline>,
    pub generation: u64,
    pub demo: bool,
    pub stopped: bool,
    pub error: Option<String>,
    pub busy: bool,
    pub duration: f32,
    pub position: f32,
    pub playing: bool,
    pub shuffle: bool,
    pub repeat: Repeat,
    pub track: usize,
    pub available: bool,
}

impl Default for MusicInfo {
    fn default() -> Self {
        Self {
            title: String::new(),
            artist: String::new(),
            album: String::new(),
            source: String::new(),
            artwork: None,
            capabilities: Capabilities::default(),
            timeline: None,
            generation: 0,
            demo: false,
            stopped: true,
            error: None,
            busy: false,
            duration: 0.0,
            position: 0.0,
            playing: false,
            shuffle: false,
            repeat: Repeat::Off,
            track: 0,
            available: false,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum MediaCommand {
    Play,
    Pause,
    PlayPause,
    Previous,
    Next,
    Shuffle,
    Repeat,
    Seek(f32),
}

impl MusicInfo {
    pub fn demo() -> Self {
        Self {
            title: "Autumn".into(),
            artist: "NIKI".into(),
            album: "Nicole".into(),
            source: "DEMO".into(),
            duration: 228.0,
            position: 84.0,
            playing: true,
            available: true,
            demo: true,
            stopped: false,
            capabilities: Capabilities {
                play: true,
                pause: true,
                toggle: true,
                previous: true,
                next: true,
                seek: true,
                shuffle: true,
                repeat: true,
            },
            ..Self::default()
        }
    }
    pub fn supports(&self, command: MediaCommand) -> bool {
        if !self.available || self.busy {
            return false;
        }
        match command {
            MediaCommand::Play => self.capabilities.play,
            MediaCommand::Pause => self.capabilities.pause,
            MediaCommand::PlayPause => {
                self.capabilities.toggle
                    || (if self.playing {
                        self.capabilities.pause
                    } else {
                        self.capabilities.play
                    })
            }
            MediaCommand::Previous => self.capabilities.previous,
            MediaCommand::Next => self.capabilities.next,
            MediaCommand::Shuffle => self.capabilities.shuffle,
            MediaCommand::Repeat => self.capabilities.repeat,
            MediaCommand::Seek(ratio) => {
                self.capabilities.seek && ratio.is_finite() && self.duration > 0.0
            }
        }
    }
    pub fn progress(&self) -> f32 {
        if self.duration > 0.0 {
            (self.position / self.duration).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }
    pub fn advance(&mut self, elapsed: Duration) {
        if !self.playing || !self.available {
            return;
        }
        self.position += elapsed.as_secs_f32();
        if !self.demo {
            self.position = self.position.min(self.duration);
            return;
        }
        if self.position >= self.duration {
            let overflow = self.position - self.duration;
            if self.repeat == Repeat::One {
                self.position %= self.duration;
            } else {
                self.select_track(1);
                self.position = overflow % self.duration;
            }
        }
    }

    pub fn command(&mut self, command: MediaCommand) {
        if !self.available {
            return;
        }
        match command {
            MediaCommand::Play => self.playing = true,
            MediaCommand::Pause => self.playing = false,
            MediaCommand::PlayPause => self.playing = !self.playing,
            MediaCommand::Previous => self.select_track(-1),
            MediaCommand::Next => self.select_track(1),
            MediaCommand::Shuffle => self.shuffle = !self.shuffle,
            MediaCommand::Repeat => {
                self.repeat = match self.repeat {
                    Repeat::Off => Repeat::All,
                    Repeat::All => Repeat::One,
                    Repeat::One => Repeat::Off,
                }
            }
            MediaCommand::Seek(ratio) => {
                if ratio.is_finite() {
                    self.position = ratio.clamp(0.0, 1.0) * self.duration;
                }
            }
        }
    }

    fn select_track(&mut self, direction: isize) {
        // A deterministic alternate order makes shuffle observable without randomness.
        let step = if self.shuffle {
            direction * 2
        } else {
            direction
        };
        self.track = (self.track as isize + step).rem_euclid(3) as usize;
        let (title, artist, duration) = match self.track {
            1 => ("Every Summertime", "NIKI", 215.0),
            2 => ("Oceans & Engines", "NIKI", 336.0),
            _ => ("Autumn", "NIKI", 228.0),
        };
        self.title = title.into();
        self.artist = artist.into();
        self.duration = duration;
        self.position = 0.0;
    }
}

pub fn timestamp(seconds: f32) -> String {
    let seconds = seconds.max(0.0) as u32;
    format!("{:02}:{:02}", seconds / 60, seconds % 60)
}
