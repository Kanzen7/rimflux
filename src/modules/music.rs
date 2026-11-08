use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Repeat {
    Off,
    All,
    One,
}

#[derive(Debug, Clone)]
pub struct MusicInfo {
    pub title: &'static str,
    pub artist: &'static str,
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
            title: "Autumn",
            artist: "NIKI",
            duration: 228.0,
            position: 84.0,
            playing: true,
            shuffle: false,
            repeat: Repeat::Off,
            track: 0,
            available: true,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum MediaCommand {
    PlayPause,
    Previous,
    Next,
    Shuffle,
    Repeat,
    Seek(f32),
}

impl MusicInfo {
    pub fn progress(&self) -> f32 {
        (self.position / self.duration).clamp(0.0, 1.0)
    }
    pub fn advance(&mut self, elapsed: Duration) {
        if !self.playing || !self.available {
            return;
        }
        self.position += elapsed.as_secs_f32();
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
        (self.title, self.artist, self.duration) = match self.track {
            1 => ("Every Summertime", "NIKI", 215.0),
            2 => ("Oceans & Engines", "NIKI", 336.0),
            _ => ("Autumn", "NIKI", 228.0),
        };
        self.position = 0.0;
    }
}

pub fn timestamp(seconds: f32) -> String {
    let seconds = seconds.max(0.0) as u32;
    format!("{:02}:{:02}", seconds / 60, seconds % 60)
}
