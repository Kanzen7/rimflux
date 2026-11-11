use rimflux::{
    modules::{
        traveling_wave::{TravelingInput, TravelingLayer, TravelingWaves},
        waveform_reactivity::{MAX_RIPPLE_PACKETS, WaveformReactivity},
    },
    services::audio::{AudioFeatures, AudioTransient},
};
use std::time::{Duration, Instant};

fn dense() -> TravelingInput {
    TravelingInput {
        features: AudioFeatures {
            mid_energy: 1.0,
            presence_energy: 1.0,
            high_energy: 1.0,
            mid_onset: 1.0,
            high_onset: 1.0,
            ..Default::default()
        },
        blend: 1.0,
        onset_density: 1.0,
        activity: 1.0,
        playing: true,
        ..Default::default()
    }
}

#[test]
fn crests_propagate_to_the_right_instead_of_pulsing_in_place() {
    let mut waves = TravelingWaves::default();
    let input = TravelingInput {
        features: AudioFeatures {
            mid_energy: 0.7,
            high_energy: 0.5,
            ..Default::default()
        },
        blend: 1.0,
        playing: true,
        ..Default::default()
    };
    waves.advance(Duration::from_secs(1), input);
    let before = TravelingLayer::ALL.map(|layer| waves.parameters(layer));
    waves.advance(Duration::from_millis(33), input);
    for (layer, previous) in TravelingLayer::ALL.into_iter().zip(before) {
        let current = waves.parameters(layer);
        let right_shift =
            (current.phase - previous.phase) / (std::f64::consts::TAU * current.cycles);
        assert!(right_shift > 0.0);
        for x in [0.21_f32, 0.43, 0.65] {
            assert!(
                (previous.sample(x, 0.6) - current.sample(x + right_shift as f32, 0.6)).abs()
                    < 0.00002,
                "pattern did not translate right for {layer:?}"
            );
        }
        println!(
            "{layer:?}: velocity={:.3}rad/s, 33ms crest shift={:.3}px at 268px width",
            current.angular_velocity,
            right_shift * 268.0
        );
    }
}

#[test]
fn phase_integration_is_frame_rate_independent_and_velocity_is_bounded() {
    fn simulate(fps: u32) -> TravelingWaves {
        let mut model = TravelingWaves::default();
        for _ in 0..fps * 3 {
            model.advance(Duration::from_secs_f64(1.0 / fps as f64), dense());
        }
        model
    }
    let a = simulate(30);
    let b = simulate(120);
    let mut previous_velocity = 0.0;
    let mut previous_crest_speed = 0.0;
    for layer in TravelingLayer::ALL {
        let pa = a.parameters(layer);
        let pb = b.parameters(layer);
        assert!((pa.phase - pb.phase).abs() < 0.00001);
        assert!((pa.angular_velocity - pb.angular_velocity).abs() < 0.00001);
        assert!(pa.angular_velocity <= layer.tuning().maximum_velocity);
        assert!(pa.angular_velocity > previous_velocity);
        previous_velocity = pa.angular_velocity;
        let crest_speed = pa.angular_velocity / (std::f64::consts::TAU * pa.cycles);
        assert!(
            crest_speed > previous_crest_speed,
            "distinct angular velocities must also produce distinct rightward crest speeds"
        );
        previous_crest_speed = crest_speed;
        assert_eq!(pa.cycles, layer.tuning().cycles);
        assert!(pa.opacity >= 0.9 && pa.opacity <= 1.0);
    }
}

#[test]
fn onset_and_capture_changes_do_not_reset_phase_and_pause_freezes_it() {
    let mut waves = TravelingWaves::default();
    waves.advance(Duration::from_secs(2), dense());
    let before = waves.parameters(TravelingLayer::Micro);
    let fallback = TravelingInput {
        playing: true,
        ..Default::default()
    };
    waves.advance(Duration::from_millis(1), fallback);
    let after = waves.parameters(TravelingLayer::Micro);
    assert!(after.phase > before.phase);
    assert!((after.angular_velocity - before.angular_velocity).abs() < 0.02);
    assert!(after.phase - before.phase < 0.009);
    waves.advance(Duration::from_secs(10), TravelingInput::default());
    assert_eq!(after.phase, waves.parameters(TravelingLayer::Micro).phase);
    assert_eq!(after.cycles, before.cycles);
}

#[test]
fn loudness_alone_does_not_create_dense_rhythm_and_packets_have_one_direction() {
    let start = Instant::now();
    let mut slow = WaveformReactivity::default();
    let mut fast = WaveformReactivity::default();
    for (model, spacing) in [(&mut slow, 1.0), (&mut fast, 0.08)] {
        for i in 0..12 {
            let now = start + Duration::from_secs_f32(i as f32 * spacing);
            let mut events = [None; 8];
            events[0] = Some(AudioTransient {
                at: now,
                low: 0.0,
                mid: 0.5,
                high: 0.5,
            });
            model.ingest(&events, now, start);
            assert!(model.ripple_count() <= MAX_RIPPLE_PACKETS);
        }
    }
    assert!(slow.onset_density(start + Duration::from_secs(11)) < 0.001);
    assert!(fast.onset_density(start + Duration::from_millis(880)) > 0.8);

    // Find the packet's absolute envelope-weighted center before and after a
    // short interval; both carrier and envelope must flow right, never alternate.
    let mut model = WaveformReactivity::default();
    let mut events = [None; 8];
    events[0] = Some(AudioTransient {
        at: start,
        low: 0.0,
        mid: 0.8,
        high: 0.6,
    });
    model.ingest(&events, start, start);
    let center = |time| {
        let points: [f32; 1001] = model.samples(time);
        let mass = points.iter().map(|v| v.abs()).sum::<f32>();
        points
            .iter()
            .enumerate()
            .map(|(i, v)| i as f32 / 1000.0 * v.abs())
            .sum::<f32>()
            / mass
    };
    assert!(
        center(start + Duration::from_millis(110))
            > center(start + Duration::from_millis(25)) + 0.01
    );
}
