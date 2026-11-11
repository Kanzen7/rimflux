# Live audio-reactive waveform

Part 3.2 adds independent layer features and transient micro-ripples. See
[current mappings and live measurements](PART3_2.md) for the latest behavior.

The Windows provider uses shared-mode WASAPI loopback on the default render
endpoint. It analyzes the **system output mix**, not Apple Music exclusively.
There is no microphone input, export, recording file, private player API, or
protected-content workaround. Protected, silent, denied, or unsupported streams
may provide no usable signal; that does not establish beat synchronization.

## Capture lifecycle and bounds

`platform/windows_audio.rs` owns one event-driven capture stream on a dedicated
COM worker. The subscription exists only while non-demo music is playing, the
music waveform is visible, reduced motion is off, `live_audio` is enabled, and
the waveform mode is `reactive` or `hybrid`. Selecting another module, pausing,
opening the information panel, disabling the waveform, or shutting down drops
the subscription and cancels capture. Cancellation is checked at each wake, with
a 100 ms maximum event wait; no UI-thread join or blocking capture operation is
used. Replacement workers serialize access to the capture stream.

The provider reads native float32/float64 and PCM8/16/24/32 mix formats, including
PCM/float extensible formats. Each native packet is released on the capture
thread before analysis. The decoded packet buffer is bounded by the endpoint
buffer (at most 65,536 frames), cleared after processing, and never sent to UI.
FFT, window, spectrum, and threshold history are fixed-size allocations.

A watch channel retains only the latest scalar snapshot; the iced delivery
channel is bounded to two messages. Continuous features publish about 30 Hz;
new onset events publish immediately through the same bounded mailbox.
The default endpoint is rechecked once per second. Device replacement,
invalidation, or service errors release native resources and retry with a
cancellable five-second backoff. Permission and system settings are never
changed automatically.

## Independent audio features

`services/audio.rs` computes:

- **RMS:** energy across input channels, normalized through a bounded dB mapping.
- **Bass:** 20–180 Hz energy from a 2,048-point Hann-windowed FFT, independently
  of the RMS envelope.
- **Orb-specific onset:** positive spectral changes measured independently in
  20–180 Hz and 180–6,000 Hz bands. Rolling mean/deviation/peak normalization,
  a small absolute floor, local-peak picking, and a 24-hop warm-up suppress
  minor/noisy/repeated triggers. Default refractory time is now 85 ms, configurable
  from 70–100 ms. Soft saturation retains hit-strength differences.
- **Layer features:** independent 20–180 Hz bass, 180–1,000 Hz mid,
  1–5 kHz presence and 5–10 kHz high energy, plus low/mid/high spectral changes.
  Fine-line transients have a separate 45 ms guard and bounded eight-event history.
  These features reuse the same FFT and do not change the established orb detector.

Analysis uses a 512-sample hop. RMS and bass use a 35 ms attack / 320 ms release;
UI interpolation is separate. Packet discontinuities reset the spectral history
to avoid treating a device glitch as a musical onset. A silent endpoint decays
the envelopes and never synthesizes an onset. No full-track measured waveform
is implied by these local features.

## Visual binding

`AudioReactiveInput` receives only scalar snapshots, rejects stale data, and
discards pre-track/pre-pause onset timestamps. Bass, mid, percussion, high-detail
and ambient responses now have separate envelopes and amplitude/phase/opacity
mappings. Transient packets add localized, rapidly decaying fine-wave motion;
event spacing determines ripple density. Main ribbon thickness/glow follows bass.
The existing logical 14/25 px scissor encloses all geometry and lighting.

A measured onset drives a separate beat-orb envelope: default 20 ms attack,
10 ms hold, 140 ms release. The core grows from 3 to at most 6 logical px;
the halo grows from 12 to at most 22 px, fitted to the slot and local clearance.
White core, cyan/violet halo, nearby highlights, and the curved streak respond
locally. This never changes timeline position, duration, playback rate, seek
targets, or GSMTC commands. See [beat-orb measurements](BEAT_ORB.md).

Scheduled procedural head sparkles have been removed entirely. The orb's size
and beat-like bursts come only from measured onsets; seeking suppresses these
events and keeps the existing pointer lighting.
Existing decorative traveling lights remain separate from beat detection and
do not claim rhythmic accuracy. With unavailable/stale capture, the original
procedural waveform motion remains available with a resting playback head.
Reduced motion stops capture and orb animation entirely.

## Configuration and status

```toml
[music.waveform]
mode = "hybrid"
live_audio = true
```

Use `mode = "procedural"` or `live_audio = false` to disable loopback. The existing
More/info control reports live system-audio analysis, an unavailable reason, or
an idle procedural state. A connected silent stream produces no detected beats.

## Execution status

Synthetic PCM, renderer geometry and an eight-second live system-output probe
are recorded in [PART3_2.md](PART3_2.md). No desktop automation, playback-control
actions, screenshots or controlled genre comparison were performed. Actual visual
acceptance, device changes, protected playback and performance remain user QA;
live feature ranges do not establish Apple Music-exclusive audio or visual acceptance.

API references: [Microsoft loopback recording](https://learn.microsoft.com/en-us/windows/win32/coreaudio/loopback-recording)
and [capture packet lifetime](https://learn.microsoft.com/en-us/windows/win32/api/audioclient/nf-audioclient-iaudiocaptureclient-getbuffer).
