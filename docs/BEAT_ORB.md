# Beat-reactive playback orb

The existing WASAPI worker now measures low-band (20–180 Hz) onset and mid/high
(180–6,000 Hz) spectral flux independently. These approximate kick-like and
snare-like attacks; they are not an instrument classifier. RMS affects secondary
brightness only. There is no beat estimation from metadata or timeline position.

Each band uses a 96-hop rolling baseline, deviation and peak scale, an absolute
noise floor and soft saturation. Band scores combine with 0.55 low / 0.45 high
weights plus 0.45 of the stronger band, bounded to 0–1. A one-hop local maximum
selects attack strength rather than the tiny leading edge. The default duplicate
suppression is 85 ms. FFT and history buffers remain fixed-size on the existing
capture worker; only scalar features cross the bounded mailbox. Onsets publish
immediately; ordinary features retain the approximately 30 Hz publication limit.
No audio recording or export is added.

## Envelope and drawing

`BeatOrb` has no playback-position input. Its time-based smoothstep envelope uses
20 ms attack, 10 ms peak hold and 140 ms release by default. Onset strength uses
`intensity.powf(0.75)` with a small gate below the configurable low threshold.
Default thresholds 0.25 / 0.60 / 0.85 control negligible response, stronger flare,
and concentrated sparkle. Pausing or losing capture releases the current pulse;
seeking consumes and suppresses new beat events. Reduced motion disables capture
and physical pulsing. Unavailable/silent capture keeps the resting head, while
the underlying procedural waveform remains available. Random playback-head
sparkles and the old repeating head-breathing timer have been removed.

The final iced Canvas pass draws the core above the supporting waves. Requested
diameters are core 3–6 px, inner glow 7–11 px, halo 12–22 px. The same geometry-fit
function used by rendering limits the core to the wave's vertical clearance and
flattens halos into ellipses near the slot edge. The full pass is still scissored
to the existing 14/25 logical px rectangles. The compact halo width is capped at
13.5 px including its safety margin. Actual core/halo heights can be smaller on
wave lobes or with a smaller configured slot. Logical dimensions follow iced DPI
scaling; no physical-pixel constants or notch-size changes are introduced.

The head's x coordinate still comes exclusively from authoritative playback or
pointer seek progress. No media command, duration, seek interval, or timeline-rate
logic is changed. Brief orb animation uses the existing frame clock at 16 ms;
normal visible wave animation remains at 33 ms, and idle states stop frame updates
after release finishes. No new capture thread or per-frame audio buffers are added.

## Configuration

See `[music.waveform.beat_orb]` in `config/default.toml`. Older configurations
default this table automatically. Numeric values are normalized on load: core
1–6 px with ordered min/max, halo between the core maximum and 22 px, sensitivity
0–1, attack 15–25 ms, hold 0–20 ms, release 100–160 ms, cooldown 70–100 ms, and
ordered intensity thresholds. NaN/infinite floating values fall back to defaults.
`enabled = false` keeps the resting head and existing RMS/bass waveform modulation.

## Reproducible synthetic measurements

Tests generate four seconds of 48 kHz PCM in memory, never through an audio device.
The following are maximum detected event values and peak dimensions calculated
by the renderer's geometry-fit function in a centered 25 px slot. They are not
desktop screenshots or observed Apple Music responses. Core/halo return to
3/12 px after an isolated pulse's 170 ms envelope. Closely spaced beats may
overlap smoothly rather than returning fully to rest between every attack.

| Synthetic input | Triggers | Raw onset max | Normalized max | Peak core px | Peak halo width px |
|---|---:|---:|---:|---:|---:|
| Quiet kick, amplitude 0.045 | 3 | 0.00790 | 0.1368 | 3.016 | 12.054 |
| Slow kick, amplitude 0.30, 1 s spacing | 3 | 0.05266 | 0.6510 | 5.174 | 19.248 |
| Strong kick, amplitude 0.90 | 3 | 0.15797 | 1.0000 | 6.000 | 22.000 |
| Mid/high snare-like attack | 3 | 0.05349 | 0.6554 | 5.185 | 19.284 |
| Fast kicks, 125 ms spacing | 16 | 0.12592 | 0.9409 | 5.866 | 21.553 |
| Sustained 65 Hz bass | 1 at initial attack | 0.15164 | 0.9826 | 5.961 | 21.869 |
| Silence | 0 | 0 | 0 | 3.000 | 12.000 |
| Quiet gradual tonal passage | 0 | 0 | 0 | 3.000 | 12.000 |

An additional single-mix weak/medium/strong test asserts successively increasing
detected intensity. Geometry checks sweep 48 animation phases and 101 playback
positions per layout: maximum fitted core 6 px in both; maximum halo width
13.5 px Compact and 22 px Expanded. Every vertical radius stays inside the
allocated rectangle, and beat strength never alters pointer-to-time mapping.
Other tests cover pause/release/resume, suppressed seek-event replay, capture
unavailable, reduced motion, malformed settings, smaller visual slots, and
unchanged non-zero-start media timelines under audio updates.

Validation: formatting, compilation and strict Clippy passed; 19 automated tests
passed (2 unit/geometry, 5 audio/orb, 12 state transitions). Cargo emitted a
filesystem hard-link cache warning and copied cache files instead; checks still
completed successfully. No release build or real-device QA was run.

Commands: `cargo fmt --all --check`, `cargo check --locked --offline`,
`cargo clippy --locked --offline --all-targets -- -D warnings`, and
`cargo test --locked --offline --all-targets -- --nocapture`.

## Debug metrics and limits

Debug builds support `RUST_LOG=rimflux=info,rimflux::beat_orb=debug`. The renderer
logs the last raw/normalized detected onset, trigger state, envelope phase,
actual fitted core diameter, halo width/height, and capture status. Output is
limited to triggers, envelope-phase changes or every 250 ms, and is compiled out
of release rendering.
The metrics contain no captured PCM. Live diagnostics can show smaller heights
than the centered measurements above because the head follows the wave curve.

Real Apple Music songs, protected-content behavior, endpoint changes, perceived
latency, and desktop visual acceptance remain untested. WASAPI measures the system
output mix, which can include other applications; protected audio may be silent.
FFT window/hop, native buffering and redraw add latency, so this is a lightweight
onset effect rather than a beat tracker or sample-accurate drum classifier.

## Files changed for this enhancement

- `src/services/audio.rs`, `src/platform/windows_audio.rs`: band analysis,
  settings and onset publication through the existing capture path.
- `src/modules/beat_orb.rs`, `src/modules/mod.rs`: envelope and fitted dimensions.
- `src/modules/waveform.rs`, `src/modules/waveform_lighting.rs`: scalar input,
  resting fallback and removal of synthetic head pulses.
- `src/core/config.rs`, `config/default.toml`: compatible, clamped orb settings.
- `src/core/state.rs`, `src/app.rs`: capture settings, pulse lifecycle and clock.
- `src/ui/waveform.rs`: fitted orb drawing, debug diagnostics and geometry tests.
- `src/ui/layout.rs`: Clippy-only conditional cleanup in the existing info text.
- `tests/beat_orb.rs`, `tests/state_transitions.rs`: synthetic regressions and
  updated transition/animation expectations for the existing waveform UI.
- `docs/BEAT_ORB.md`, `docs/AUDIO_REACTIVE.md`, `README.md`: behavior and evidence.
