use super::music::MusicInfo;
use super::traveling_wave::{TravelingInput, TravelingWaves};
use super::waveform_reactivity::WaveformReactivity;
use crate::core::config::{WaveformConfig, WaveformMode};
use crate::services::audio::{AudioCaptureStatus, AudioSnapshot};
use std::time::{Duration, Instant};

pub const PROFILE_SAMPLES: usize = 96;

/// A stable visual envelope, not a recording or a measured complete-track waveform.
#[derive(Debug, Clone)]
pub struct WaveformProfile {
    pub seed: u64,
    pub envelope: [f32; PROFILE_SAMPLES],
}

impl WaveformProfile {
    pub fn track_seed(music: &MusicInfo) -> u64 {
        // FNV-1a is stable between runs, unlike a process-randomized hash.
        let mut hash = 0xcbf29ce484222325_u64;
        for field in [&music.source, &music.title, &music.artist, &music.album] {
            for byte in field.bytes().chain(std::iter::once(0)) {
                hash = (hash ^ byte as u64).wrapping_mul(0x100000001b3);
            }
        }
        hash
    }

    pub fn new(seed: u64) -> Self {
        let phase = (seed & 0xffff) as f32 / 65535.0 * std::f32::consts::TAU;
        let frequency = 1.2 + ((seed >> 16) & 0xff) as f32 / 255.0;
        let envelope = std::array::from_fn(|i| {
            let x = i as f32 / (PROFILE_SAMPLES - 1) as f32;
            0.82 + 0.12 * (x * std::f32::consts::TAU * frequency + phase).sin()
                + 0.06 * (x * 17.0 + phase * 0.7).sin()
        });
        Self { seed, envelope }
    }
}

/// Fresh scalar features only. No samples or native capture objects enter UI state.
#[derive(Debug, Clone)]
pub struct AudioReactiveInput {
    snapshot: Option<AudioSnapshot>,
    rms: f32,
    bass: f32,
    blend: f32,
    not_before: Instant,
    pub reactivity: WaveformReactivity,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct AudioModulation {
    pub rms: f32,
    pub bass: f32,
    pub blend: f32,
}
impl Default for AudioReactiveInput {
    fn default() -> Self {
        Self {
            snapshot: None,
            rms: 0.0,
            bass: 0.0,
            blend: 0.0,
            not_before: Instant::now(),
            reactivity: WaveformReactivity::default(),
        }
    }
}
impl AudioReactiveInput {
    pub fn receive(&mut self, mut snapshot: AudioSnapshot) {
        if snapshot.captured_at >= self.not_before {
            if snapshot.onset_at.is_some_and(|at| at < self.not_before) {
                snapshot.onset_at = None;
                snapshot.onset_strength = 0.0;
                snapshot.raw_onset_strength = 0.0;
                snapshot.low_onset = 0.0;
                snapshot.high_onset = 0.0;
            }
            for event in &mut snapshot.transients {
                if event.is_some_and(|e| e.at < self.not_before) {
                    *event = None;
                }
            }
            if snapshot.status == AudioCaptureStatus::Live {
                self.reactivity
                    .ingest(&snapshot.transients, snapshot.captured_at, self.not_before);
            }
            self.snapshot = Some(snapshot);
        }
    }
    pub fn snapshot(&self) -> Option<&AudioSnapshot> {
        self.snapshot.as_ref()
    }
    pub fn clear(&mut self) {
        if self.snapshot.take().is_some() {
            self.not_before = Instant::now();
            self.reactivity.clear_ripples();
        }
    }
    fn is_live(&self, now: Instant) -> bool {
        self.snapshot.as_ref().is_some_and(|s| {
            s.status == AudioCaptureStatus::Live
                && now.saturating_duration_since(s.captured_at) < Duration::from_millis(350)
        })
    }
    pub fn modulation(&self, _now: Instant) -> AudioModulation {
        AudioModulation {
            rms: self.rms,
            bass: self.bass,
            blend: self.blend,
        }
    }
    pub fn status_text(&self, now: Instant) -> String {
        if self.is_live(now) {
            "Live system audio · bass / mid / treble".into()
        } else {
            self.snapshot
                .as_ref()
                .and_then(|s| s.reason.clone())
                .unwrap_or_else(|| "Procedural · live audio unavailable".into())
        }
    }
    fn advance(&mut self, seconds: f32, now: Instant, playing: bool) {
        let live = playing && self.is_live(now);
        let (rms, bass) = if live {
            self.snapshot
                .as_ref()
                .map_or((0.0, 0.0), |s| (s.rms, s.bass))
        } else {
            (0.0, 0.0)
        };
        let attack = 1.0 - (-seconds / 0.025).exp();
        self.rms += (rms - self.rms) * attack;
        self.bass += (bass - self.bass) * attack;
        let target = if live { 1.0 } else { 0.0 };
        self.blend += (target - self.blend) * (1.0 - (-seconds / 0.18).exp());
        let features = if live {
            self.snapshot
                .as_ref()
                .map_or(Default::default(), |s| s.features)
        } else {
            Default::default()
        };
        if live && let Some(snapshot) = &self.snapshot {
            self.reactivity
                .ingest(&snapshot.transients, now, self.not_before);
        }
        if !playing {
            self.reactivity.clear_ripples();
        }
        self.reactivity.advance(seconds, now, features, live);
    }
    pub fn needs_redraw(&self) -> bool {
        self.blend > 0.001
    }
}

#[derive(Debug, Clone, Default)]
pub struct WaveformAnimation {
    pub phase: f64,
    pub profile_mix: f32,
    pub playing_seconds: f64,
}

#[derive(Debug, Clone)]
pub struct WaveformState {
    pub profile: WaveformProfile,
    previous: Option<WaveformProfile>,
    pub animation: WaveformAnimation,
    pub audio: AudioReactiveInput,
    pub lighting: super::waveform_lighting::WaveformLighting,
    pub orb: super::beat_orb::BeatOrb,
    pub traveling: TravelingWaves,
}
impl WaveformState {
    pub fn new(music: &MusicInfo) -> Self {
        Self {
            profile: WaveformProfile::new(WaveformProfile::track_seed(music)),
            previous: None,
            animation: WaveformAnimation {
                phase: 0.0,
                profile_mix: 1.0,
                playing_seconds: 0.0,
            },
            audio: AudioReactiveInput::default(),
            lighting: super::waveform_lighting::WaveformLighting::default(),
            orb: super::beat_orb::BeatOrb::default(),
            traveling: TravelingWaves::default(),
        }
    }
    pub fn select_track(&mut self, music: &MusicInfo, animate: bool) -> bool {
        let seed = WaveformProfile::track_seed(music);
        if self.profile.seed == seed {
            return false;
        }
        let profile = WaveformProfile::new(seed);
        self.previous = animate.then(|| self.profile.clone());
        self.profile = profile;
        self.lighting.clear();
        self.orb = super::beat_orb::BeatOrb::default();
        self.animation.profile_mix = if animate { 0.0 } else { 1.0 };
        true
    }
    pub fn advance(
        &mut self,
        elapsed: Duration,
        playing: bool,
        config: &WaveformConfig,
        now: Instant,
    ) {
        let seconds = elapsed.as_secs_f32().min(0.1);
        if playing {
            self.animation.playing_seconds += seconds as f64;
            self.animation.phase += seconds as f64 * config.animation_speed as f64 * 1.5;
        }
        self.animation.profile_mix = (self.animation.profile_mix + seconds / 0.3).min(1.0);
        if self.animation.profile_mix >= 1.0 {
            self.previous = None;
        }
        self.audio.advance(seconds, now, playing);
        let reactive = &self.audio.reactivity;
        let live_settings = config.reactivity.enabled
            && config.live_audio
            && config.mode != WaveformMode::Procedural;
        self.traveling.advance(
            elapsed,
            TravelingInput {
                features: reactive.features,
                blend: if live_settings { self.audio.blend } else { 0.0 },
                onset_density: reactive.onset_density(now),
                activity: reactive.activity(now),
                playing,
                animation_speed: config.animation_speed,
            },
        );
    }
    pub fn morphing(&self) -> bool {
        self.previous.is_some()
    }
    pub fn gain(&self, index: usize) -> f32 {
        let current = self.profile.envelope[index];
        self.previous.as_ref().map_or(current, |old| {
            old.envelope[index] + (current - old.envelope[index]) * self.animation.profile_mix
        })
    }
}
