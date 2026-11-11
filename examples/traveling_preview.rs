//! Export actual prepared waveform geometry for a synthetic quiet/dense preview.
//! No desktop, audio capture, media session or GPU is used.
#[cfg(debug_assertions)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use rimflux::{
        core::{
            config::WaveformConfig,
            state::{NotchExpansion, NotchMode, dimensions},
        },
        modules::{music::MusicInfo, waveform::WaveformState},
        services::audio::{AudioCaptureStatus, AudioFeatures, AudioSnapshot, AudioTransient},
        ui::{music_layout::WaveformSlot, waveform::WaveformRenderer},
    };
    use std::{
        io::{BufWriter, Write},
        time::{Duration, Instant},
    };
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "target/traveling-preview.csv".into());
    let mut output = BufWriter::new(std::fs::File::create(path)?);
    writeln!(output, "frame,passage,layer,point,x,y,opacity,width")?;
    let config = WaveformConfig::default();
    let slot = WaveformSlot::new(
        NotchExpansion::Expanded,
        dimensions(NotchMode::Music, NotchExpansion::Expanded),
        &config,
    );
    for (passage, energy, interval) in [("quiet", 0.08, 1.5), ("dense", 0.85, 0.10)] {
        let mut state = WaveformState::new(&MusicInfo::demo());
        let start = Instant::now();
        let dt = Duration::from_secs_f64(1.0 / 30.0);
        let mut last_beat = -1_i32;
        for step in 0..180 {
            let time = step as f64 / 30.0;
            let now = start + Duration::from_secs_f64(time);
            let beat = (time / interval) as i32;
            let onset_age = time - beat as f64 * interval;
            let onset = if onset_age < 0.045 { energy } else { 0.0 };
            let mut snapshot = AudioSnapshot {
                status: AudioCaptureStatus::Live,
                captured_at: now,
                features: AudioFeatures {
                    overall_energy: energy,
                    bass_energy: energy * 0.6,
                    mid_energy: energy,
                    presence_energy: energy,
                    high_energy: energy * 0.8,
                    low_onset: onset * 0.5,
                    mid_onset: onset,
                    high_onset: onset,
                },
                ..AudioSnapshot::unavailable("synthetic preview")
            };
            if beat != last_beat {
                snapshot.transients[0] = Some(AudioTransient {
                    at: now,
                    low: onset * 0.5,
                    mid: onset,
                    high: onset,
                });
                last_beat = beat;
            }
            state.audio.receive(snapshot);
            state.advance(dt, true, &config, now);
            if step < 60 {
                continue;
            } // Let adaptive velocities/envelopes settle.
            for (index, layer) in WaveformRenderer::preview_layers(slot, &state, &config, now)
                .into_iter()
                .enumerate()
            {
                if ![0, 2, 3, 4].contains(&index) {
                    continue;
                }
                for (i, p) in layer.points.iter().enumerate() {
                    writeln!(
                        output,
                        "{},{},{},{},{:.6},{:.6},{:.6},{:.3}",
                        step - 60,
                        passage,
                        index,
                        i,
                        p.x - slot.visual.x,
                        p.y - slot.visual.y,
                        layer.opacity,
                        layer.width
                    )?;
                }
            }
        }
        let velocities = rimflux::modules::traveling_wave::TravelingLayer::ALL
            .map(|layer| state.traveling.parameters(layer).angular_velocity);
        println!("{passage}: final angular velocities (mid/micro/high) = {velocities:?}");
    }
    Ok(())
}

#[cfg(not(debug_assertions))]
fn main() {
    eprintln!("Use a debug build for headless waveform preview data.");
}
