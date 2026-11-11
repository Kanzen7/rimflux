//! Continuous rightward phase clocks. These never read or change playback progress.
use crate::services::audio::AudioFeatures;
use std::time::Duration;

#[derive(Debug, Clone, Copy)]
pub enum TravelingLayer {
    Mid,
    Micro,
    High,
}
impl TravelingLayer {
    pub const ALL: [Self; 3] = [Self::Mid, Self::Micro, Self::High];
    fn index(self) -> usize {
        match self {
            Self::Mid => 0,
            Self::Micro => 1,
            Self::High => 2,
        }
    }
}

/// Independent wavelength, vertical strength and angular velocity; no shared
/// global amplitude/speed knob is used to manufacture musical activity.
#[derive(Debug, Clone, Copy)]
pub struct TravelTuning {
    pub cycles: f64,
    pub amplitude: f32,
    pub minimum_velocity: f64,
    pub maximum_velocity: f64,
}
impl TravelingLayer {
    pub fn tuning(self) -> TravelTuning {
        match self {
            Self::Mid => TravelTuning {
                cycles: 5.0,
                amplitude: 0.46,
                minimum_velocity: 1.6,
                maximum_velocity: 4.2,
            },
            Self::Micro => TravelTuning {
                cycles: 8.0,
                amplitude: 0.42,
                minimum_velocity: 3.0,
                maximum_velocity: 8.4,
            },
            Self::High => TravelTuning {
                cycles: 9.0,
                amplitude: 0.34,
                minimum_velocity: 4.0,
                maximum_velocity: 10.4,
            },
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TravelingInput {
    pub features: AudioFeatures,
    pub blend: f32,
    pub onset_density: f32,
    pub activity: f32,
    pub playing: bool,
    pub animation_speed: f32,
}
impl Default for TravelingInput {
    fn default() -> Self {
        Self {
            features: AudioFeatures::default(),
            blend: 0.0,
            onset_density: 0.0,
            activity: 0.0,
            playing: false,
            animation_speed: 1.0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TravelingParameters {
    pub phase: f64,
    pub angular_velocity: f64,
    pub cycles: f64,
    pub amplitude: f32,
    pub definition: f32,
    pub opacity: f32,
}
impl TravelingParameters {
    /// k*x - phase moves crests right for positive phase velocity. The harmonic
    /// has the same propagation speed, so it does not create a counter-wave.
    pub fn sample(self, x: f32, offset: f64) -> f32 {
        let theta = std::f64::consts::TAU * self.cycles * x as f64 - self.phase + offset;
        ((theta.sin() + self.definition as f64 * (theta * 2.0 + 0.6).sin()) * self.amplitude as f64)
            as f32
    }
}

#[derive(Debug, Clone)]
pub struct TravelingWaves {
    phases: [f64; 3],
    velocities: [f64; 3],
    density: f32,
}
impl Default for TravelingWaves {
    fn default() -> Self {
        Self {
            phases: [0.0; 3],
            velocities: [1.6, 3.0, 4.0],
            density: 0.0,
        }
    }
}
impl TravelingWaves {
    pub fn advance(&mut self, elapsed: Duration, input: TravelingInput) {
        if !input.playing {
            return;
        }
        let seconds = elapsed.as_secs_f64();
        let blend = input.blend.clamp(0.0, 1.0) as f64;
        let f = input.features;
        let density = input.onset_density.clamp(0.0, 1.0) as f64;
        let activity = input.activity.clamp(0.0, 1.0) as f64;
        let targets = [
            1.6 + blend * (1.7 * f.mid_energy as f64 + 0.6 * f.mid_onset as f64 + 0.3 * activity),
            3.0 + blend
                * (0.8 * f.presence_energy as f64
                    + 1.2 * f.high_onset as f64
                    + 2.8 * density
                    + 0.6 * activity),
            4.0 + blend * (3.0 * f.high_energy as f64 + 1.3 * f.high_onset as f64 + 2.1 * density),
        ];
        let speed = (input.animation_speed as f64).clamp(0.1, 3.0);
        for (i, layer) in TravelingLayer::ALL.into_iter().enumerate() {
            let tuning = layer.tuning();
            let target = targets[i].clamp(tuning.minimum_velocity, tuning.maximum_velocity) * speed;
            let old = self.velocities[i];
            let tau = if target > old { 0.16 } else { 0.36 };
            let decay = (-seconds / tau).exp();
            // Analytic integration of the smoothed velocity; independent of
            // frame frequency for a constant target. Never multiply phase by blend.
            self.phases[i] += target * seconds + (old - target) * tau * (1.0 - decay);
            self.velocities[i] = target + (old - target) * decay;
        }
        self.density += (input.onset_density * input.blend - self.density)
            * (1.0 - (-seconds as f32 / 0.18).exp());
    }

    pub fn parameters(&self, layer: TravelingLayer) -> TravelingParameters {
        let i = layer.index();
        let tuning = layer.tuning();
        let velocity_fraction = ((self.velocities[i] - tuning.minimum_velocity)
            / (tuning.maximum_velocity - tuning.minimum_velocity))
            .clamp(0.0, 1.0) as f32;
        TravelingParameters {
            phase: self.phases[i],
            angular_velocity: self.velocities[i],
            cycles: tuning.cycles,
            amplitude: tuning.amplitude,
            // Slightly finer structure through a co-propagating harmonic, not a
            // changing k*x term that could pull peaks backward as density rises.
            definition: 0.16 + self.density.clamp(0.0, 1.0) * 0.12,
            opacity: 1.0 - velocity_fraction * 0.10,
        }
    }
}
