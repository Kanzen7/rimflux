# Part 3 — Waveform playback timeline

The music waveform is now the single timeline component. The separate horizontal
seekbar and the static decorative waveform are no longer rendered.

## Rendering and layout

- Centerlines are normalized samples from the supplied main ribbon, secondary
  ribbon, cyan wave, and prism wave SVGs. Original source files are preserved in
  `assets/references/waveform/`.
- Compact: three layers in a maximum 14 logical-pixel clip, within the existing
  345 × 42 notch. Expanded: five layers in a maximum 25 logical-pixel clip,
  within the existing 456 × 129 reference geometry.
- `ui/music_layout.rs` defines the shared visual rectangle and larger interaction
  rectangle. Metadata, durations, and controls occupy separate allocations.
- Ribbon fills, line strokes, a restrained wide-stroke highlight, and playback
  markers are all scissored to the visual rectangle. No blur or image waveform
  is generated at runtime.
- Played and remaining regions reuse the exact same paths and horizontal
  gradients, with separate clipping and opacity. Animation changes vertical
  displacement and thickness, independently of the playback boundary.

## Profile and motion

`modules/waveform.rs` derives a stable FNV-1a seed from source, title, artist,
and album. It caches a smooth deterministic envelope per track, retains the
profile across seeks, and interpolates it on track changes. The profile is
procedural artwork, **not measured whole-track audio amplitude**.

Visible playing music requests 30 FPS animation; notch expansion retains its
existing transition clock. Paused motion freezes, hidden music stops requesting
waveform frames, and reduced motion keeps the waveform static. Timeline progress
continues independently from animation.

## Playback and seeking

- GSMTC snapshots remain authoritative. Local progress uses monotonic time and
  the reported playback rate between updates, bounded by playback start/end.
- Unchanged timeline timestamps are anchored to the prior local snapshot instead
  of adding time spent paused to elapsed playback when resuming. New timeline
  stamps, track changes, and seek updates reconcile the displayed position.
- Both compact and expanded waveforms support hover and drag preview when the
  media session exposes Seek. Release sends one capability-checked command.
- The target is mapped through the actual start/end interval and clamped to the
  reported min/max seek range. Preview survives briefly while waiting for the
  authoritative position. Errors, track changes, lost focus, and unavailable
  sessions cancel or clear the preview.
- Unsupported seeking is read-only, with no grab cursor or simulated seek.

## Audio-reactive status

WASAPI endpoint loopback uses `IAudioClient` / `IAudioCaptureClient` and
`AUDCLNT_STREAMFLAGS_LOOPBACK`; it is a separate source from GSMTC metadata.
No loopback worker or audio recording is started by this implementation.
`AudioReactiveInput` accepts optional normalized RMS for a future permitted
capture provider and smooths it separately from the timeline.

`procedural` uses simulated movement. `reactive` and `hybrid` currently fall back
to the same procedural motion because no capture provider is attached. No
Apple-Music-only capture, measured audio energy, or DRM bypass is claimed.

## Configuration

See `[music.waveform]` in `config/default.toml`. Visual heights are clamped to
1–14 / 1–25 logical pixels; speed, intensity, and glow are bounded. Legacy
`music.enable_waveform` remains a master switch for older configurations.

## Verification status

At the user's request, no validation commands, automated tests, desktop
interaction, screenshots, or manual QA were run for Part 3. The application is
launched normally for the user to inspect. Compilation performed as part of
starting `cargo run` is distinct from desktop or media acceptance.

Live WASAPI modulation remains deferred. Visual appearance, seek support in the
current Apple Music session, and desktop acceptance remain user QA items.
