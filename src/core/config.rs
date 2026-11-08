use std::path::PathBuf;

use serde::Deserialize;

use super::{errors::ConfigError, state::NotchMode};

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub appearance: Appearance,
    pub notch: Notch,
    pub music: Music,
    pub monitoring: Monitoring,
    pub development: Development,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Appearance {
    pub theme: String,
    pub accent: String,
    pub reduced_motion: bool,
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            theme: "dark".into(),
            accent: "dynamic".into(),
            reduced_motion: false,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Notch {
    pub default_mode: NotchMode,
    pub auto_collapse: bool,
}

impl Default for Notch {
    fn default() -> Self {
        Self {
            default_mode: NotchMode::Clock,
            auto_collapse: true,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Music {
    pub enable_waveform: bool,
    pub waveform_style: String,
}

impl Default for Music {
    fn default() -> Self {
        Self {
            enable_waveform: true,
            waveform_style: "layered".into(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Monitoring {
    pub refresh_interval_ms: u64,
}

impl Default for Monitoring {
    fn default() -> Self {
        Self {
            refresh_interval_ms: 1000,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Development {
    pub demo_mode: bool,
}

impl Default for Development {
    fn default() -> Self {
        Self { demo_mode: true }
    }
}

impl Config {
    pub fn parse(source: &str) -> Result<Self, ConfigError> {
        let config: Self = toml::from_str(source)?;
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.appearance.theme != "dark" || self.appearance.accent != "dynamic" {
            return Err(ConfigError::Validation(
                "Part 1 supports theme='dark', accent='dynamic'",
            ));
        }
        if self.music.waveform_style != "layered" {
            return Err(ConfigError::Validation("waveform_style must be 'layered'"));
        }
        if !(500..=60_000).contains(&self.monitoring.refresh_interval_ms) {
            return Err(ConfigError::Validation(
                "refresh_interval_ms must be 500..=60000",
            ));
        }
        Ok(())
    }

    pub fn user_path() -> Option<PathBuf> {
        if let Some(path) = std::env::var_os("RIMFLUX_CONFIG") {
            return Some(path.into());
        }
        #[cfg(target_os = "windows")]
        return std::env::var_os("APPDATA")
            .map(|base| PathBuf::from(base).join("Rimflux/config.toml"));
        #[cfg(not(target_os = "windows"))]
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|base| PathBuf::from(base).join(".config")))
            .map(|base| base.join("rimflux/config.toml"))
    }

    pub fn load() -> Self {
        let Some(path) = Self::user_path() else {
            return Self::default();
        };
        match std::fs::read_to_string(&path) {
            Ok(source) => match Self::parse(&source) {
                Ok(config) => {
                    tracing::info!(?path, "Loaded user configuration");
                    config
                }
                Err(error) => {
                    tracing::warn!(%error, ?path, "Using built-in defaults");
                    Self::default()
                }
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Self::default(),
            Err(error) => {
                tracing::warn!(%error, ?path, "Using built-in defaults");
                Self::default()
            }
        }
    }
}
