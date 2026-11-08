#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("cannot read configuration: {0}")]
    Read(#[from] std::io::Error),
    #[error("invalid TOML: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("invalid configuration: {0}")]
    Validation(&'static str),
}
