use rimflux::{core::config::Config, platform};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "rimflux=info,wgpu_hal=warn".into()),
        )
        .init();
    let mut preview = false;
    for argument in std::env::args().skip(1) {
        match argument.as_str() {
            "--preview" => preview = true,
            "--help" | "-h" => {
                println!(
                    "Rimflux\n  --preview  Decorated, portable inspection window\n\nFocus the notch, then: 1 clock, 2/3 music, 4/5 monitoring.\nSpace play/pause; Enter expand; Esc collapse; Ctrl+Q quit.\nRIMFLUX_CONFIG overrides the user TOML path. RUST_LOG controls diagnostics."
                );
                return Ok(());
            }
            _ => return Err(format!("Unknown argument {argument:?}; use --help").into()),
        }
    }
    platform::run(Config::load(), preview)
}
