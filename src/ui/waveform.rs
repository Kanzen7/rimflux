//! Layered playback timeline. Lighting follows prepared wave curves, never a slider line.
use super::{music_layout::WaveformSlot, wave_reference as reference};
use crate::{
    core::config::{WaveformConfig, WaveformMode},
    modules::{
        beat_orb::{BeatOrb, OrbVisual},
        traveling_wave::TravelingLayer,
        waveform::{PROFILE_SAMPLES, WaveformState},
        waveform_lighting::{
            PlaybackHeadIntensity, PlaybackHeadState, playback_head_intensity, traveling_flares,
        },
        waveform_reactivity::{LayerResponse, MusicalLayer},
    },
};
use iced::{
    Color, Point, Rectangle, Renderer, Size,
    widget::canvas::{Frame, Geometry, Gradient, LineCap, LineJoin, Path, Stroke, Style, gradient},
};
use std::time::Instant;

#[derive(Debug, Default)]
pub struct WaveformInteraction {
    pub dragging: bool,
    pub hovered: bool,
    pub track_seed: u64,
}

#[derive(Debug, Clone, Copy)]
pub struct WaveformPlayback {
    pub progress: f32,
    pub hover: Option<f32>,
    pub playing: bool,
    pub reduced_motion: bool,
    pub now: Instant,
    pub opacity: f32,
}

struct Layer {
    path: Path,
    center_path: Option<Path>,
    points: [Point; PROFILE_SAMPLES],
    ribbon: bool,
    width: f32,
    opacity: f32,
    cool: bool,
    response: LayerResponse,
}

impl Layer {
    fn centerline(&self) -> &Path {
        self.center_path.as_ref().unwrap_or(&self.path)
    }
}

struct WaveGeometry {
    layers: Vec<Layer>,
}
#[derive(Clone, Copy)]
struct LayerSpec {
    curve: &'static [f32; PROFILE_SAMPLES],
    phase: f64,
    speed: f64,
    frequency: f64,
    amplitude: f32,
    thickness: f32,
    width: f32,
    opacity: f32,
    cool: bool,
    role: MusicalLayer,
}

impl WaveGeometry {
    fn prepare(
        slot: WaveformSlot,
        state: &WaveformState,
        config: &WaveformConfig,
        now: Instant,
    ) -> Self {
        let area = slot.visual;
        let audio = if config.mode == WaveformMode::Procedural || !config.live_audio {
            crate::modules::waveform::AudioModulation::default()
        } else {
            state.audio.modulation(now)
        };
        let amplitude = 0.52 + config.intensity.clamp(0.0, 1.0) * 0.33 + 0.04;
        let reactive = &state.audio.reactivity;
        // Sample the bounded ripple field once; all fine layers reuse these points.
        let ripple_points: [f32; PROFILE_SAMPLES] = if config.reactivity.enabled
            && config.reactivity.micro_ripples
            && audio.blend > 0.001
        {
            reactive.samples(now).map(|value| value * audio.blend)
        } else {
            [0.0; PROFILE_SAMPLES]
        };
        let main = LayerSpec {
            curve: &reference::MAIN,
            phase: 0.0,
            speed: 1.0,
            frequency: 10.8,
            amplitude: 1.0,
            thickness: if slot.expanded { 3.8 } else { 2.3 },
            width: 0.0,
            opacity: 0.84,
            cool: false,
            role: MusicalLayer::Bass,
        };
        let cyan = LayerSpec {
            curve: &reference::CYAN,
            phase: 2.1,
            speed: -0.79,
            frequency: 16.2,
            amplitude: 0.95,
            thickness: 0.0,
            width: if slot.expanded { 1.05 } else { 0.95 },
            opacity: 0.86,
            cool: true,
            role: MusicalLayer::Percussion,
        };
        let prism = LayerSpec {
            curve: &reference::PRISM,
            phase: 4.6,
            speed: 0.18,
            frequency: 7.8,
            amplitude: 0.92,
            thickness: 0.0,
            width: 0.7,
            opacity: 0.58,
            cool: false,
            role: MusicalLayer::Ambient,
        };
        let specs = [
            main,
            LayerSpec {
                curve: &reference::SECONDARY,
                phase: 1.6,
                speed: -0.61,
                frequency: 13.5,
                amplitude: 0.82,
                thickness: 2.0,
                width: 0.0,
                opacity: 0.27,
                cool: true,
                role: MusicalLayer::Mid,
            },
            cyan,
            LayerSpec {
                curve: &reference::SECONDARY,
                phase: 3.3,
                speed: 0.93,
                frequency: 18.3,
                amplitude: 1.12,
                thickness: 0.0,
                width: 0.85,
                opacity: 0.63,
                cool: true,
                role: MusicalLayer::Mid,
            },
            LayerSpec {
                curve: &reference::PRISM,
                phase: 5.2,
                speed: -0.48,
                frequency: 34.0,
                amplitude: 0.88,
                thickness: 0.0,
                width: 0.85,
                opacity: 0.60,
                cool: false,
                role: MusicalLayer::High,
            },
            LayerSpec {
                curve: &reference::CYAN,
                phase: 6.8,
                speed: 0.56,
                frequency: 43.0,
                amplitude: 0.76,
                thickness: 0.0,
                width: 0.65,
                opacity: 0.46,
                cool: false,
                role: MusicalLayer::High,
            },
            prism,
        ];
        let mut layers = Vec::with_capacity(if slot.expanded { 7 } else { 3 });
        for (index, spec) in specs.into_iter().enumerate() {
            if !slot.expanded && ![0, 2, 6].contains(&index) {
                continue;
            }
            let response = reactive.response(spec.role, &config.reactivity, audio.blend, now);
            // Only three thin lines propagate. Neither ribbon nor the supporting
            // ambient/high line changes its established motion calculation.
            let traveling = match index {
                2 => Some(state.traveling.parameters(TravelingLayer::Micro)),
                3 => Some(state.traveling.parameters(TravelingLayer::Mid)),
                4 => Some(state.traveling.parameters(TravelingLayer::High)),
                _ => None,
            };
            let phase = state.animation.phase * spec.speed + spec.phase + response.phase;
            let slow = (state.animation.phase * 0.137 + spec.phase).sin();
            let local_amplitude =
                amplitude * spec.amplitude * (0.93 + slow as f32 * 0.07) * response.amplitude;
            let half_height = (area.height * 0.5 - 1.4).max(0.0);
            let ripple_gain = if index == 2 { 1.0 } else { 0.0 };
            let points = std::array::from_fn(|i| {
                let x = i as f32 / (PROFILE_SAMPLES - 1) as f32;
                let taper = (std::f32::consts::PI * x).sin().max(0.0).sqrt();
                let x64 = x as f64;
                let motion = ((x64 * spec.frequency
                    + phase
                    + (state.animation.phase * 0.19 + spec.phase).sin() * 0.4)
                    .sin()
                    * 0.24
                    + (x64 * (spec.frequency * 1.71) - phase * 0.73).sin() * 0.13
                    + (x64 * 6.4 + phase * 0.31 + spec.phase).sin() * 0.1)
                    as f32
                    * taper;
                let ripple = ripple_points[i] * ripple_gain;
                let shape = if let Some(travel) = traveling {
                    spec.curve[i] * state.gain(i) * 0.22
                        + travel.sample(x, spec.phase)
                            * (0.35 + response.motion * 0.65).min(1.0)
                            * taper
                } else {
                    spec.curve[i] * state.gain(i) * 0.74 + motion * response.motion
                };
                let displacement = shape * local_amplitude + ripple;
                let y = (area.height * 0.5 + displacement * half_height)
                    .clamp(0.6, (area.height - 0.6).max(0.6));
                Point::new(area.x + x * area.width, area.y + y)
            });
            let center_path = if traveling.is_some() {
                traveling_path(&points)
            } else {
                curve_path(&points, 0.0, 1.0)
            };
            let ribbon = spec.thickness > 0.0;
            let (path, center_path) = if ribbon {
                let thickness: [f32; PROFILE_SAMPLES] = std::array::from_fn(|i| {
                    let x = i as f64 / (PROFILE_SAMPLES - 1) as f64;
                    let taper = (std::f64::consts::PI * x).sin().max(0.0).sqrt() as f32;
                    spec.thickness
                        * response.thickness
                        * 0.5
                        * taper
                        * (0.83 + 0.17 * (phase * 0.61 + x * 9.7).sin() as f32)
                });
                let ribbon_path = Path::new(|path| {
                    for (i, point) in points.iter().enumerate() {
                        let top = Point::new(point.x, point.y - thickness[i]);
                        if i == 0 {
                            path.move_to(top);
                        } else {
                            path.line_to(top);
                        }
                    }
                    for (i, point) in points.iter().enumerate().rev() {
                        path.line_to(Point::new(point.x, point.y + thickness[i]));
                    }
                    path.close();
                });
                (ribbon_path, Some(center_path))
            } else {
                (center_path, None)
            };
            layers.push(Layer {
                path,
                center_path,
                points,
                ribbon,
                width: spec.width,
                opacity: spec.opacity
                    * response.opacity
                    * traveling.map_or(1.0, |wave| wave.opacity),
                cool: spec.cool,
                response,
            });
        }
        Self { layers }
    }
}

pub struct WaveformRenderer;

/// Headless development preview data; no GPU, desktop surface or media access.
#[cfg(debug_assertions)]
pub struct WaveformPreviewLayer {
    pub points: [Point; PROFILE_SAMPLES],
    pub opacity: f32,
    pub width: f32,
}
impl WaveformRenderer {
    #[cfg(debug_assertions)]
    pub fn preview_layers(
        slot: WaveformSlot,
        state: &WaveformState,
        config: &WaveformConfig,
        now: Instant,
    ) -> Vec<WaveformPreviewLayer> {
        WaveGeometry::prepare(slot, state, config, now)
            .layers
            .into_iter()
            .map(|layer| WaveformPreviewLayer {
                points: layer.points,
                opacity: layer.opacity,
                width: layer.width,
            })
            .collect()
    }
    pub fn draw(
        renderer: &Renderer,
        slot: WaveformSlot,
        state: &WaveformState,
        config: &WaveformConfig,
        playback: WaveformPlayback,
    ) -> Vec<Geometry> {
        let area = slot.visual;
        if playback.opacity <= 0.0 || area.width <= 0.0 || area.height <= 0.0 {
            return Vec::new();
        }
        let geometry = WaveGeometry::prepare(slot, state, config, playback.now);
        let audio = if config.mode == WaveformMode::Procedural || !config.live_audio {
            crate::modules::waveform::AudioModulation::default()
        } else {
            state.audio.modulation(playback.now)
        };
        let glow_gain = geometry.layers[0].response.glow;
        #[cfg(debug_assertions)]
        layer_diagnostics(state, &geometry, playback.now, audio.blend);
        let progress = finite_ratio(playback.progress);
        let played_width = area.width * progress;
        let opacity = playback.opacity;
        let mut output = Vec::with_capacity(3);
        for (clip, strength) in [
            (
                Rectangle::new(
                    Point::new(area.x + played_width, area.y),
                    Size::new(area.width - played_width, area.height),
                ),
                0.24,
            ),
            (
                Rectangle::new(
                    Point::new(area.x, area.y),
                    Size::new(played_width, area.height),
                ),
                1.0,
            ),
        ] {
            if clip.width <= 0.0 {
                continue;
            }
            // All primitives are scissored to the region AND the strict 14/25px height.
            let mut frame = Frame::with_bounds(renderer, clip);
            let main = &geometry.layers[0];
            let glow =
                config.glow.clamp(0.0, 0.35) * glow_gain * if slot.expanded { 1.0 } else { 0.3 };
            // Multiple translucent strokes provide a soft base halo without full-surface blur.
            for (width, gain) in [(7.5, 0.17), (4.7, 0.30)] {
                stroke(
                    &mut frame,
                    main.centerline(),
                    palette(area, opacity * strength * glow * gain, false),
                    width,
                );
            }
            for layer in &geometry.layers {
                let paint = palette(area, opacity * strength * layer.opacity, layer.cool);
                if layer.ribbon {
                    frame.fill(&layer.path, paint);
                } else {
                    stroke(&mut frame, &layer.path, paint, layer.width);
                }
            }
            output.push(frame.into_geometry());
        }
        let mut lighting = Frame::with_bounds(renderer, area);
        let main = &geometry.layers[0];
        // Pick high-amplitude lobes in separate thirds, using the same prepared geometry.
        if config.glow > 0.0 {
            let regions = if slot.expanded { 3 } else { 1 };
            for region in 0..regions {
                let from = region * PROFILE_SAMPLES / regions;
                let to = (region + 1) * PROFILE_SAMPLES / regions;
                let peak = (from..to)
                    .max_by(|&a, &b| {
                        (main.points[a].y - area.y - area.height * 0.5)
                            .abs()
                            .total_cmp(&(main.points[b].y - area.y - area.height * 0.5).abs())
                    })
                    .unwrap_or(from);
                let strength =
                    ((main.points[peak].y - area.y - area.height * 0.5).abs() / area.height * 2.0)
                        .clamp(0.0, 1.0);
                light_streak(
                    &mut lighting,
                    main,
                    area,
                    LightPulse {
                        position: peak as f32 / (PROFILE_SAMPLES - 1) as f32,
                        energy: opacity * config.glow * glow_gain * strength * 0.28,
                        width: if slot.expanded { 20.0 } else { 10.0 },
                        white: 0.0,
                        direction: 0.0,
                    },
                );
            }
        }
        let seek_energy = state
            .lighting
            .seek_energy(playback.now, playback.reduced_motion);
        let seeking = state.lighting.dragging || seek_energy > 0.02;
        if playback.playing && !playback.reduced_motion {
            for flare in traveling_flares(
                state.animation.playing_seconds,
                state.profile.seed,
                slot.expanded,
            )
            .into_iter()
            .flatten()
            {
                let competition = if seeking { 0.12 } else { 1.0 };
                let distance = (flare.position - progress).abs() * area.width;
                let separation = smoothstep(8.0, 24.0, distance);
                light_streak(
                    &mut lighting,
                    &geometry.layers[flare.layer],
                    area,
                    LightPulse {
                        position: flare.position,
                        energy: opacity * flare.energy * competition * separation,
                        width: if slot.expanded { 8.0 } else { 5.0 },
                        white: 0.04,
                        direction: 1.0,
                    },
                );
            }
        }
        // Interaction lights have their own lifetime. They never move the authoritative progress.
        if !playback.reduced_motion {
            for sample in state.lighting.trail.iter().rev().flatten() {
                let age = playback
                    .now
                    .saturating_duration_since(sample.born)
                    .as_secs_f32();
                let distance = (sample.position - progress).abs() * area.width;
                let reach = if slot.expanded { 26.0 } else { 12.0 };
                if age < 0.32 && distance < reach {
                    light_streak(
                        &mut lighting,
                        main,
                        area,
                        LightPulse {
                            position: sample.position,
                            energy: opacity
                                * seek_energy
                                * (1.0 - age / 0.32).powi(2)
                                * (1.0 - distance / reach)
                                * 0.35,
                            width: if slot.expanded { 10.0 } else { 5.0 },
                            white: 0.23,
                            direction: state.lighting.direction,
                        },
                    );
                }
            }
            if let Some(position) = state.lighting.released_position(playback.now) {
                light_streak(
                    &mut lighting,
                    main,
                    area,
                    LightPulse {
                        position,
                        energy: opacity * seek_energy * 0.4,
                        width: if slot.expanded { 16.0 } else { 8.0 },
                        white: 0.3,
                        direction: state.lighting.direction,
                    },
                );
            }
        }
        if let Some(position) = playback.hover.filter(|v| v.is_finite())
            && !state.lighting.dragging
        {
            light_streak(
                &mut lighting,
                main,
                area,
                LightPulse {
                    position,
                    energy: opacity * 0.22,
                    width: if slot.expanded { 11.0 } else { 6.0 },
                    white: 0.15,
                    direction: 0.0,
                },
            );
        }
        let orb = state
            .orb
            .visual(playback.now, &config.beat_orb, playback.reduced_motion);
        let head = playback_head_intensity(
            playback.playing,
            playback.reduced_motion,
            state.lighting.dragging,
            seek_energy,
            orb.flare,
            audio.rms * audio.blend,
        );
        let energy = head.glow * opacity;
        // Nearby thin waves catch a little light, tying the head into the layered geometry.
        for layer in geometry
            .layers
            .iter()
            .filter(|layer| !layer.ribbon)
            .take(if slot.expanded { 3 } else { 1 })
        {
            light_streak(
                &mut lighting,
                layer,
                area,
                LightPulse {
                    position: progress,
                    energy: energy * 0.28,
                    width: if slot.expanded { 10.0 } else { 5.0 },
                    white: 0.32,
                    direction: 0.0,
                },
            );
        }
        output.push(lighting.into_geometry());
        // Dedicated last pass: no supporting waveform or flare can cover the white core.
        output.push(playback_head(
            renderer,
            main,
            slot,
            playback,
            HeadLighting {
                intensity: head,
                seek_energy,
                direction: state.lighting.direction,
                orb,
                state: &state.orb,
            },
        ));
        output
    }
}

#[derive(Clone, Copy)]
struct LightPulse {
    position: f32,
    energy: f32,
    width: f32,
    white: f32,
    direction: f32,
}

struct HeadLighting<'a> {
    intensity: PlaybackHeadIntensity,
    seek_energy: f32,
    direction: f32,
    orb: OrbVisual,
    state: &'a BeatOrb,
}

fn light_streak(frame: &mut Frame, layer: &Layer, area: Rectangle, pulse: LightPulse) {
    if pulse.energy <= 0.001 {
        return;
    }
    let center = finite_ratio(pulse.position);
    let radius = pulse.width * 0.5 / area.width;
    let bias = pulse.direction * radius * 0.23;
    let start = (center - radius - bias).max(0.0);
    let end = (center + radius - bias).min(1.0);
    if end <= start {
        return;
    }
    let path = curve_path(&layer.points, start, end);
    // Local alpha feathering follows the curve: no circle primitive, thumb, or straight marker.
    for (width, gain, white) in [
        (
            if area.height > 14.0 { 6.0 } else { 3.4 },
            0.10,
            pulse.white * 0.3,
        ),
        (3.0, 0.22, pulse.white * 0.65),
        (1.1, 0.72, pulse.white),
    ] {
        stroke(
            frame,
            &path,
            pulse_palette(
                area,
                start,
                end,
                center,
                pulse.energy * gain,
                white,
                layer.cool,
            ),
            width,
        );
    }
}

/// Four composited layers: curved trail, soft outer halo, concentrated inner glow,
/// and an opaque white core. Every primitive is clipped to the existing waveform slot.
fn playback_head(
    renderer: &Renderer,
    layer: &Layer,
    slot: WaveformSlot,
    playback: WaveformPlayback,
    lighting: HeadLighting<'_>,
) -> Geometry {
    let HeadLighting {
        intensity,
        seek_energy,
        direction,
        orb,
        state: orb_state,
    } = lighting;
    let area = slot.visual;
    let expanded = slot.expanded;
    let progress = playback.progress;
    let opacity = playback.opacity;
    let mut frame = Frame::with_bounds(renderer, area);
    let position = finite_ratio(progress);
    let point = curve_point(&layer.points, position);
    let visibility = (opacity * edge_fade(position)).clamp(0.0, 1.0);
    if visibility <= 0.0 {
        return frame.into_geometry();
    }
    let clearance = (point.y - area.y)
        .min(area.y + area.height - point.y)
        .max(0.0);
    let scrubbing = matches!(intensity.state, PlaybackHeadState::Scrubbing);
    let seek = if scrubbing {
        seek_energy.max(0.65)
    } else {
        seek_energy
    };
    let length = if expanded {
        14.0 + seek * 6.0
    } else {
        8.0 + seek * 4.0
    };
    // A directional tail follows the actual primary curve, never a straight progress line.
    let direction = if scrubbing && direction < 0.0 {
        -1.0
    } else {
        1.0
    };
    let start = if direction > 0.0 {
        (position - length / area.width).max(0.0)
    } else {
        position
    };
    let end = if direction > 0.0 {
        position
    } else {
        (position + length / area.width).min(1.0)
    };
    if end > start {
        let trail = curve_path(&layer.points, start, end);
        for (width, gain, white) in [(4.0, 0.24, 0.35), (1.8, 0.70, 0.64), (0.8, 0.9, 0.88)] {
            let mut paint = gradient::Linear::new(
                Point::new(area.x + start * area.width, area.y),
                Point::new(area.x + end * area.width, area.y),
            );
            for t in [0.0, 0.16, 0.36, 0.6, 0.82, 1.0] {
                let x = start + (end - start) * t;
                let strength = if direction > 0.0 {
                    smoothstep(0.0, 1.0, t)
                } else {
                    1.0 - smoothstep(0.0, 1.0, t)
                };
                let mut color = head_color(x, white);
                color.a =
                    (opacity * edge_fade(x) * gain * intensity.glow * strength).clamp(0.0, 1.0);
                paint = paint.add_stop(t, color);
            }
            stroke(&mut frame, &trail, paint.into(), width);
        }
    }
    // Flatten halos near the visual top/bottom rather than allowing a cut-off disc.
    let dimensions = orb.fit(area.height, clearance);
    let halo_x = dimensions.halo_x;
    let halo_y = dimensions.halo_y;
    radial_glow(
        &mut frame,
        point,
        halo_x,
        halo_y,
        head_color(position, 0.0),
        visibility * (0.29 + intensity.breathing * 0.04 + orb.response * 0.16 + seek * 0.05),
    );
    let inner_x = dimensions.inner_x;
    let inner_y = dimensions.inner_y;
    radial_glow(
        &mut frame,
        point,
        inner_x,
        inner_y,
        head_color(position, 0.55),
        visibility * intensity.glow.max(0.78),
    );
    if orb.sparkle > 0.005 {
        let radius = if expanded {
            4.5 + orb.sparkle
        } else {
            2.4 + orb.sparkle * 0.6
        };
        let vertical = radius.min((clearance - 0.2).max(0.0));
        let sparkle = Path::new(|path| {
            let points = [
                (0.0, -vertical),
                (radius * 0.22, -vertical * 0.22),
                (radius, 0.0),
                (radius * 0.22, vertical * 0.22),
                (0.0, vertical),
                (-radius * 0.22, vertical * 0.22),
                (-radius, 0.0),
                (-radius * 0.22, -vertical * 0.22),
            ];
            for (index, (x, y)) in points.into_iter().enumerate() {
                let p = Point::new(point.x + x, point.y + y);
                if index == 0 {
                    path.move_to(p);
                } else {
                    path.line_to(p);
                }
            }
            path.close();
        });
        frame.fill(
            &sparkle,
            Color::from_rgba(0.92, 1.0, 1.0, visibility * orb.sparkle * 0.92),
        );
    }
    // Core opacity is independent of glow intensity, breathing, or paused state.
    // Only the global content transition and the existing horizontal edge fade apply.
    let radius = dimensions.core;
    #[cfg(debug_assertions)]
    orb_diagnostics(playback.now, orb_state, orb, dimensions);
    #[cfg(not(debug_assertions))]
    let _ = orb_state;
    if radius > 0.0 {
        frame.fill(
            &Path::circle(point, radius),
            Color::from_rgba(1.0, 1.0, 1.0, visibility),
        );
    }
    frame.into_geometry()
}

#[cfg(debug_assertions)]
fn layer_diagnostics(state: &WaveformState, geometry: &WaveGeometry, now: Instant, blend: f32) {
    if !tracing::enabled!(target: "rimflux::waveform_bands", tracing::Level::DEBUG) {
        return;
    }
    thread_local! { static LAST: std::cell::Cell<Option<Instant>> = const { std::cell::Cell::new(None) }; }
    LAST.with(|last| {
        if last.get().is_some_and(|at| now.saturating_duration_since(at).as_millis() < 250) { return; }
        last.set(Some(now));
        let f = state.audio.reactivity.features;
        let travel = TravelingLayer::ALL.map(|layer| state.traveling.parameters(layer));
        let velocities = travel.map(|p| p.angular_velocity);
        let phases = travel.map(|p| p.phase);
        let mut amplitudes = [0.0_f32; 7];
        for (value, layer) in amplitudes.iter_mut().zip(&geometry.layers) { *value = layer.response.amplitude; }
        tracing::debug!(target: "rimflux::waveform_bands",
            bass=f.bass_energy, mid=f.mid_energy, presence=f.presence_energy, high=f.high_energy,
            low_onset=f.low_onset, mid_onset=f.mid_onset, high_onset=f.high_onset,
            ripple_activity=state.audio.reactivity.activity(now), ripple_packets=state.audio.reactivity.ripple_count(),
            ?amplitudes, ?velocities, ?phases, onset_density=state.audio.reactivity.onset_density(now), blend, capture=?state.audio.snapshot().map(|s| s.status), "Final waveform layer parameters");
    });
}

#[cfg(debug_assertions)]
fn orb_diagnostics(
    now: Instant,
    state: &BeatOrb,
    visual: OrbVisual,
    dimensions: crate::modules::beat_orb::OrbDimensions,
) {
    if !tracing::enabled!(target: "rimflux::beat_orb", tracing::Level::DEBUG) {
        return;
    }
    thread_local! { static LAST: std::cell::Cell<Option<(Instant, crate::modules::beat_orb::OrbPhase)>> = const { std::cell::Cell::new(None) }; }
    LAST.with(|last| {
        if state.triggered
            || last.get().is_none_or(|(at, phase)| {
                phase != visual.phase || now.saturating_duration_since(at).as_millis() >= 250
            })
        {
            last.set(Some((now, visual.phase)));
            tracing::debug!(target: "rimflux::beat_orb",
                raw_onset = state.raw_onset, normalized_onset = state.normalized_onset,
                beat_trigger = state.triggered, phase = ?visual.phase,
                core_diameter = dimensions.core * 2.0,
                halo_width = dimensions.halo_x * 2.0, halo_height = dimensions.halo_y * 2.0,
                capture = ?state.capture_status, "Rendered beat orb (logical px)");
        }
    });
}

fn head_color(position: f32, white: f32) -> Color {
    // Restrict head lighting to cyan/blue/violet even in the waveform's magenta region.
    let base = wave_color(0.25 + finite_ratio(position) * 0.48, true);
    Color::from_rgb(
        base.r + (1.0 - base.r) * white,
        base.g + (1.0 - base.g) * white,
        base.b + (1.0 - base.b) * white,
    )
}

fn radial_glow(frame: &mut Frame, center: Point, rx: f32, ry: f32, color: Color, peak: f32) {
    if rx <= 0.0 || ry <= 0.0 {
        return;
    }
    let mut previous = 0.0_f32;
    for shell in (1..=12).rev() {
        let radius = shell as f32 / 12.0;
        let target = peak.clamp(0.0, 0.95) * (1.0 - radius * radius).powi(2);
        let alpha = ((target - previous) / (1.0 - previous)).clamp(0.0, 1.0);
        previous = target;
        if alpha <= 0.0 {
            continue;
        }
        let (x, y) = (rx * radius, ry * radius);
        let k = 0.5522848;
        let ellipse = Path::new(|path| {
            path.move_to(Point::new(center.x + x, center.y));
            path.bezier_curve_to(
                Point::new(center.x + x, center.y + k * y),
                Point::new(center.x + k * x, center.y + y),
                Point::new(center.x, center.y + y),
            );
            path.bezier_curve_to(
                Point::new(center.x - k * x, center.y + y),
                Point::new(center.x - x, center.y + k * y),
                Point::new(center.x - x, center.y),
            );
            path.bezier_curve_to(
                Point::new(center.x - x, center.y - k * y),
                Point::new(center.x - k * x, center.y - y),
                Point::new(center.x, center.y - y),
            );
            path.bezier_curve_to(
                Point::new(center.x + k * x, center.y - y),
                Point::new(center.x + x, center.y - k * y),
                Point::new(center.x + x, center.y),
            );
            path.close();
        });
        frame.fill(&ellipse, Color { a: alpha, ..color });
    }
}

fn curve_point(points: &[Point; PROFILE_SAMPLES], position: f32) -> Point {
    let index = finite_ratio(position) * (PROFILE_SAMPLES - 1) as f32;
    let left = (index as usize).min(PROFILE_SAMPLES - 2);
    let t = index - left as f32;
    Point::new(
        points[left].x + (points[left + 1].x - points[left].x) * t,
        points[left].y + (points[left + 1].y - points[left].y) * t,
    )
}
fn traveling_controls(points: &[Point; PROFILE_SAMPLES], index: usize) -> (Point, Point) {
    let tangent = |i: usize| {
        let before = if i == 0 {
            points[1].y - points[0].y
        } else {
            points[i].y - points[i - 1].y
        };
        let after = if i + 1 == PROFILE_SAMPLES {
            before
        } else {
            points[i + 1].y - points[i].y
        };
        if before * after <= 0.0 {
            0.0
        } else {
            2.0 * before * after / (before + after)
        }
    };
    let (a, b) = (points[index], points[index + 1]);
    let third = (b.x - a.x) / 3.0;
    (
        Point::new(a.x + third, a.y + tangent(index) / 3.0),
        Point::new(b.x - third, b.y - tangent(index + 1) / 3.0),
    )
}

/// Shape-preserving cubic interpolation for the three traveling lines. Control
/// points stay inside each segment's y range; x(t) stays linear and has no drift.
fn traveling_path(points: &[Point; PROFILE_SAMPLES]) -> Path {
    Path::new(|path| {
        path.move_to(points[0]);
        for index in 0..PROFILE_SAMPLES - 1 {
            let (a, b) = traveling_controls(points, index);
            path.bezier_curve_to(a, b, points[index + 1]);
        }
    })
}

fn curve_path(points: &[Point; PROFILE_SAMPLES], start: f32, end: f32) -> Path {
    Path::new(|path| {
        path.move_to(curve_point(points, start));
        for (index, point) in points.iter().enumerate() {
            let x = index as f32 / (PROFILE_SAMPLES - 1) as f32;
            if x > start && x < end {
                path.line_to(*point);
            }
        }
        path.line_to(curve_point(points, end));
    })
}
fn stroke(frame: &mut Frame, path: &Path, paint: Gradient, width: f32) {
    frame.stroke(
        path,
        Stroke {
            style: Style::Gradient(paint),
            width,
            line_cap: LineCap::Round,
            line_join: LineJoin::Round,
            ..Stroke::default()
        },
    );
}
fn smoothstep(low: f32, high: f32, value: f32) -> f32 {
    let t = ((value - low) / (high - low)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
fn finite_ratio(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.0
    }
}
fn edge_fade(x: f32) -> f32 {
    smoothstep(0.0, 0.1, x) * (1.0 - smoothstep(0.9, 1.0, x))
}

// Eight alpha stops fit iced's native gradient format. Both ends sample the smooth
// envelope, while the middle retains the original cyan/blue/violet/magenta identity.
fn palette(area: Rectangle, opacity: f32, cool: bool) -> Gradient {
    let mut gradient = gradient::Linear::new(
        Point::new(area.x, area.y),
        Point::new(area.x + area.width, area.y),
    );
    for x in [0.0, 0.035, 0.1, 0.38, 0.60, 0.9, 0.965, 1.0] {
        let mut color = wave_color(x, cool);
        color.a = opacity * edge_fade(x);
        gradient = gradient.add_stop(x, color);
    }
    gradient.into()
}
fn pulse_palette(
    area: Rectangle,
    start: f32,
    end: f32,
    center: f32,
    opacity: f32,
    white: f32,
    cool: bool,
) -> Gradient {
    let mut gradient = gradient::Linear::new(
        Point::new(area.x + start * area.width, area.y),
        Point::new(area.x + end * area.width, area.y),
    );
    let peak = ((center - start) / (end - start)).clamp(0.05, 0.95);
    for local in [
        0.0,
        peak * 0.35,
        peak * 0.7,
        peak,
        peak + (1.0 - peak) * 0.3,
        peak + (1.0 - peak) * 0.65,
        1.0,
    ] {
        let x = start + (end - start) * local;
        let feather = smoothstep(0.0, peak, local) * (1.0 - smoothstep(peak, 1.0, local));
        let mut color = wave_color(x, cool);
        color.r += (1.0 - color.r) * white;
        color.g += (1.0 - color.g) * white;
        color.b += (1.0 - color.b) * white;
        color.a = opacity * feather * edge_fade(x);
        gradient = gradient.add_stop(local, color);
    }
    gradient.into()
}
fn wave_color(x: f32, cool: bool) -> Color {
    let stops: &[(f32, (u8, u8, u8))] = if cool {
        &[
            (0.0, (9, 242, 255)),
            (0.26, (45, 99, 252)),
            (0.52, (10, 190, 255)),
            (0.76, (130, 72, 253)),
            (1.0, (243, 41, 218)),
        ]
    } else {
        &[
            (0.0, (71, 249, 108)),
            (0.18, (0, 232, 218)),
            (0.38, (17, 217, 255)),
            (0.52, (34, 108, 255)),
            (0.66, (119, 71, 255)),
            (0.83, (252, 39, 215)),
            (1.0, (227, 0, 168)),
        ]
    };
    let pair = stops
        .windows(2)
        .find(|pair| x <= pair[1].0)
        .unwrap_or(&stops[stops.len() - 2..]);
    let t = ((x - pair[0].0) / (pair[1].0 - pair[0].0)).clamp(0.0, 1.0);
    let (ar, ag, ab) = pair[0].1;
    let (br, bg, bb) = pair[1].1;
    let mix = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t) / 255.0;
    Color::from_rgb(mix(ar, br), mix(ag, bg), mix(ab, bb))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        core::state::{NotchExpansion, NotchMode, dimensions},
        modules::music::MusicInfo,
    };

    #[test]
    fn traveling_clocks_change_only_three_thin_layers_and_keep_bass_geometry_identical() {
        use crate::modules::traveling_wave::TravelingInput;
        let config = WaveformConfig::default();
        let mut state = WaveformState::new(&MusicInfo::demo());
        let now = Instant::now();
        let slot = WaveformSlot::new(
            NotchExpansion::Expanded,
            dimensions(NotchMode::Music, NotchExpansion::Expanded),
            &config,
        );
        let before = WaveGeometry::prepare(slot, &state, &config, now);
        state.traveling.advance(
            std::time::Duration::from_millis(100),
            TravelingInput {
                playing: true,
                ..Default::default()
            },
        );
        let after = WaveGeometry::prepare(slot, &state, &config, now);
        for index in 0..7 {
            let moved = before.layers[index]
                .points
                .iter()
                .zip(&after.layers[index].points)
                .any(|(a, b)| (a.y - b.y).abs() > 0.01);
            assert_eq!(
                moved,
                [2, 3, 4].contains(&index),
                "unexpected traveling layer {index}"
            );
            for (a, b) in before.layers[index]
                .points
                .iter()
                .zip(&after.layers[index].points)
            {
                assert_eq!(a.x, b.x);
            }
            if [2, 3, 4].contains(&index) {
                let points = &after.layers[index].points;
                for i in 0..PROFILE_SAMPLES - 1 {
                    let (a, b) = traveling_controls(points, i);
                    let low = points[i].y.min(points[i + 1].y) - 0.0001;
                    let high = points[i].y.max(points[i + 1].y) + 0.0001;
                    assert!((low..=high).contains(&a.y) && (low..=high).contains(&b.y));
                }
            }
        }
    }

    #[test]
    fn measured_band_changes_reach_final_geometry_and_preserve_horizontal_coordinates() {
        use crate::services::audio::{
            AudioCaptureStatus, AudioFeatures, AudioSnapshot, AudioTransient,
        };
        fn driven(features: AudioFeatures) -> (WaveformState, Instant) {
            let mut state = WaveformState::new(&MusicInfo::demo());
            let start = Instant::now();
            for i in 1..=90 {
                let now = start + std::time::Duration::from_millis(i * 33);
                state.audio.receive(AudioSnapshot {
                    status: AudioCaptureStatus::Live,
                    features,
                    captured_at: now,
                    ..AudioSnapshot::unavailable("synthetic")
                });
                state.advance(
                    std::time::Duration::from_millis(33),
                    true,
                    &WaveformConfig::default(),
                    now,
                );
            }
            (state, start + std::time::Duration::from_millis(2970))
        }
        let config = WaveformConfig::default();
        let (quiet, now) = driven(AudioFeatures::default());
        let slot = WaveformSlot::new(
            NotchExpansion::Expanded,
            dimensions(NotchMode::Music, NotchExpansion::Expanded),
            &config,
        );
        let calm = WaveGeometry::prepare(slot, &quiet, &config, now);
        for (features, index) in [
            (
                AudioFeatures {
                    bass_energy: 1.0,
                    ..Default::default()
                },
                0,
            ),
            (
                AudioFeatures {
                    mid_energy: 1.0,
                    ..Default::default()
                },
                3,
            ),
            (
                AudioFeatures {
                    high_energy: 1.0,
                    ..Default::default()
                },
                4,
            ),
        ] {
            let (state, now) = driven(features);
            let active = WaveGeometry::prepare(slot, &state, &config, now);
            let mut displacement = 0.0_f32;
            for (q, p) in calm.layers[index]
                .points
                .iter()
                .zip(active.layers[index].points)
            {
                assert_eq!(q.x, p.x);
                displacement = displacement.max((q.y - p.y).abs());
            }
            println!(
                "renderer layer {index}: max quiet/active vertical difference={displacement:.3} logical px"
            );
            assert!(
                displacement > 0.5,
                "band features were flattened by rendering"
            );
        }
        let (mut state, now) = driven(AudioFeatures {
            overall_energy: 1.0,
            bass_energy: 1.0,
            mid_energy: 1.0,
            presence_energy: 1.0,
            high_energy: 1.0,
            low_onset: 1.0,
            mid_onset: 1.0,
            high_onset: 1.0,
        });
        let mut events = [None; 8];
        events[0] = Some(AudioTransient {
            at: now - std::time::Duration::from_millis(20),
            low: 1.0,
            mid: 1.0,
            high: 1.0,
        });
        state
            .audio
            .reactivity
            .ingest(&events, now, now - std::time::Duration::from_secs(1));
        for expansion in [NotchExpansion::Compact, NotchExpansion::Expanded] {
            let slot =
                WaveformSlot::new(expansion, dimensions(NotchMode::Music, expansion), &config);
            for phase in 0..48 {
                state.animation.phase = phase as f64 * 0.25;
                let active = WaveGeometry::prepare(slot, &state, &config, now);
                for layer in active.layers {
                    for p in layer.points {
                        assert!(p.y >= slot.visual.y && p.y <= slot.visual.y + slot.visual.height);
                    }
                }
            }
        }
    }

    #[test]
    fn animated_head_fits_compact_and_expanded_curves_without_moving_time_mapping() {
        let config = WaveformConfig::default();
        let mut state = WaveformState::new(&MusicInfo::demo());
        let now = Instant::now();
        let peak = OrbVisual {
            core_diameter: 6.0,
            inner_diameter: 11.0,
            halo_diameter: 22.0,
            response: 1.0,
            flare: 1.0,
            sparkle: 1.0,
            phase: crate::modules::beat_orb::OrbPhase::Hold,
        };
        for expansion in [NotchExpansion::Compact, NotchExpansion::Expanded] {
            let slot =
                WaveformSlot::new(expansion, dimensions(NotchMode::Music, expansion), &config);
            let mut max_core = 0.0_f32;
            let mut max_halo = 0.0_f32;
            for phase in 0..48 {
                state.animation.phase = phase as f64 * 0.25;
                let prepared = WaveGeometry::prepare(slot, &state, &config, now);
                for index in 0..101 {
                    let progress = index as f32 / 100.0;
                    let point = curve_point(&prepared.layers[0].points, progress);
                    assert!(
                        (point.x - (slot.visual.x + progress * slot.visual.width)).abs() < 0.001
                    );
                    let clearance =
                        (point.y - slot.visual.y).min(slot.visual.y + slot.visual.height - point.y);
                    let fitted = peak.fit(slot.visual.height, clearance);
                    for radius in [fitted.core, fitted.inner_y, fitted.halo_y] {
                        assert!(point.y - radius >= slot.visual.y);
                        assert!(point.y + radius <= slot.visual.y + slot.visual.height);
                    }
                    max_core = max_core.max(fitted.core * 2.0);
                    max_halo = max_halo.max(fitted.halo_x * 2.0);
                }
            }
            assert!((max_core - 6.0).abs() < 0.001);
            assert!(max_halo <= slot.visual.height);
            println!(
                "{expansion:?}: visual_height={}, max_fitted_core={max_core}, max_fitted_halo_width={max_halo}",
                slot.visual.height
            );
        }
    }
}
