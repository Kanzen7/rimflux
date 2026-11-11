use rimflux::{core::config::Config, platform};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "rimflux=info,wgpu_hal=warn".into()),
        )
        .init();
    let mut preview = false;
    let mut config = Config::load();
    for argument in std::env::args().skip(1) {
        match argument.as_str() {
            "--preview" => preview = true,
            "--demo" => config.development.demo_mode = true,
            #[cfg(all(target_os = "windows", debug_assertions))]
            "--audio-probe" => {
                rimflux::platform::windows_audio::probe(
                    &config.music.waveform,
                    std::time::Duration::from_secs(8),
                )?;
                return Ok(());
            }
            #[cfg(target_os = "windows")]
            "--media-probe" => {
                rimflux::platform::windows_media::probe(config.music.source_app_id.clone())?;
                return Ok(());
            }
            "--help" | "-h" => {
                println!(
                    "Rimflux\n  --preview      Decorated inspection window\n  --demo         Explicit mock playback (no native controls)\n  --media-probe  Read-only Windows session diagnostics\n  --audio-probe  Eight-second system-audio feature probe (Windows debug)\n\nFocus the notch, then: 1 clock, 2/3 music, 4/5 monitoring (debug/demo/preview).\nSpace play/pause; Enter expand; Esc collapse; Ctrl+Q quit.\nRIMFLUX_CONFIG overrides the user TOML path. RUST_LOG controls diagnostics."
                );
                return Ok(());
            }
            _ => return Err(format!("Unknown argument {argument:?}; use --help").into()),
        }
    }
    platform::run(config, preview)
}
