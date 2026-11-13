//! Measured-transient envelope only. This module has no playback-position input.
use crate::{
    core::config::BeatOrbConfig,
    services::audio::{AudioCaptureStatus, AudioSnapshot},
};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum OrbPhase {
    #[default]
    Rest,
    Attack,
    Hold,
    Release,
}

#[derive(Debug, Clone, Copy)]
struct Pulse {
    at: Instant,
    from: f32,
    peak: f32,
    intensity: f32,
    releasing: bool,
}

#[derive(Debug, Clone, Default)]
pub struct BeatOrb {
    pulse: Option<Pulse>,
    seen: Option<Instant>,
    last_trigger: Option<Instant>,
    pub raw_onset: f32,
    pub normalized_onset: f32,
    pub triggered: bool,
    pub capture_status: Option<AudioCaptureStatus>,
}

#[derive(Debug, Clone, Copy)]
pub struct OrbVisual {
    pub core_diameter: f32,
    pub inner_diameter: f32,
    pub halo_diameter: f32,
    pub response: f32,
    pub flare: f32,
    pub sparkle: f32,
    pub phase: OrbPhase,
}

#[derive(Debug, Clone, Copy)]
pub struct OrbDimensions {
    pub core: f32,
    pub inner_x: f32,
    pub inner_y: f32,
    pub halo_x: f32,
    pub halo_y: f32,
}
impl OrbVisual {
    /// Radii used directly by the renderer, in logical pixels. No geometry can
    /// cross the vertical bounds even when the head sits on a high wave lobe.
    pub fn fit(&self, height: f32, clearance: f32) -> OrbDimensions {
        let radius = (height * 0.5 - 0.25).max(0.0);
        let halo_x = (self.halo_diameter * 0.5).min(radius);
        let inner_x = (self.inner_diameter * 0.5).min(radius);
        OrbDimensions {
            core: (self.core_diameter * 0.5).min((clearance - 0.1).max(0.0)),
            inner_x,
            inner_y: inner_x.min((clearance - 0.2).max(0.0)),
            halo_x,
            halo_y: halo_x.min((clearance - 0.25).max(0.0)),
        }
    }
}

impl BeatOrb {
    pub fn update(
        &mut self,
        now: Instant,
        snapshot: Option<&AudioSnapshot>,
        config: &BeatOrbConfig,
        active: bool,
        seeking: bool,
        reduced: bool,
    ) {
        self.triggered = false;
        self.capture_status = snapshot.map(|s| s.status);
        self.raw_onset = snapshot.map_or(0.0, |s| s.raw_onset_strength);
        if reduced || !config.enabled {
            self.pulse = None;
            self.seen = snapshot.and_then(|s| s.onset_at);
            self.normalized_onset = 0.0;
            return;
        }
        let fresh = snapshot.filter(|s| {
            s.status == AudioCaptureStatus::Live
                && now.saturating_duration_since(s.captured_at) < Duration::from_millis(350)
        });
        if !active || seeking || fresh.is_none() {
            self.release(now, config);
        }
        if let Some(s) = snapshot {
            self.normalized_onset = s.onset_strength;
            if let Some(at) = s.onset_at.filter(|at| Some(*at) != self.seen) {
                self.seen = Some(at); // Suppressed events cannot reappear after a seek or resume.
                let cooled = self.last_trigger.is_none_or(|last| {
                    at.saturating_duration_since(last) >= Duration::from_millis(config.cooldown_ms)
                });
                if active
                    && !seeking
                    && fresh.is_some()
                    && cooled
                    && now.saturating_duration_since(at) <= Duration::from_millis(100)
                {
                    let intensity = s.onset_strength.clamp(0.0, 1.0);
                    let gate = if intensity <= config.low_threshold {
                        0.08 * (intensity / config.low_threshold).powi(2)
                    } else {
                        0.08 + 0.92
                            * ease(
                                (intensity - config.low_threshold)
                                    / (config.strong_threshold - config.low_threshold),
                            )
                    };
                    let peak = intensity.powf(0.75) * gate;
                    let from = self.envelope(at, config).0;
                    self.pulse = Some(Pulse {
                        at,
                        from,
                        peak: peak.max(from),
                        intensity,
                        releasing: false,
                    });
                    self.last_trigger = Some(at);
                    self.triggered = true;
                }
            }
        }
        if self.envelope(now, config).1 == OrbPhase::Rest {
            self.pulse = None;
        }
    }

    fn release(&mut self, now: Instant, config: &BeatOrbConfig) {
        if self.pulse.is_some_and(|p| !p.releasing) {
            let (value, _) = self.envelope(now, config);
            if value <= 0.0001 {
                self.pulse = None;
                return;
            }
            let intensity = self.pulse.map_or(0.0, |p| p.intensity);
            self.pulse = Some(Pulse {
                at: now,
                from: value,
                peak: value,
                intensity,
                releasing: true,
            });
        }
    }

    fn envelope(&self, now: Instant, config: &BeatOrbConfig) -> (f32, OrbPhase) {
        let Some(p) = self.pulse else {
            return (0.0, OrbPhase::Rest);
        };
        let age = now.saturating_duration_since(p.at).as_secs_f32() * 1000.0;
        let attack = if p.releasing {
            0.0
        } else {
            config.attack_ms as f32
        };
        let hold = if p.releasing {
            0.0
        } else {
            config.peak_hold_ms as f32
        };
        if age < attack {
            (
                p.from + (p.peak - p.from) * ease(age / attack),
                OrbPhase::Attack,
            )
        } else if age < attack + hold {
            (p.peak, OrbPhase::Hold)
        } else {
            let t = (age - attack - hold) / config.release_ms as f32;
            if t >= 1.0 {
                (0.0, OrbPhase::Rest)
            } else {
                (p.peak * (1.0 - ease(t)), OrbPhase::Release)
            }
        }
    }

    pub fn is_animating(&self) -> bool {
        self.pulse.is_some()
    }

    /// Circular core; elliptical halos fit the head's actual clearance on the wave.
    pub fn visual(&self, now: Instant, config: &BeatOrbConfig, reduced: bool) -> OrbVisual {
        let (response, phase) = if reduced || !config.enabled {
            (0.0, OrbPhase::Rest)
        } else {
            self.envelope(now, config)
        };
        let intensity = self.pulse.map_or(0.0, |p| p.intensity);
        let flare = response
            * ease(
                (intensity - config.strong_threshold)
                    / (config.very_strong_threshold - config.strong_threshold),
            );
        let sparkle = response
            * ease(
                (intensity - config.very_strong_threshold) / (1.0 - config.very_strong_threshold),
            );
        OrbVisual {
            core_diameter: config.minimum_core_diameter
                + (config.maximum_core_diameter - config.minimum_core_diameter) * response,
            inner_diameter: 7.0 + 4.0 * response,
            halo_diameter: config.minimum_halo_diameter
                + (config.maximum_halo_diameter - config.minimum_halo_diameter) * response,
            response,
            flare,
            sparkle,
            phase,
        }
    }
}

fn ease(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
