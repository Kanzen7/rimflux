# Rimflux

A native desktop notch foundation written entirely in Rust with iced 0.14.

Part 3 adds a procedural layered waveform as the single playback timeline, with
played/remaining shading and capability-aware drag seeking. Compact and expanded
visuals are capped at 14 and 25 logical pixels. See [Part 3 implementation notes](docs/PART3.md)
for configuration, timing, rendering, and the current audio-reactive limitations.
Part 3 validation and desktop QA are left to the user as requested.

Part 3.1 replaces the vertical marker with a curved luminous playback head,
adds brief seek trails and sparse traveling flares, and fades waveform ends
through alpha. See [Part 3.1 lighting notes](docs/PART3_1.md).

Windows reactive/hybrid modes now support optional live system-output loopback
features: RMS, bass energy, and adaptive spectral-flux onsets. See
[audio-reactive implementation notes](docs/AUDIO_REACTIVE.md) for lifecycle,
bounded analysis, fallback behavior, configuration, and verification status.

The beat-reactive orb adds separate low/mid-high onset analysis, strength-scaled
core/halo expansion, and configurable fast attack/release. Head pulses require
measured audio. See [beat-orb implementation and synthetic measurements](docs/BEAT_ORB.md)
for current behavior, changed files, diagnostics and validation. These notes
supersede older descriptions of scheduled playback-head sparkles.

Part 3.2 maps independent bass/mid/presence/high features to distinct waveform
layers and adds short-lived, onset-driven micro-ripples. See
[Part 3.2 implementation and live feature measurements](docs/PART3_2.md) for
configuration, diagnostics, validation and remaining visual acceptance.

Thin lines now have continuous rightward phase propagation with independent,
audio-responsive velocities. See [traveling-wave refinement](docs/TRAVELING_WAVES.md)
for its synthetic animation preview, measured speeds, and validation.

Part 4 adds mode navigation, smart visibility, and Windows tray recovery. See
[Part 4 usage and implementation](docs/PART4.md) for controls and platform coverage.

Minimal when idle. Useful when needed. Invisible when unnecessary.

## Part 1 foundation

- Local clock and date, compact and expanded music, and compact and expanded system monitoring.
- Procedural Canvas silhouette and layered animated waveform, with reduced-motion support.
- Explicitly labeled mock media: playback, previous/next, seek, shuffle, repeat, and information controls.
- Real CPU, RAM, and aggregate network measurements through sysinfo, collected off the UI thread.
- Typed state transitions, validated TOML preferences, diagnostics, and headless tests.
- Windows top-center positioning, DPI conversion, transparency, and a shaped input region.
- Linux Wayland registry detection and an iced_layershell adapter with a bounded surface and input region.
- A decorated portable preview when requested, or when native Wayland support is unavailable.

This is a development foundation. Desktop integration and visual acceptance are still in progress.

## Run

Install Rust stable, then run from the repository root:

```sh
cargo run --locked
cargo run --locked -- --preview
```

On Linux, install the desktop development libraries first. For Debian/Ubuntu:

```sh
sudo apt-get install pkg-config libwayland-dev libxkbcommon-dev libx11-dev libxi-dev libxcursor-dev libxrandr-dev libvulkan-dev
```

Focus or click the notch before using keyboard shortcuts:

| Input | Action |
| --- | --- |
| `1` | Clock |
| `2` / `3` | Compact / expanded music |
| `4` / `5` | Compact / expanded monitoring |
| `Enter` or click the background | Expand / collapse |
| Right-click, `F10`, or expanded `⋮` | Open the mode and settings menu |
| `Space` | Play / pause the supported media session |
| `Escape` | Close the current panel, dismiss a focused peek, or collapse |
| `Ctrl+Alt+R` | Show, hide, or peek on Windows (configurable) |
| `Ctrl+Q` | Quit |

Outside the menu, `1`–`5` are inspection shortcuts available in debug, demo, or preview mode. The timeline supports click and drag seeking when the media session allows it. By default, expanded views collapse after eight seconds of inactivity with the pointer outside the notch.

On Windows, set `RIMFLUX_INSPECT=1` before launching to retain a taskbar entry for development inspection. Normal native mode hides the taskbar entry.

## Configuration

Built-in defaults are documented in [`config/default.toml`](config/default.toml). User configuration is read from:

- Windows: `%APPDATA%\Rimflux\config.toml`
- Linux: `$XDG_CONFIG_HOME/rimflux/config.toml`, or `~/.config/rimflux/config.toml`
- Either platform: the file specified by `RIMFLUX_CONFIG`

Missing optional fields use defaults. Missing or invalid files fall back to built-in defaults. Settings and explicit mode preferences are written to the same TOML path. `RUST_LOG` controls logging.

Rimflux supports the dark theme, dynamic accent palette, and layered waveform style. `appearance.reduced_motion` disables waveform motion and snaps layout transitions. The native Windows media provider is used by default.

## Source structure

```text
src/
  main.rs              CLI and logging
  lib.rs               Testable application modules
  app.rs               iced update, subscriptions, and background sampling
  message.rs           Typed application messages
  core/                Configuration, errors, and state
  modules/             Clock, music, and monitoring models
  services/            Mock media and native monitoring providers
  ui/                  Canvas layouts, silhouette, waveform, animation, palette
  platform/            Window lifecycle, Win32, Wayland, and preview adapters
config/default.toml    Built-in preference reference
tests/                 Headless integration tests
.github/workflows/     Windows/Linux validation workflow
```

Primary dependencies: iced, chrono, sysinfo, serde, toml, tracing, tracing-subscriber, tokio, and thiserror. Windows APIs and raw-window-handle are Windows-gated; iced_layershell and wayland-client are Linux-gated. Exact resolved versions are recorded in `Cargo.lock`.

## Validation

```sh
cargo fmt --all -- --check
cargo check --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
cargo build --locked
cargo build --locked --release
```

The initial host is Windows x86_64 MSVC with Rust stable 1.91.1. Windows compilation, formatting, strict Clippy, and headless tests have been exercised locally. The native clock launched and rendered. Music inspection exposed a waveform endpoint panic and Canvas clipping/layer-order problems; fixes are present, but full visual revalidation remains pending after inspection was stopped.

Linux cross-checking from Windows stopped in a dependency build script because a Linux pkg-config/sysroot was unavailable. Linux compilation and compositor behavior are unverified. The GitHub Actions workflow defines Windows and Linux checks; its presence does not establish a successful CI run.

## Remaining work / Part 2

- Complete visual checks for all five states, interaction controls, transitions, and the portable preview.
- Validate release builds and native behavior on Windows 10/11 and Linux compositors, including unsupported Wayland environments.
- Exercise multiple monitors, fractional scaling, transparent-corner click-through, and display changes.
- Add accessible control semantics and keyboard navigation beyond the development shortcuts; Canvas controls currently lack screen-reader semantics.
- Integrate Linux MPRIS and Windows media sessions, then real audio analysis. Music and waveform amplitude are procedural demo data today.
- Add monitor selection and robust hot-plug handling. No AppBar or exclusive desktop area is reserved.

Network readings aggregate interfaces and may include virtual adapters. Initial monitoring samples warm up before displaying values. No performance benchmark or production-readiness claim is made.

No project license has been selected.
