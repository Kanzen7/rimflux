//! Prepared layer responses and bounded, measured-onset wave packets; no media APIs.
use crate::{
    core::config::ReactivityConfig,
    services::audio::{AudioFeatures, AudioTransient, TRANSIENT_CAPACITY},
};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy)]
pub enum MusicalLayer {
    Bass,
    Mid,
    Percussion,
    High,
    Ambient,
}
pub const MAX_RIPPLE_PACKETS: usize = 4;

#[derive(Debug, Clone, Copy)]
pub struct LayerResponse {
    pub amplitude: f32,
    pub thickness: f32,
    pub opacity: f32,
    pub glow: f32,
    pub motion: f32,
    pub phase: f64,
}
impl Default for LayerResponse {
    fn default() -> Self {
        Self {
            amplitude: 1.0,
            thickness: 1.0,
            opacity: 1.0,
            glow: 1.0,
            motion: 1.0,
            phase: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct MicroRipple {
    born: Instant,
    origin: f32,
    velocity: f32,
    strength: f32,
    density: f32,
}
impl MicroRipple {
    fn envelope(self, now: Instant) -> f32 {
        let age = now.saturating_duration_since(self.born).as_secs_f32();
        if age >= 0.36 {
            return 0.0;
        }
        let attack = smoothstep(age / 0.015);
        self.strength * attack * (-(age - 0.015).max(0.0) / 0.105).exp() * (1.0 - age / 0.36)
    }
}

#[derive(Debug, Clone)]
pub struct WaveformReactivity {
    pub features: AudioFeatures,
    phases: [f64; 5],
    ripples: [Option<MicroRipple>; MAX_RIPPLE_PACKETS],
    last_event: Option<Instant>,
    serial: u32,
    last_density: f32,
}
impl Default for WaveformReactivity {
    fn default() -> Self {
        Self {
            features: AudioFeatures::default(),
            phases: [0.0; 5],
            ripples: [None; MAX_RIPPLE_PACKETS],
            last_event: None,
            serial: 0,
            last_density: 0.0,
        }
    }
}
impl WaveformReactivity {
    pub fn ingest(
        &mut self,
        events: &[Option<AudioTransient>; TRANSIENT_CAPACITY],
        now: Instant,
        cutoff: Instant,
    ) {
        for event in events.iter().rev().flatten() {
            if event.at < cutoff
                || self.last_event.is_some_and(|last| event.at <= last)
                || now.saturating_duration_since(event.at) > Duration::from_millis(250)
            {
                continue;
            }
            let spacing = self.last_event.map_or(1.0, |last| {
                event.at.saturating_duration_since(last).as_secs_f32()
            });
            self.last_event = Some(event.at);
            self.last_density = ((0.28 - spacing) / 0.225).clamp(0.0, 1.0);
            self.serial = self.serial.wrapping_add(1);
            self.ripples.rotate_right(1);
            self.ripples[0] = Some(MicroRipple {
                born: event.at,
                origin: 0.12 + (self.serial.wrapping_mul(37) % 71) as f32 * 0.01,
                velocity: 0.22 + self.last_density * 0.12,
                strength: (event.low * 0.35)
                    .max(event.mid)
                    .max(event.high)
                    .clamp(0.0, 1.0)
                    .powf(0.65),
                density: (0.16 / spacing.max(0.045)).clamp(0.0, 1.0),
            });
        }
    }

    pub fn clear_ripples(&mut self) {
        self.ripples.fill(None);
        self.last_density = 0.0;
    }
    pub fn onset_density(&self, now: Instant) -> f32 {
        self.last_event.map_or(0.0, |last| {
            self.last_density * (-now.saturating_duration_since(last).as_secs_f32() / 0.38).exp()
        })
    }

    pub fn advance(&mut self, seconds: f32, now: Instant, target: AudioFeatures, active: bool) {
        let f = &mut self.features;
        // Fast fine detail, flowing mids, heavy bass and a slower ambient envelope.
        f.overall_energy = follow(f.overall_energy, target.overall_energy, seconds, 0.14, 0.40);
        f.bass_energy = follow(f.bass_energy, target.bass_energy, seconds, 0.028, 0.18);
        f.mid_energy = follow(f.mid_energy, target.mid_energy, seconds, 0.035, 0.15);
        f.presence_energy = follow(
            f.presence_energy,
            target.presence_energy,
            seconds,
            0.015,
            0.09,
        );
        f.high_energy = follow(f.high_energy, target.high_energy, seconds, 0.018, 0.10);
        f.low_onset = follow(f.low_onset, target.low_onset, seconds, 0.022, 0.15);
        f.mid_onset = follow(f.mid_onset, target.mid_onset, seconds, 0.012, 0.09);
        f.high_onset = follow(f.high_onset, target.high_onset, seconds, 0.012, 0.08);
        for ripple in &mut self.ripples {
            if ripple.is_some_and(|r| {
                now.saturating_duration_since(r.born) >= Duration::from_millis(360)
            }) {
                *ripple = None;
            }
        }
        if active {
            for (phase, speed) in self.phases.iter_mut().zip([
                0.12 + f.bass_energy * 0.65,
                0.18 + f.mid_energy * 1.5,
                0.35 + f.presence_energy * 1.5,
                0.5 + f.high_energy * 2.0,
                0.06 + f.overall_energy * 0.22,
            ]) {
                *phase += (seconds * speed) as f64;
            }
        }
    }

    pub fn activity(&self, now: Instant) -> f32 {
        self.ripples
            .iter()
            .flatten()
            .map(|r| r.envelope(now))
            .sum::<f32>()
            .clamp(0.0, 1.0)
    }
    pub fn ripple_count(&self) -> usize {
        self.ripples.iter().flatten().count()
    }
    pub fn transient_count(&self) -> u32 {
        self.serial
    }

    pub fn response(
        &self,
        layer: MusicalLayer,
        config: &ReactivityConfig,
        blend: f32,
        now: Instant,
    ) -> LayerResponse {
        if !config.enabled {
            return LayerResponse::default();
        }
        let f = self.features;
        let bass = drive(f.bass_energy, config.bass_sensitivity);
        let mid = drive(f.mid_energy, config.mid_sensitivity);
        let high = drive(f.high_energy, config.high_sensitivity);
        let kick = drive(f.low_onset, config.transient_sensitivity);
        let flux = drive(f.mid_onset.max(f.high_onset), config.transient_sensitivity);
        let high_flux = drive(f.high_onset, config.transient_sensitivity);
        let activity = if config.micro_ripples {
            self.activity(now)
        } else {
            0.0
        };
        let (amplitude, thickness, opacity, glow, motion, phase) = match layer {
            MusicalLayer::Bass => (
                0.58 + bass * 0.50 + kick * 0.10,
                0.78 + bass * 0.40 + kick * 0.12,
                0.65 + bass * 0.35,
                0.45 + bass * 0.50 + kick * 0.20,
                0.18 + bass * 0.82,
                self.phases[0],
            ),
            MusicalLayer::Mid => (
                0.38 + mid * 0.78 + f.mid_onset * 0.12,
                1.0,
                0.38 + mid * 0.62,
                0.3 + mid * 0.6,
                0.16 + mid * 0.95,
                self.phases[1],
            ),
            MusicalLayer::Percussion => (
                0.28 + f.presence_energy * 0.45 + activity * 0.55,
                1.0,
                0.25 + f.presence_energy * 0.35 + activity * 0.40,
                0.25 + activity * 0.5,
                0.15 + flux * 0.6 + activity * 0.25,
                self.phases[2],
            ),
            MusicalLayer::High => (
                0.22 + high * 0.65 + f.high_onset * 0.25,
                1.0,
                0.20 + high * 0.50 + high_flux * 0.30,
                0.2 + high_flux * 0.5,
                0.10 + high * 0.6 + high_flux * 0.3,
                self.phases[3],
            ),
            MusicalLayer::Ambient => (
                0.5 + f.overall_energy * 0.50,
                1.0,
                0.4 + f.overall_energy * 0.5,
                0.3 + f.overall_energy * 0.4,
                0.10 + f.overall_energy * 0.55,
                self.phases[4],
            ),
        };
        let mix = |value: f32| 1.0 + (value - 1.0) * blend;
        LayerResponse {
            amplitude: mix(amplitude),
            thickness: mix(thickness),
            opacity: mix(opacity),
            glow: mix(glow),
            motion: mix(motion),
            phase: phase * blend as f64,
        }
    }

    /// Local vertical displacement only. Event spacing sets wavelength, not volume.
    pub fn samples<const N: usize>(&self, now: Instant) -> [f32; N] {
        let mut points = [0.0_f32; N];
        for r in self.ripples.iter().flatten() {
            let amplitude = r.envelope(now) * 0.22;
            if amplitude < 0.0001 {
                continue;
            }
            let age = now.saturating_duration_since(r.born).as_secs_f32();
            let center = r.origin + r.velocity * age;
            let width = 0.055 + r.strength * 0.045;
            for (i, value) in points.iter_mut().enumerate() {
                let x = i as f32 / N.saturating_sub(1).max(1) as f32;
                let offset = (x - center) / width;
                if offset.abs() >= 1.0 {
                    continue;
                }
                let local = (1.0 - offset * offset).powi(2);
                let phase = (x - r.origin - (r.velocity + 0.08) * age)
                    * std::f32::consts::TAU
                    * (18.0 + r.density * 20.0);
                *value += amplitude * local * phase.sin();
            }
        }
        points.map(|value| value.clamp(-0.24, 0.24))
    }

    /// Single-point diagnostics. Rendering uses `samples` to share envelopes.
    pub fn displacement(&self, x: f32, now: Instant) -> f32 {
        self.ripples
            .iter()
            .flatten()
            .map(|r| {
                let age = now.saturating_duration_since(r.born).as_secs_f32();
                let center = r.origin + r.velocity * age;
                let width = 0.055 + r.strength * 0.045;
                let offset = (x - center) / width;
                let local = (1.0 - offset * offset).max(0.0).powi(2);
                let phase = (x - r.origin - (r.velocity + 0.08) * age)
                    * std::f32::consts::TAU
                    * (18.0 + r.density * 20.0);
                r.envelope(now) * local * phase.sin() * 0.22
            })
            .sum::<f32>()
            .clamp(-0.24, 0.24)
    }
}
fn drive(value: f32, sensitivity: f32) -> f32 {
    (value * sensitivity * 1.5).clamp(0.0, 1.0).powf(0.8)
}
fn smoothstep(value: f32) -> f32 {
    let t = value.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
fn follow(value: f32, target: f32, dt: f32, attack: f32, release: f32) -> f32 {
    value + (target - value) * (1.0 - (-dt / if target > value { attack } else { release }).exp())
}
