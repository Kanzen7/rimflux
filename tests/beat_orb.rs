//! Synthetic PCM only: these tests do not open WASAPI, play music, or access a device.
use rimflux::{
    core::config::{BeatOrbConfig, Config},
    modules::beat_orb::{BeatOrb, OrbPhase},
    services::audio::{
        AnalysisSettings, AudioAnalyzer, AudioCaptureStatus, AudioSample, AudioSnapshot,
    },
};
use std::time::{Duration, Instant};

const RATE: u32 = 48_000;
fn at(start: Instant, seconds: f64) -> Instant {
    start + Duration::from_secs_f64(seconds)
}
fn kick(t: f64) -> f32 {
    if !(0.0..0.16).contains(&t) {
        return 0.0;
    }
    ((std::f64::consts::TAU * (60.0 * t + 0.45 * (1.0 - (-t / 0.012).exp()))).sin()
        * (-t / 0.025).exp()) as f32
}
fn snare(t: f64) -> f32 {
    if !(0.0..0.16).contains(&t) {
        return 0.0;
    }
    let tone: f64 = [730.0, 1510.0, 2930.0, 4770.0]
        .iter()
        .enumerate()
        .map(|(i, f)| (std::f64::consts::TAU * f * t + i as f64).sin())
        .sum();
    (tone * 0.25 * (-t / 0.022).exp()) as f32
}

fn measure(name: &str, sample: impl Fn(f64) -> f32) -> Vec<AudioSnapshot> {
    let config = BeatOrbConfig::default();
    let mut analyzer = AudioAnalyzer::new(RATE, AnalysisSettings::from(&config));
    let start = Instant::now();
    let mut events = Vec::new();
    let mut last = None;
    for i in 0..RATE * 4 {
        let now = at(start, i as f64 / RATE as f64);
        let value = sample(i as f64 / RATE as f64);
        analyzer.push(
            AudioSample {
                mono: value,
                power: value * value,
            },
            now,
        );
        if analyzer.snapshot.onset_at.is_some() && analyzer.snapshot.onset_at != last {
            last = analyzer.snapshot.onset_at;
            events.push(analyzer.snapshot.clone());
        }
    }
    let mut max_core = 3.0_f32;
    let mut max_halo = 12.0_f32;
    let mut intensity = 0.0_f32;
    let mut raw = 0.0_f32;
    for s in &events {
        let mut orb = BeatOrb::default();
        orb.update(s.captured_at, Some(s), &config, true, false, false);
        let peak_at = s.onset_at.unwrap() + Duration::from_millis(config.attack_ms);
        let visual = orb.visual(peak_at, &config, false);
        let fitted = visual.fit(25.0, 12.5);
        max_core = max_core.max(fitted.core * 2.0);
        max_halo = max_halo.max(fitted.halo_x * 2.0);
        intensity = intensity.max(s.onset_strength);
        raw = raw.max(s.raw_onset_strength);
        let end = peak_at + Duration::from_millis(config.peak_hold_ms + config.release_ms);
        assert!((orb.visual(end, &config, false).core_diameter - 3.0).abs() < 0.0001);
    }
    println!(
        "{name}: triggers={}, raw_max={raw:.5}, normalized_max={intensity:.4}, fitted_core=3.000..{max_core:.3}px, fitted_halo=12.000..{max_halo:.3}px (25px slot, centered)",
        events.len()
    );
    events
}

#[test]
fn measured_kicks_scale_with_strength_and_fast_patterns_are_not_400ms_gated() {
    let weak = measure("quiet kick", |t| {
        (1..=3).map(|i| 0.045 * kick(t - i as f64)).sum()
    });
    let slow = measure("slow kick", |t| {
        (1..=3).map(|i| 0.30 * kick(t - i as f64)).sum()
    });
    let strong = measure("strong kick", |t| {
        (1..=3).map(|i| 0.90 * kick(t - i as f64)).sum()
    });
    for events in [&weak, &slow, &strong] {
        assert_eq!(events.len(), 3);
    }
    let peak = |events: &[AudioSnapshot]| {
        events
            .iter()
            .map(|s| s.onset_strength)
            .fold(0.0_f32, f32::max)
    };
    assert!(peak(&weak) < peak(&slow));
    assert!(peak(&slow) < peak(&strong));
    assert!(strong.iter().all(|s| s.low_onset > s.high_onset));
    let mixed = measure("weak/medium/strong in one mix", |t| {
        0.045 * kick(t - 1.0) + 0.30 * kick(t - 2.0) + 0.90 * kick(t - 3.0)
    });
    assert_eq!(mixed.len(), 3);
    assert!(
        mixed
            .windows(2)
            .all(|w| w[0].onset_strength < w[1].onset_strength)
    );
    let fast = measure("fast kick / 125ms", |t| {
        (0..16)
            .map(|i| 0.70 * kick(t - 1.0 - i as f64 * 0.125))
            .sum()
    });
    assert_eq!(fast.len(), 16);
    assert!(fast.windows(2).all(|w| {
        w[1].onset_at
            .unwrap()
            .duration_since(w[0].onset_at.unwrap())
            >= Duration::from_millis(85)
    }));
}

#[test]
fn snare_band_sustained_bass_quiet_passages_and_silence() {
    let snares = measure("snare-like mid/high", |t| {
        (1..=3).map(|i| 0.8 * snare(t - i as f64)).sum()
    });
    assert_eq!(snares.len(), 3);
    assert!(snares.iter().all(|s| s.high_onset > s.low_onset));
    let sustained = measure("sustained bass", |t| {
        if t >= 1.0 {
            (std::f64::consts::TAU * 65.0 * t).sin() as f32 * 0.7
        } else {
            0.0
        }
    });
    assert!(sustained.len() <= 1, "sustained bass retriggered");
    assert!(measure("silence", |_| 0.0).is_empty());
    assert!(
        measure(
            "quiet gradual passage",
            |t| (std::f64::consts::TAU * 440.0 * t).sin() as f32 * (t / 4.0) as f32 * 0.006
        )
        .is_empty()
    );
}

fn hit(now: Instant, strength: f32) -> AudioSnapshot {
    AudioSnapshot {
        status: AudioCaptureStatus::Live,
        onset_at: Some(now),
        onset_strength: strength,
        raw_onset_strength: strength * 0.1,
        captured_at: now,
        ..AudioSnapshot::unavailable("test")
    }
}

#[test]
fn envelope_pause_seek_resume_unavailable_and_reduced_motion() {
    let config = BeatOrbConfig::default();
    let start = Instant::now();
    let mut orb = BeatOrb::default();
    let s = hit(start, 1.0);
    orb.update(start, Some(&s), &config, true, false, false);
    assert_eq!(orb.visual(start, &config, false).core_diameter, 3.0);
    assert_eq!(
        orb.visual(at(start, 0.020), &config, false).core_diameter,
        6.0
    );
    assert_eq!(
        orb.visual(at(start, 0.020), &config, false).halo_diameter,
        22.0
    );
    let pause = at(start, 0.025);
    orb.update(pause, None, &config, false, false, false);
    assert!(!orb.triggered);
    assert_eq!(orb.visual(pause, &config, false).core_diameter, 6.0);
    assert!(orb.visual(at(start, 0.08), &config, false).core_diameter < 6.0);
    orb.update(at(start, 0.18), None, &config, false, false, false);
    assert!(!orb.is_animating());
    assert_eq!(
        orb.visual(at(start, 0.18), &config, false).core_diameter,
        3.0
    );
    let s = hit(at(start, 0.3), 0.8);
    orb.update(s.captured_at, Some(&s), &config, true, true, false);
    assert!(!orb.triggered);
    orb.update(at(start, 0.31), Some(&s), &config, true, false, false);
    assert!(!orb.triggered, "a suppressed seek event replayed");
    let resume = hit(at(start, 0.5), 0.8);
    orb.update(
        resume.captured_at,
        Some(&resume),
        &config,
        true,
        false,
        false,
    );
    assert!(orb.triggered);
    assert!(orb.visual(at(start, 0.52), &config, false).core_diameter > 5.0);
    orb.update(at(start, 0.53), Some(&resume), &config, true, false, true);
    assert_eq!(
        orb.visual(at(start, 0.53), &config, true).phase,
        OrbPhase::Rest
    );
    assert!(!orb.is_animating());
    let unavailable = AudioSnapshot::unavailable("no endpoint");
    orb.update(
        at(start, 4.0),
        Some(&unavailable),
        &config,
        true,
        false,
        false,
    );
    assert!(!orb.triggered);
    assert_eq!(
        orb.visual(at(start, 4.0), &config, false).core_diameter,
        3.0
    );
}

#[test]
fn logical_geometry_is_bounded_and_configuration_is_backwards_compatible() {
    let config = BeatOrbConfig::default();
    let start = Instant::now();
    let mut orb = BeatOrb::default();
    let s = hit(start, 1.0);
    orb.update(start, Some(&s), &config, true, false, false);
    for height in [1.0_f32, 8.0, 14.0, 25.0] {
        for ms in 0..200 {
            for step in 0..101 {
                let clearance = height * 0.5 * step as f32 / 100.0;
                let d = orb
                    .visual(start + Duration::from_millis(ms), &config, false)
                    .fit(height, clearance);
                assert!(d.core * 2.0 <= 6.0);
                for radius in [d.core, d.inner_y, d.halo_y] {
                    assert!(radius <= clearance && radius * 2.0 <= height);
                }
                assert!(d.halo_x * 2.0 <= height);
            }
        }
    }
    assert_eq!(
        Config::parse("")
            .unwrap()
            .music
            .waveform
            .beat_orb
            .maximum_core_diameter,
        6.0
    );
    let parsed = Config::parse("[music.waveform.beat_orb]\nsensitivity=nan\nminimum_core_diameter=99\nmaximum_core_diameter=-1\nminimum_halo_diameter=99\nmaximum_halo_diameter=-1\nattack_ms=1\nrelease_ms=999\ncooldown_ms=1\nlow_threshold=0.45\nstrong_threshold=0.1\nvery_strong_threshold=0.2").unwrap();
    let c = parsed.music.waveform.beat_orb;
    assert_eq!(
        (
            c.sensitivity,
            c.minimum_core_diameter,
            c.maximum_core_diameter
        ),
        (0.65, 6.0, 6.0)
    );
    assert_eq!(
        (c.minimum_halo_diameter, c.maximum_halo_diameter),
        (22.0, 22.0)
    );
    assert_eq!((c.attack_ms, c.release_ms, c.cooldown_ms), (15, 160, 70));
    assert!(c.low_threshold < c.strong_threshold && c.strong_threshold < c.very_strong_threshold);
}

#[cfg(target_os = "windows")]
#[test]
fn audio_updates_leave_authoritative_time_and_pointer_seek_unchanged() {
    use rimflux::{
        core::state::{Action, AppState, NotchExpansion, NotchMode},
        modules::music::{MusicInfo, PlaybackTimeline},
    };
    let start = Instant::now();
    let mut config = Config::default();
    config.notch.default_mode = NotchMode::Music;
    let mut state = AppState::new(config, start);
    state.music = MusicInfo::demo();
    state.music.demo = false;
    state.music.timeline = Some(PlaybackTimeline {
        start: 20.0,
        end: 220.0,
        position: 70.0,
        min_seek: 20.0,
        max_seek: 220.0,
        rate: 1.0,
        captured_at: start,
    });
    state.advance(at(start, 0.4));
    let position = state.music.position;
    state.receive_audio(hit(at(start, 0.4), 1.0), at(start, 0.4));
    assert!(state.waveform.orb.triggered);
    assert_eq!(state.music.position, position);
    assert_eq!(
        state.music.timeline.as_ref().unwrap().seek_ticks(0.5),
        Some(1_200_000_000)
    );
    state.action(
        Action::Select(NotchMode::Music, NotchExpansion::Compact),
        at(start, 0.42),
    );
    state.action(Action::SeekPreview(0.75), at(start, 0.43));
    let position = state.music.position;
    state.receive_audio(hit(at(start, 0.55), 1.0), at(start, 0.55));
    assert!(!state.waveform.orb.triggered);
    assert_eq!(state.waveform_progress(), 0.75);
    assert_eq!(state.music.position, position);
    state.advance(at(start, 0.65));
    assert!((state.music.position - 50.65).abs() < 0.0001);
    assert_eq!(state.waveform_progress(), 0.75);
}
