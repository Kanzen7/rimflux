# Rimflux

A native desktop notch foundation written entirely in Rust with iced 0.14.

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
| `Space` | Play / pause mock media |
| `Escape` | Collapse |
| `Ctrl+Q` | Quit |

The `1`–`5` shortcuts require demo mode, which is enabled by default. Click music controls directly to use them. The timeline supports click-to-seek. Expanded views automatically collapse after eight seconds without interaction while the pointer is outside the notch.

On Windows, set `RIMFLUX_INSPECT=1` before launching to retain a taskbar entry for development inspection. Normal native mode hides the taskbar entry.

## Configuration

Built-in defaults are documented in [`config/default.toml`](config/default.toml). User configuration is read from:

- Windows: `%APPDATA%\Rimflux\config.toml`
- Linux: `$XDG_CONFIG_HOME/rimflux/config.toml`, or `~/.config/rimflux/config.toml`
- Either platform: the file specified by `RIMFLUX_CONFIG`

Missing optional fields use defaults. Missing or invalid files fall back to built-in defaults; invalid values produce a diagnostic. User files are never created or overwritten automatically. `RUST_LOG` controls logging.

Part 1 supports the dark theme, dynamic accent palette, and layered waveform style. `appearance.reduced_motion` disables waveform motion and snaps layout transitions. Disabling demo mode leaves media unavailable until an OS media provider is implemented.

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
