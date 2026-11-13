//! Headless synthetic PCM and prepared-renderer mapping. No device or playback control.
use rimflux::{
    core::config::{Config, ReactivityConfig, WaveformConfig},
    modules::waveform_reactivity::{MusicalLayer, WaveformReactivity},
    services::audio::{
        AnalysisSettings, AudioAnalyzer, AudioFeatures, AudioSample, AudioSnapshot, AudioTransient,
    },
};
use std::time::{Duration, Instant};
const RATE: u32 = 48_000;
fn at(start: Instant, t: f64) -> Instant {
    start + Duration::from_secs_f64(t)
}
fn analyze(signal: impl Fn(f64) -> f32) -> (AudioSnapshot, Vec<Instant>) {
    let start = Instant::now();
    let mut analyzer = AudioAnalyzer::new(RATE, AnalysisSettings::from(&WaveformConfig::default()));
    let mut events = Vec::new();
    for i in 0..RATE * 3 {
        let t = i as f64 / RATE as f64;
        let value = signal(t);
        analyzer.push(
            AudioSample {
                mono: value,
                power: value * value,
            },
            at(start, t),
        );
        if let Some(e) = analyzer.snapshot.transients[0]
            && events.last() != Some(&e.at)
        {
            events.push(e.at);
        }
        if i % 512 == 0 {
            let f = analyzer.snapshot.features;
            for v in [
                f.overall_energy,
                f.bass_energy,
                f.mid_energy,
                f.presence_energy,
                f.high_energy,
                f.low_onset,
                f.mid_onset,
                f.high_onset,
            ] {
                assert!(v.is_finite() && (0.0..=1.0).contains(&v));
            }
        }
    }
    (analyzer.snapshot, events)
}
fn tone(frequency: f64, time: f64) -> f32 {
    (time * frequency * std::f64::consts::TAU).sin() as f32
}

#[test]
fn measured_bands_separate_bass_mids_presence_and_treble() {
    for (index, hz) in [65.0, 500.0, 3000.0, 8000.0].into_iter().enumerate() {
        let (s, _) = analyze(|t| tone(hz, t) * 0.15);
        let f = s.features;
        let bands = [
            f.bass_energy,
            f.mid_energy,
            f.presence_energy,
            f.high_energy,
        ];
        assert!(bands[index] > 0.6, "{hz}: {bands:?}");
        for (i, value) in bands.into_iter().enumerate() {
            if i != index {
                assert!(value < 0.02, "{hz}: cross-band leakage {bands:?}");
            }
        }
        println!("tone {hz}Hz: bass/mid/presence/high={bands:?}");
    }
    let (silence, events) = analyze(|_| 0.0);
    assert!(events.is_empty());
    assert_eq!(silence.features.overall_energy, 0.0);
    assert_eq!(silence.features.high_onset, 0.0);
}

#[test]
fn fast_treble_attacks_emit_independent_events_but_sustained_tones_do_not() {
    let (_, fast) = analyze(|t| {
        (0..16)
            .map(|i| {
                let age = t - 1.0 - i as f64 * 0.0625;
                if (0.0..0.03).contains(&age) {
                    tone(8000.0, age) * (-age / 0.007).exp() as f32 * 0.8
                } else {
                    0.0
                }
            })
            .sum()
    });
    println!("62.5ms treble attacks: {} events", fast.len());
    assert_eq!(fast.len(), 16);
    let (_, sustained) = analyze(|t| if t >= 1.0 { tone(65.0, t) * 0.4 } else { 0.0 });
    assert!(
        sustained.len() <= 1,
        "sustained tone retriggered {} times",
        sustained.len()
    );
}

fn settle(target: AudioFeatures) -> (WaveformReactivity, Instant) {
    let start = Instant::now();
    let mut model = WaveformReactivity::default();
    for step in 1..=150 {
        model.advance(0.02, at(start, step as f64 * 0.02), target, true);
    }
    (model, at(start, 3.0))
}

#[test]
fn final_layer_mapping_responds_independently_and_quiet_music_calms_it() {
    let config = ReactivityConfig::default();
    let roles = [
        MusicalLayer::Bass,
        MusicalLayer::Mid,
        MusicalLayer::Percussion,
        MusicalLayer::High,
        MusicalLayer::Ambient,
    ];
    let (quiet, now) = settle(AudioFeatures::default());
    let quiet_amplitudes = roles.map(|role| quiet.response(role, &config, 1.0, now).amplitude);
    for (target, expected) in [
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
            1,
        ),
        (
            AudioFeatures {
                presence_energy: 1.0,
                ..Default::default()
            },
            2,
        ),
        (
            AudioFeatures {
                high_energy: 1.0,
                ..Default::default()
            },
            3,
        ),
        (
            AudioFeatures {
                overall_energy: 1.0,
                ..Default::default()
            },
            4,
        ),
    ] {
        let (model, now) = settle(target);
        let amplitudes = roles.map(|role| model.response(role, &config, 1.0, now).amplitude);
        assert!(amplitudes[expected] - quiet_amplitudes[expected] > 0.4);
        for i in 0..5 {
            if i != expected {
                assert!((amplitudes[i] - quiet_amplitudes[i]).abs() < 0.001);
            }
        }
        println!("isolated layer {expected}: final amplitude multipliers {amplitudes:?}");
    }
    assert_eq!(
        quiet
            .response(MusicalLayer::High, &config, 0.0, now)
            .amplitude,
        1.0,
        "unavailable capture must preserve procedural fallback"
    );
    let (mid_attack, now) = settle(AudioFeatures {
        mid_onset: 1.0,
        ..Default::default()
    });
    assert!(
        (mid_attack
            .response(MusicalLayer::High, &config, 1.0, now)
            .opacity
            - 0.2)
            .abs()
            < 0.001,
        "mid-only onsets must not light the treble layer"
    );
}

#[test]
fn ripples_are_local_bounded_deduplicated_and_density_follows_event_spacing() {
    let start = Instant::now();
    let mut model = WaveformReactivity::default();
    let mut events = [None; 8];
    events[0] = Some(AudioTransient {
        at: start,
        low: 0.0,
        mid: 0.7,
        high: 0.5,
    });
    model.ingest(&events, start, start);
    model.ingest(&events, start, start);
    assert_eq!(model.transient_count(), 1);
    let now = at(start, 0.03);
    let samples: Vec<f32> = (0..201)
        .map(|i| model.displacement(i as f32 / 200.0, now))
        .collect();
    assert!(samples.iter().any(|v| v.abs() > 0.01));
    assert!(
        samples.iter().filter(|v| v.abs() > 0.001).count() < 60,
        "ripple moved the whole curve"
    );
    assert!(samples.iter().all(|v| v.abs() <= 0.24));
    let activity = model.activity(now);
    assert!(model.activity(at(start, 0.2)) < activity);
    model.advance(0.1, at(start, 0.4), AudioFeatures::default(), false);
    assert_eq!(model.ripple_count(), 0);
    assert_eq!(model.displacement(0.5, at(start, 0.4)), 0.0);
    model.ingest(&events, at(start, 0.5), at(start, 0.1));
    assert_eq!(model.ripple_count(), 0, "pre-pause event replayed");

    // Same hit strength and position; only the prior-event spacing differs.
    fn spatial_variation(spacing: f64) -> f32 {
        let start = Instant::now();
        let mut model = WaveformReactivity::default();
        let mut events = [None; 8];
        for t in [0.0, spacing] {
            events[0] = Some(AudioTransient {
                at: at(start, t),
                low: 0.0,
                mid: 0.8,
                high: 0.8,
            });
            model.ingest(&events, at(start, t), start);
        }
        let now = at(start, spacing + 0.03);
        let values: Vec<f32> = (0..1001)
            .map(|i| model.displacement(i as f32 / 1000.0, now))
            .collect();
        values.windows(2).map(|w| (w[1] - w[0]).abs()).sum()
    }
    assert!(spatial_variation(0.08) > spatial_variation(1.0));
}

#[test]
fn reactivity_config_defaults_and_clamps_without_affecting_orb() {
    let old = Config::parse("[music.waveform]\nmode='hybrid'").unwrap();
    assert!(old.music.waveform.reactivity.micro_ripples);
    let invalid = Config::parse("[music.waveform.reactivity]\nbass_sensitivity=nan\nmid_sensitivity=-5\nhigh_sensitivity=99\ntransient_sensitivity=inf\nenabled=false\nadaptive_normalization=false").unwrap();
    let r = invalid.music.waveform.reactivity;
    assert_eq!(
        (
            r.bass_sensitivity,
            r.mid_sensitivity,
            r.high_sensitivity,
            r.transient_sensitivity
        ),
        (0.65, 0.0, 1.0, 0.75)
    );
    assert!(!r.enabled && !r.adaptive_normalization);
    assert!(invalid.music.waveform.beat_orb.enabled);
}

#[cfg(target_os = "windows")]
#[test]
fn pause_clears_micro_ripples_finishes_fading_and_reduced_motion_stops_capture() {
    use rimflux::{
        core::state::{AppState, NotchMode},
        modules::music::MusicInfo,
        services::audio::AudioCaptureStatus,
    };
    let start = Instant::now();
    let mut config = Config::default();
    config.notch.default_mode = NotchMode::Music;
    let mut state = AppState::new(config, start);
    state.music = MusicInfo::demo();
    state.music.demo = false;
    let mut s = AudioSnapshot {
        status: AudioCaptureStatus::Live,
        captured_at: at(start, 1.0),
        features: AudioFeatures {
            overall_energy: 0.7,
            high_energy: 0.8,
            high_onset: 0.8,
            ..Default::default()
        },
        ..AudioSnapshot::unavailable("synthetic")
    };
    s.transients[0] = Some(AudioTransient {
        at: s.captured_at,
        low: 0.0,
        mid: 0.3,
        high: 0.8,
    });
    state.receive_audio(s, at(start, 1.0));
    assert_eq!(state.waveform.audio.reactivity.ripple_count(), 1);
    assert_eq!(state.frame_interval(), Some(Duration::from_millis(16)));
    state.advance(at(start, 1.02));
    state.music.playing = false;
    state.waveform.audio.clear();
    assert_eq!(state.waveform.audio.reactivity.ripple_count(), 0);
    for step in 2..150 {
        state.advance(at(start, 1.0 + step as f64 * 0.02));
    }
    assert!(state.frame_interval().is_none());
    assert!(!state.wants_audio_capture());
    state.music.playing = true;
    state.config.appearance.reduced_motion = true;
    assert!(!state.wants_audio_capture());
    assert!(state.frame_interval().is_none());
}
