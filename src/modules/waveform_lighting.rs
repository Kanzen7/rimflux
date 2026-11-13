//! Transient lighting is independent from authoritative playback time.
use std::time::Instant;

#[derive(Debug, Clone, Copy)]
pub struct LightTrailSample {
    pub position: f32,
    pub born: Instant,
}

#[derive(Debug, Clone, Default)]
pub struct WaveformLighting {
    pub dragging: bool,
    pub direction: f32,
    pub trail: [Option<LightTrailSample>; 8],
    position: Option<f32>,
    last_event: Option<Instant>,
    released: Option<(f32, Instant)>,
}

impl WaveformLighting {
    pub fn drag(&mut self, position: f32, now: Instant) {
        if !position.is_finite() {
            return;
        }
        let position = position.clamp(0.0, 1.0);
        if let Some(previous) = self.position.filter(|_| self.dragging) {
            let distance = position - previous;
            if distance.abs() > 0.001 {
                self.direction = distance.signum();
                if distance.abs() > 0.18 {
                    self.trail.fill(None);
                }
                self.trail.rotate_right(1);
                self.trail[0] = Some(LightTrailSample {
                    position: previous,
                    born: now,
                });
            }
        }
        self.position = Some(position);
        self.dragging = true;
        self.last_event = Some(now);
        self.released = None;
    }

    pub fn release(&mut self, now: Instant) {
        if self.dragging {
            self.released = self.position.map(|position| (position, now));
        }
        self.dragging = false;
        self.last_event = Some(now);
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn needs_redraw(&self, now: Instant) -> bool {
        self.last_event
            .is_some_and(|event| now.saturating_duration_since(event).as_secs_f32() < 0.75)
    }

    pub fn seek_energy(&self, now: Instant, reduced_motion: bool) -> f32 {
        if reduced_motion {
            return if self.dragging { 0.65 } else { 0.0 };
        }
        if self.dragging {
            let age = self.last_event.map_or(1.0, |event| {
                now.saturating_duration_since(event).as_secs_f32()
            });
            0.65 + 0.35 * (-age * 10.0).exp()
        } else {
            self.released.map_or(0.0, |(_, released)| {
                let age = now.saturating_duration_since(released).as_secs_f32();
                if age >= 0.75 {
                    0.0
                } else {
                    (-age * 6.5).exp() * (1.0 - age / 0.75)
                }
            })
        }
    }

    pub fn released_position(&self, now: Instant) -> Option<f32> {
        self.released
            .filter(|(_, at)| now.saturating_duration_since(*at).as_secs_f32() < 0.75)
            .map(|(position, _)| position)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TravelingFlare {
    pub position: f32,
    pub energy: f32,
    pub layer: usize,
}

#[derive(Debug, Clone, Copy)]
pub enum PlaybackHeadState {
    IdleGlow,
    Breathing,
    Flare,
    Scrubbing,
}

#[derive(Debug, Clone, Copy)]
pub struct PlaybackHeadIntensity {
    pub state: PlaybackHeadState,
    pub breathing: f32,
    pub flare: f32,
    pub glow: f32,
}

/// Measured energy affects brightness; only real transients affect beat flares.
/// Unavailable capture never schedules substitute beat pulses.
pub fn playback_head_intensity(
    playing: bool,
    reduced: bool,
    scrubbing: bool,
    seek_energy: f32,
    measured_flare: f32,
    measured_rms: f32,
) -> PlaybackHeadIntensity {
    let breathing = if playing && !reduced {
        measured_rms.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let flare = if reduced {
        0.0
    } else {
        measured_flare.clamp(0.0, 1.0)
    };
    let state = if scrubbing {
        PlaybackHeadState::Scrubbing
    } else if flare > 0.01 {
        PlaybackHeadState::Flare
    } else if breathing > 0.0 {
        PlaybackHeadState::Breathing
    } else {
        PlaybackHeadState::IdleGlow
    };
    PlaybackHeadIntensity {
        state,
        breathing,
        flare,
        glow: (0.76 + breathing * 0.08 + flare * 0.14 + seek_energy * 0.16).clamp(0.0, 1.0),
    }
}

/// Sparse, deterministic pulses: at most one short pulse per lane every 10–14s.
/// These travel on supporting lines, never on the playback-head ribbon.
pub fn traveling_flares(time: f64, seed: u64, expanded: bool) -> [Option<TravelingFlare>; 2] {
    std::array::from_fn(|lane| {
        if !expanded && lane > 0 {
            return None;
        }
        let period = if expanded {
            10.5 + lane as f64 * 2.9
        } else {
            15.5
        };
        let offset = ((seed >> (lane * 12)) & 0xff) as f64 / 255.0 * 7.0;
        let local = (time + offset).rem_euclid(period);
        let lifetime = if expanded { 1.9 } else { 1.3 };
        if local > lifetime {
            return None;
        }
        let t = (local / lifetime) as f32;
        let energy = (std::f32::consts::PI * t).sin().max(0.0).powi(2);
        Some(TravelingFlare {
            position: if lane == 0 {
                0.1 + t * 0.8
            } else {
                0.9 - t * 0.8
            },
            energy: energy * if expanded { 0.3 } else { 0.12 },
            layer: if expanded { 2 + lane * 2 } else { 1 },
        })
    })
}
