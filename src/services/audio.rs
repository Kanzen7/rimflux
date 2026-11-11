//! Bounded streaming DSP. Only scalar features leave this module; audio is never persisted.
use std::time::Instant;

const FFT_SIZE: usize = 2048;
const HOP: usize = 512;
const FLUX_HISTORY: usize = 96;
pub const TRANSIENT_CAPACITY: usize = 8;

#[derive(Debug, Clone, Copy, Default)]
pub struct AudioFeatures {
    pub overall_energy: f32,
    pub bass_energy: f32,
    pub mid_energy: f32,
    pub presence_energy: f32,
    pub high_energy: f32,
    pub low_onset: f32,
    pub mid_onset: f32,
    pub high_onset: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct AudioTransient {
    pub at: Instant,
    pub low: f32,
    pub mid: f32,
    pub high: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioCaptureStatus {
    Starting,
    Live,
    Unavailable,
}

#[derive(Debug, Clone)]
pub struct AudioSnapshot {
    pub status: AudioCaptureStatus,
    pub rms: f32,
    pub bass: f32,
    pub spectral_flux: f32,
    pub low_onset: f32,
    pub high_onset: f32,
    pub raw_onset_strength: f32,
    pub onset_strength: f32,
    pub onset_at: Option<Instant>,
    pub captured_at: Instant,
    pub reason: Option<String>,
    pub features: AudioFeatures,
    pub transients: [Option<AudioTransient>; TRANSIENT_CAPACITY],
}
impl AudioSnapshot {
    pub fn unavailable(reason: impl Into<String>) -> Self {
        Self {
            status: AudioCaptureStatus::Unavailable,
            rms: 0.0,
            bass: 0.0,
            spectral_flux: 0.0,
            low_onset: 0.0,
            high_onset: 0.0,
            raw_onset_strength: 0.0,
            onset_strength: 0.0,
            onset_at: None,
            captured_at: Instant::now(),
            reason: Some(reason.into()),
            features: AudioFeatures::default(),
            transients: [None; TRANSIENT_CAPACITY],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AnalysisSettings {
    pub sensitivity_milli: u16,
    pub cooldown_ms: u64,
    pub adaptive_normalization: bool,
    pub transient_sensitivity_milli: u16,
}
impl From<&crate::core::config::BeatOrbConfig> for AnalysisSettings {
    fn from(config: &crate::core::config::BeatOrbConfig) -> Self {
        Self {
            sensitivity_milli: (config.sensitivity.clamp(0.0, 1.0) * 1000.0).round() as u16,
            cooldown_ms: config.cooldown_ms.clamp(70, 100),
            adaptive_normalization: true,
            transient_sensitivity_milli: 750,
        }
    }
}
impl From<&crate::core::config::WaveformConfig> for AnalysisSettings {
    fn from(config: &crate::core::config::WaveformConfig) -> Self {
        Self {
            adaptive_normalization: config.reactivity.adaptive_normalization,
            transient_sensitivity_milli: (config.reactivity.transient_sensitivity * 1000.0).round()
                as u16,
            ..Self::from(&config.beat_orb)
        }
    }
}

struct BandBaseline {
    history: [f32; FLUX_HISTORY],
    len: usize,
    cursor: usize,
}
impl Default for BandBaseline {
    fn default() -> Self {
        Self {
            history: [0.0; FLUX_HISTORY],
            len: 0,
            cursor: 0,
        }
    }
}
impl BandBaseline {
    fn normalize(&mut self, flux: f32, sensitivity: f32) -> f32 {
        self.normalize_mode(flux, sensitivity, true)
    }
    fn normalize_mode(&mut self, flux: f32, sensitivity: f32, adaptive: bool) -> f32 {
        let count = self.len.max(1) as f32;
        let mean = self.history[..self.len].iter().sum::<f32>() / count;
        let deviation = (self.history[..self.len]
            .iter()
            .map(|v| (v - mean).powi(2))
            .sum::<f32>()
            / count)
            .sqrt();
        let peak = self.history[..self.len]
            .iter()
            .copied()
            .fold(0.0_f32, f32::max);
        let threshold = if adaptive {
            (mean + deviation * (1.65 - sensitivity * 0.8)).max(0.0007)
        } else {
            0.0007
        };
        // A small absolute floor avoids amplifying near-silence into a strong beat.
        // The rolling scale adapts to the mix; stronger attacks within it stay larger.
        let floor = 0.10 - sensitivity * 0.05;
        let scale = if adaptive {
            (peak * 0.35).max(mean + deviation * 3.0).max(floor)
        } else {
            floor
        };
        // Soft saturation retains intensity differences instead of making every
        // above-baseline hit reach the maximum orb size.
        let strength = 1.0 - (-0.85 * (flux - threshold).max(0.0) / scale).exp();
        self.history[self.cursor] = flux;
        self.cursor = (self.cursor + 1) % FLUX_HISTORY;
        self.len = (self.len + 1).min(FLUX_HISTORY);
        strength
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct AudioSample {
    pub mono: f32,
    pub power: f32,
}

#[derive(Clone, Copy, Default)]
struct Complex {
    re: f32,
    im: f32,
}
impl Complex {
    fn multiply(self, other: Self) -> Self {
        Self {
            re: self.re * other.re - self.im * other.im,
            im: self.re * other.im + self.im * other.re,
        }
    }
}

/// Fixed FFT/window/history storage is allocated once on the capture worker.
pub struct AudioAnalyzer {
    sample_rate: f32,
    ring: [f32; FFT_SIZE],
    power: [f32; FFT_SIZE],
    hann: [f32; FFT_SIZE],
    work: [Complex; FFT_SIZE],
    twiddles: [Complex; FFT_SIZE / 2],
    previous_spectrum: [f32; FFT_SIZE / 2],
    low: BandBaseline,
    high: BandBaseline,
    feature_flux: [BandBaseline; 4],
    energy_reference: [f32; 5],
    previous_features: AudioFeatures,
    last_transient_frame: u64,
    settings: AnalysisSettings,
    cursor: usize,
    filled: usize,
    since_fft: usize,
    frames: u64,
    previous_strength: f32,
    previous_raw: f32,
    previous_bands: (f32, f32),
    previous_at: Option<Instant>,
    last_onset_frame: u64,
    pub snapshot: AudioSnapshot,
}
impl AudioAnalyzer {
    pub fn new(sample_rate: u32, settings: AnalysisSettings) -> Self {
        Self {
            sample_rate: sample_rate.max(8000) as f32,
            ring: [0.0; FFT_SIZE],
            power: [0.0; FFT_SIZE],
            hann: std::array::from_fn(|i| {
                0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / (FFT_SIZE - 1) as f32).cos()
            }),
            work: [Complex::default(); FFT_SIZE],
            twiddles: std::array::from_fn(|i| {
                let angle = -std::f32::consts::TAU * i as f32 / FFT_SIZE as f32;
                Complex {
                    re: angle.cos(),
                    im: angle.sin(),
                }
            }),
            previous_spectrum: [0.0; FFT_SIZE / 2],
            low: BandBaseline::default(),
            high: BandBaseline::default(),
            feature_flux: std::array::from_fn(|_| BandBaseline::default()),
            energy_reference: [0.0; 5],
            previous_features: AudioFeatures::default(),
            last_transient_frame: 0,
            settings,
            cursor: 0,
            filled: 0,
            since_fft: 0,
            frames: 0,
            previous_strength: 0.0,
            previous_raw: 0.0,
            previous_bands: (0.0, 0.0),
            previous_at: None,
            last_onset_frame: 0,
            snapshot: AudioSnapshot {
                status: AudioCaptureStatus::Starting,
                ..AudioSnapshot::unavailable("Warming up audio analysis")
            },
        }
    }

    pub fn push(&mut self, sample: AudioSample, now: Instant) {
        self.ring[self.cursor] = if sample.mono.is_finite() {
            sample.mono.clamp(-1.0, 1.0)
        } else {
            0.0
        };
        self.power[self.cursor] = if sample.power.is_finite() {
            sample.power.clamp(0.0, 1.0)
        } else {
            0.0
        };
        self.cursor = (self.cursor + 1) % FFT_SIZE;
        self.filled = (self.filled + 1).min(FFT_SIZE);
        self.since_fft += 1;
        self.frames += 1;
        if self.filled == FFT_SIZE && self.since_fft >= HOP {
            self.since_fft = 0;
            self.analyze(now);
        }
    }

    fn analyze(&mut self, now: Instant) {
        let raw_rms = (self.power.iter().sum::<f32>() / FFT_SIZE as f32).sqrt();
        for i in 0..FFT_SIZE {
            let reversed = i.reverse_bits() >> (usize::BITS - FFT_SIZE.ilog2());
            self.work[reversed] = Complex {
                re: self.ring[(self.cursor + i) % FFT_SIZE] * self.hann[i],
                im: 0.0,
            };
        }
        let mut length = 2;
        while length <= FFT_SIZE {
            let half = length / 2;
            let stride = FFT_SIZE / length;
            for block in (0..FFT_SIZE).step_by(length) {
                for j in 0..half {
                    let a = self.work[block + j];
                    let b = self.work[block + j + half].multiply(self.twiddles[j * stride]);
                    self.work[block + j] = Complex {
                        re: a.re + b.re,
                        im: a.im + b.im,
                    };
                    self.work[block + j + half] = Complex {
                        re: a.re - b.re,
                        im: a.im - b.im,
                    };
                }
            }
            length *= 2;
        }
        let mut bass_power = 0.0;
        let mut low_flux = 0.0;
        let mut high_flux = 0.0;
        let mut band_power = [0.0_f32; 4];
        let mut band_flux = [0.0_f32; 4];
        for bin in 1..FFT_SIZE / 2 {
            let frequency = bin as f32 * self.sample_rate / FFT_SIZE as f32;
            let value = self.work[bin];
            let magnitude = (value.re * value.re + value.im * value.im).sqrt() / FFT_SIZE as f32;
            if (20.0..=180.0).contains(&frequency) {
                bass_power += magnitude * magnitude * 2.0 / 0.375;
            }
            if (20.0..=10000.0).contains(&frequency) {
                let rise = (magnitude - self.previous_spectrum[bin]).max(0.0);
                let band = if frequency <= 180.0 {
                    0
                } else if frequency <= 1000.0 {
                    1
                } else if frequency <= 5000.0 {
                    2
                } else {
                    3
                };
                band_power[band] += magnitude * magnitude * 2.0 / 0.375;
                band_flux[band] += rise * rise * 2.0 / 0.375;
                // Keep the proven orb detector's exact 20–180 / 180–6000 Hz split.
                if frequency <= 180.0 {
                    low_flux += rise * rise;
                } else if frequency <= 6000.0 {
                    high_flux += rise * rise;
                }
                self.previous_spectrum[bin] = magnitude;
            }
        }
        let low_raw = (low_flux * 2.0 / 0.375).sqrt();
        let high_raw = (high_flux * 2.0 / 0.375).sqrt();
        let sensitivity = self.settings.sensitivity_milli.min(1000) as f32 / 1000.0;
        let low = self.low.normalize(low_raw, sensitivity);
        let high = self.high.normalize(high_raw, sensitivity);
        let transient_sensitivity =
            self.settings.transient_sensitivity_milli.min(1000) as f32 / 1000.0;
        let onsets: [f32; 4] = std::array::from_fn(|i| {
            self.feature_flux[i].normalize_mode(
                band_flux[i].sqrt(),
                transient_sensitivity,
                self.settings.adaptive_normalization,
            )
        });
        let seconds = HOP as f32 / self.sample_rate;
        let raw_energy = [
            raw_rms,
            band_power[0].sqrt(),
            band_power[1].sqrt(),
            band_power[2].sqrt(),
            band_power[3].sqrt(),
        ];
        let floors = [0.06, 0.02, 0.025, 0.012, 0.006];
        let energies: [f32; 5] = std::array::from_fn(|i| {
            self.energy_reference[i] =
                raw_energy[i].max(self.energy_reference[i] * (-seconds / 8.0).exp());
            let divisor = if self.settings.adaptive_normalization {
                (self.energy_reference[i] * 1.15).max(floors[i])
            } else {
                floors[i] * 3.0
            };
            ((raw_energy[i] / divisor).clamp(0.0, 1.0).powf(0.8)
                * (raw_energy[i] / (floors[i] * 0.15)).clamp(0.0, 1.0))
            .clamp(0.0, 1.0)
        });
        let features = AudioFeatures {
            overall_energy: energies[0],
            bass_energy: energies[1],
            mid_energy: energies[2],
            presence_energy: energies[3],
            high_energy: energies[4],
            low_onset: onsets[0],
            mid_onset: onsets[1].max(onsets[2]),
            high_onset: onsets[3],
        };
        let percussion = |f: AudioFeatures| (f.low_onset * 0.55).max(f.mid_onset).max(f.high_onset);
        if self.feature_flux[0].len >= 24
            && raw_rms > 0.0008
            && (self.frames - self.last_transient_frame) as f32 / self.sample_rate >= 0.045
            && percussion(self.previous_features) > 0.05
            && percussion(features) < percussion(self.previous_features) * 0.97
            && let Some(at) = self.previous_at
        {
            self.snapshot.transients.rotate_right(1);
            self.snapshot.transients[0] = Some(AudioTransient {
                at,
                low: self.previous_features.low_onset,
                mid: self.previous_features.mid_onset,
                high: self.previous_features.high_onset,
            });
            self.last_transient_frame = self.frames;
        }
        self.previous_features = features;
        self.snapshot.features = features;
        let strength = (low * 0.55 + high * 0.45 + low.max(high) * 0.45).clamp(0.0, 1.0);
        let refractory = (self.frames - self.last_onset_frame) as f32 / self.sample_rate
            >= self.settings.cooldown_ms as f32 / 1000.0;
        // Pick the local maximum one hop later rather than triggering on the tiny
        // leading edge. One timestamp/strength survives the bounded UI mailbox.
        if self.low.len >= 24
            && refractory
            && raw_rms > 0.001
            && self.previous_strength > 0.04
            && strength < self.previous_strength * 0.97
        {
            self.snapshot.onset_at = self.previous_at;
            self.snapshot.onset_strength = self.previous_strength;
            self.snapshot.low_onset = self.previous_bands.0;
            self.snapshot.high_onset = self.previous_bands.1;
            self.snapshot.raw_onset_strength = self.previous_raw;
            self.last_onset_frame = self.frames;
        }
        self.previous_strength = strength;
        self.previous_raw = low_raw * 0.55 + high_raw * 0.45;
        self.previous_bands = (low, high);
        self.previous_at = Some(now);
        self.snapshot.rms = smooth(self.snapshot.rms, energy_level(raw_rms), seconds);
        self.snapshot.bass = smooth(self.snapshot.bass, energy_level(bass_power.sqrt()), seconds);
        self.snapshot.spectral_flux = low_raw + high_raw;
        self.snapshot.captured_at = now;
        self.snapshot.status = AudioCaptureStatus::Live;
        self.snapshot.reason = None;
    }

    /// A packet discontinuity or silent endpoint must not become an artificial onset.
    pub fn reset_window(&mut self) {
        self.ring.fill(0.0);
        self.power.fill(0.0);
        self.previous_spectrum.fill(0.0);
        self.low = BandBaseline::default();
        self.high = BandBaseline::default();
        self.feature_flux = std::array::from_fn(|_| BandBaseline::default());
        self.energy_reference.fill(0.0);
        self.previous_features = AudioFeatures::default();
        self.snapshot.features = AudioFeatures::default();
        self.snapshot.transients.fill(None);
        self.filled = 0;
        self.since_fft = 0;
        self.previous_strength = 0.0;
        self.previous_raw = 0.0;
        self.previous_bands = (0.0, 0.0);
        self.previous_at = None;
        self.snapshot.onset_at = None;
        self.snapshot.onset_strength = 0.0;
        self.snapshot.raw_onset_strength = 0.0;
        self.snapshot.low_onset = 0.0;
        self.snapshot.high_onset = 0.0;
    }

    pub fn silence(&mut self, now: Instant) {
        let seconds = now
            .saturating_duration_since(self.snapshot.captured_at)
            .as_secs_f32()
            .min(0.5);
        self.snapshot.rms = smooth(self.snapshot.rms, 0.0, seconds);
        self.snapshot.bass = smooth(self.snapshot.bass, 0.0, seconds);
        self.snapshot.spectral_flux = 0.0;
        self.snapshot.onset_at = None;
        self.snapshot.onset_strength = 0.0;
        self.snapshot.raw_onset_strength = 0.0;
        self.snapshot.low_onset = 0.0;
        self.snapshot.high_onset = 0.0;
        self.snapshot.features = AudioFeatures::default();
        self.snapshot.transients.fill(None);
        for reference in &mut self.energy_reference {
            *reference *= (-seconds / 8.0).exp();
        }
        self.snapshot.status = AudioCaptureStatus::Live;
        self.snapshot.reason = None;
        self.snapshot.captured_at = now;
    }
}

fn energy_level(rms: f32) -> f32 {
    if rms < 0.00001 {
        0.0
    } else {
        ((20.0 * rms.log10() + 65.0) / 53.0).clamp(0.0, 1.0)
    }
}
fn smooth(current: f32, target: f32, seconds: f32) -> f32 {
    let time_constant = if target > current { 0.035 } else { 0.32 };
    current + (target - current) * (1.0 - (-seconds / time_constant).exp())
}
