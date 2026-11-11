# Part 3.2 — Independent audio layers and micro-ripples

The subsequent [traveling-wave refinement](TRAVELING_WAVES.md) updates thin-line
motion and limits visible packets to four. The measurements below record Part 3.2.

## Audit and scope

The previous worker produced RMS, bass and two-band onset data. Rendering reused
one RMS multiplier for every fine line and one RMS/bass response for both ribbons.
An eight-second read-only WASAPI audit at 48 kHz observed RMS 0.624–0.854 and bass
0.391–0.628, confirming live data while identifying the shared renderer mapping.

This change extends the same FFT and capture worker. It preserves the established
orb detector's 20–180 / 180–6,000 Hz split and its configurable 85 ms cooldown.
No notch dimensions, controls, artwork, media commands or seek behavior change.
There is no additional GUI framework or live capture thread in normal operation.

## Features and timing

`AudioFeatures` exposes normalized overall, bass, mid, presence and high energy,
plus independent low/mid/high positive spectral-change scores. All are finite
and bounded to 0–1. The existing 2,048-point Hann FFT with a 512-sample hop is
reused; there is no second transform and no PCM delivery to the UI.

| Feature region | Frequency | Purpose |
|---|---|---|
| Bass | 20–180 Hz | Main ribbon; keep continuity with the working bass detector |
| Mid | 180–1,000 Hz | Melodic/mid-wave movement |
| Presence | 1–5 kHz | Percussion and mid-band spectral changes |
| High | 5–10 kHz, limited by Nyquist | Fine detail and high-band transients |

Energy normalization combines an eight-second decaying peak reference with a
small absolute audibility floor. This lets quieter sections contract rather
than boosting every small sound to full amplitude. Each region has an independent
rolling flux baseline; soft saturation preserves differences in hit strength.
Frequency regions approximate spectral content, not specific instruments.

Fine-wave transients have a separate 45 ms duplicate guard and one-hop local-peak
selection, so they can respond faster than the orb. Eight recent scalar events
are retained in a fixed array and sent through the existing latest-value watch
channel and bounded iced channel. New events publish immediately; ordinary
features publish at roughly 30 Hz. Reset, silence, pause and stale-capture paths
clear or expire events so old packets cannot retrigger the visualization.

| Layer | Attack / release | Response |
|---|---|---|
| Bass ribbon | 28 / 180 ms; low onset 22 / 150 ms | Broad amplitude, thickness, subtle glow |
| Mid wave | 35 / 150 ms | Medium amplitude, phase and flowing curvature |
| Percussion line | 15 / 90 ms; onset 12 / 90 ms | Local transient packets, visibility, fine curvature |
| High detail | 18 / 100 ms; onset 12 / 80 ms | Faster fine motion and high-only shimmer |
| Ambient | 140 / 400 ms | Slow, longer-wave, low-opacity continuity |

The seven existing Expanded layers map to bass, mid ribbon, percussion, mid line,
two high-detail lines and ambient. Compact retains three: bass, fast percussion
detail and ambient. Each layer has independent amplitude/phase/opacity mapping.
High-detail shimmer is not driven by mid-only onsets.

## Local ripple geometry

Each measured event creates one bounded wave packet with a 15 ms attack,
approximately 105 ms decay constant and 360 ms maximum lifetime. Its origin and
direction follow event order; they are not randomized every frame. The packet
travels a short distance through the fine lines. A smooth local envelope confines
its displacement to a small part of the curve rather than moving the whole line.
Recent onset spacing controls spatial density (18–38 cycles over the timeline),
not RMS volume. Quiet passages do not spawn substitute events.

The ripple field is sampled once into the existing 96-point normalized grid and
reused across fine layers. Envelope evaluation is shared; there are no per-frame
audio allocations or CPU blur. Displacement is capped at 0.24 of the available
half-height. The original 14/25 logical px clipping rectangles enclose paths,
strokes, fills, glow, ripples and orb. Horizontal coordinates, played/remaining
clips, edge fading and the head's authoritative playback/seek x coordinate are
unchanged. Both progress regions draw the same prepared geometry.

Visible ripple/orb motion uses the existing 16 ms frame subscription while
needed; ordinary visible playback retains 33 ms. Pause clears ripple packets and
finishes the short scalar fade before frame updates stop. Hidden states do not
render; reduced motion stops capture and motion. Unavailable capture fades to
the procedural waveform with a resting orb, without simulated beat events.

## Configuration and diagnostics

`config/default.toml` includes backward-compatible, defaulted settings:

```toml
[music.waveform.reactivity]
enabled = true
bass_sensitivity = 0.65
mid_sensitivity = 0.70
high_sensitivity = 0.80
transient_sensitivity = 0.75
micro_ripples = true
adaptive_normalization = true
```

Sensitivities clamp to 0–1; nonfinite values use defaults. Disabling reactivity
restores procedural layer parameters while retaining the independently configured
beat orb. Disabling micro-ripples leaves energy-driven layer motion available.
Disabling adaptive normalization uses fixed feature scales; the established orb
detector retains its previous adaptive behavior. No settings panel was added.

Debug builds expose final renderer diagnostics with
`RUST_LOG=rimflux=info,rimflux::waveform_bands=debug`. Every 250 ms the renderer
reports smoothed band energies, three onset scores, ripple activity/count, the
final per-layer amplitude multipliers, blend and capture status. Orb diagnostics
remain under `rimflux::beat_orb`. These logging paths are absent in release builds.

`RUST_LOG=rimflux=info cargo run -- --audio-probe` (set the environment variable
using the host shell) runs an eight-second Windows debug-only read-only probe.
It reuses the normal capture worker and layer mapping, reports scalar ranges,
then closes the stream. It does not start playback, select songs, open a notch,
or retain captured PCM. A probe is explicit diagnostics and does not use the
normal visible-music capture gate. Normal operation still uses that gate.

## Measured results

An eight-second live system-output probe on 2026-11-08, at 48 kHz, received 220
distinct live snapshots. Values below are observed scalar ranges, not instrument
classification or desktop visual acceptance. Layer amplitude ranges use the same
response function as rendering, excluding the first second of filter warm-up.

| Feature | Minimum | Maximum |
|---|---:|---:|
| Overall energy | 0.204 | 0.894 |
| Bass energy | 0.001 | 0.894 |
| Mid energy | 0.110 | 0.894 |
| Presence energy | 0.080 | 0.894 |
| High energy | 0.002 | 0.894 |
| Low onset | 0 | 0.219 |
| Mid/presence onset | 0 | 0.333 |
| High onset | 0 | 0.204 |

| Layer | Observed amplitude multiplier |
|---|---:|
| Bass | 0.589–1.036 |
| Mid | 0.581–1.121 |
| Percussion | 0.334–0.947 |
| High detail | 0.249–0.912 |
| Ambient | 0.628–0.916 |

The probe ingested 42 transient events; peak ripple activity was 0.588. These
counts describe detected spectral changes, not 42 classified drum hits. The
eight-event packet capacity and short release keep overlapping responses bounded.

Synthetic checks separately measured:

- 65 / 500 / 3,000 / 8,000 Hz tones primarily activated their intended bands,
  with normalized target energy approximately 0.894 and other-band leakage below
  0.02. Silence produced no transient events.
- Sixteen treble attacks separated by 62.5 ms produced sixteen fine-wave events;
  sustained bass produced at most its initial attack.
- Isolated energy changes modified the intended amplitude multiplier while
  unrelated energy-layer multipliers remained constant.
- Final prepared Expanded geometry changed vertically by up to 4.309 px for bass,
  5.125 px for mid and 2.412 px for high detail between quiet and active synthetic
  inputs. Horizontal sample positions stayed identical. These are geometry
  calculations, not observed screenshots.
- Maximum-feature/ripple geometry stayed within Compact/Expanded visual bounds
  across 48 phases. Existing orb fitting still capped its core at 6 px and halo
  width at 13.5 px Compact / 22 px Expanded.
- Pause removed packets and stopped redraws after fading; reduced motion disabled
  capture; seek/timeline and working orb regression tests remained green.

Validation: formatting, compilation, strict Clippy and 26 tests pass. Cargo's
filesystem hard-link cache warning falls back to copying and does not fail checks.
No release, Linux, FPS/CPU benchmark or desktop visual QA is claimed.

The live excerpt was the system mix already playing; no application was controlled
and no genres or songs were selected. A controlled comparison of quiet acoustic,
bass-heavy, vocal, fast-percussion and chorus excerpts remains user acceptance.
Protected audio can still be unavailable, and other applications can contribute
to the mix. FFT/window, buffering and rendering introduce normal analysis latency.

## Modified files

`src/services/audio.rs`, `src/platform/windows_audio.rs`, `src/main.rs`,
`src/modules/waveform_reactivity.rs`, `src/modules/waveform.rs`,
`src/modules/mod.rs`, `src/core/config.rs`, `src/core/state.rs`, `src/app.rs`,
`src/ui/waveform.rs`, `config/default.toml`, `tests/multiband.rs`,
`docs/PART3_2.md`, `docs/AUDIO_REACTIVE.md`, `README.md`.
