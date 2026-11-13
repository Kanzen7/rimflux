#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("configuration I/O: {0}")]
    Read(#[from] std::io::Error),
    #[error("invalid TOML: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("cannot serialize configuration: {0}")]
    Serialize(#[from] toml::ser::Error),
    #[error("invalid configuration: {0}")]
    Validation(&'static str),
}
