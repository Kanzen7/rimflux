use std::path::PathBuf;

use serde::Deserialize;

use super::{
    desktop::{General, InteractionSettings, Navigation, Shortcut, VisibilitySettings},
    errors::ConfigError,
    state::NotchMode,
};

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub appearance: Appearance,
    pub notch: Notch,
    pub music: Music,
    pub monitoring: Monitoring,
    pub development: Development,
    pub navigation: Navigation,
    pub visibility: VisibilitySettings,
    pub interaction: InteractionSettings,
    pub general: General,
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
    pub auto_switch: bool,
    pub idle_delay_secs: u64,
    pub source_app_id: Option<String>,
    pub waveform: WaveformConfig,
}

impl Default for Music {
    fn default() -> Self {
        Self {
            enable_waveform: true,
            waveform_style: "layered".into(),
            auto_switch: true,
            idle_delay_secs: 8,
            source_app_id: None,
            waveform: WaveformConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WaveformMode {
    Procedural,
    Reactive,
    Hybrid,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct WaveformConfig {
    pub enabled: bool,
    pub style: String,
    pub mode: WaveformMode,
    pub compact_max_height: f32,
    pub expanded_max_height: f32,
    pub animation_speed: f32,
    pub intensity: f32,
    pub glow: f32,
    pub interactive_seek: bool,
    pub live_audio: bool,
    pub beat_orb: BeatOrbConfig,
    pub reactivity: ReactivityConfig,
}

impl Default for WaveformConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            style: "layered".into(),
            mode: WaveformMode::Hybrid,
            compact_max_height: 14.0,
            expanded_max_height: 25.0,
            animation_speed: 1.0,
            intensity: 0.55,
            glow: 0.20,
            interactive_seek: true,
            live_audio: true,
            beat_orb: BeatOrbConfig::default(),
            reactivity: ReactivityConfig::default(),
        }
    }
}

impl WaveformConfig {
    fn normalize(&mut self) {
        fn bounded(value: f32, low: f32, high: f32, default: f32) -> f32 {
            if value.is_finite() {
                value.clamp(low, high)
            } else {
                default
            }
        }
        self.compact_max_height = bounded(self.compact_max_height, 1.0, 14.0, 14.0);
        self.expanded_max_height = bounded(self.expanded_max_height, 1.0, 25.0, 25.0);
        self.animation_speed = bounded(self.animation_speed, 0.1, 3.0, 1.0);
        self.intensity = bounded(self.intensity, 0.0, 1.0, 0.55);
        self.glow = bounded(self.glow, 0.0, 0.35, 0.20);
        self.beat_orb.normalize();
        self.reactivity.normalize();
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ReactivityConfig {
    pub enabled: bool,
    pub bass_sensitivity: f32,
    pub mid_sensitivity: f32,
    pub high_sensitivity: f32,
    pub transient_sensitivity: f32,
    pub micro_ripples: bool,
    pub adaptive_normalization: bool,
}
impl Default for ReactivityConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            bass_sensitivity: 0.65,
            mid_sensitivity: 0.70,
            high_sensitivity: 0.80,
            transient_sensitivity: 0.75,
            micro_ripples: true,
            adaptive_normalization: true,
        }
    }
}
impl ReactivityConfig {
    fn normalize(&mut self) {
        for (value, fallback) in [
            (&mut self.bass_sensitivity, 0.65),
            (&mut self.mid_sensitivity, 0.70),
            (&mut self.high_sensitivity, 0.80),
            (&mut self.transient_sensitivity, 0.75),
        ] {
            *value = if value.is_finite() {
                value.clamp(0.0, 1.0)
            } else {
                fallback
            };
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct BeatOrbConfig {
    pub enabled: bool,
    pub sensitivity: f32,
    pub minimum_core_diameter: f32,
    pub maximum_core_diameter: f32,
    pub minimum_halo_diameter: f32,
    pub maximum_halo_diameter: f32,
    pub attack_ms: u64,
    pub peak_hold_ms: u64,
    pub release_ms: u64,
    pub cooldown_ms: u64,
    pub low_threshold: f32,
    pub strong_threshold: f32,
    pub very_strong_threshold: f32,
}
impl Default for BeatOrbConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            sensitivity: 0.65,
            minimum_core_diameter: 3.0,
            maximum_core_diameter: 6.0,
            minimum_halo_diameter: 12.0,
            maximum_halo_diameter: 22.0,
            attack_ms: 20,
            peak_hold_ms: 10,
            release_ms: 140,
            cooldown_ms: 85,
            low_threshold: 0.25,
            strong_threshold: 0.60,
            very_strong_threshold: 0.85,
        }
    }
}
impl BeatOrbConfig {
    fn normalize(&mut self) {
        fn bound(value: f32, low: f32, high: f32, fallback: f32) -> f32 {
            if value.is_finite() {
                value.clamp(low, high)
            } else {
                fallback
            }
        }
        self.sensitivity = bound(self.sensitivity, 0.0, 1.0, 0.65);
        self.minimum_core_diameter = bound(self.minimum_core_diameter, 1.0, 6.0, 3.0);
        self.maximum_core_diameter = bound(
            self.maximum_core_diameter,
            self.minimum_core_diameter,
            6.0,
            6.0,
        );
        self.minimum_halo_diameter = bound(
            self.minimum_halo_diameter,
            self.maximum_core_diameter,
            22.0,
            12.0,
        );
        self.maximum_halo_diameter = bound(
            self.maximum_halo_diameter,
            self.minimum_halo_diameter,
            22.0,
            22.0,
        );
        self.attack_ms = self.attack_ms.clamp(15, 25);
        self.peak_hold_ms = self.peak_hold_ms.min(20);
        self.release_ms = self.release_ms.clamp(100, 160);
        self.cooldown_ms = self.cooldown_ms.clamp(70, 100);
        self.low_threshold = bound(self.low_threshold, 0.05, 0.45, 0.25);
        self.strong_threshold = bound(self.strong_threshold, self.low_threshold + 0.05, 0.8, 0.60);
        self.very_strong_threshold = bound(
            self.very_strong_threshold,
            self.strong_threshold + 0.05,
            0.98,
            0.85,
        );
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

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Development {
    pub demo_mode: bool,
}

impl Config {
    pub fn parse(source: &str) -> Result<Self, ConfigError> {
        let mut config: Self = toml::from_str(source)?;
        config.music.waveform.normalize();
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        if Shortcut::parse(&self.interaction.global_shortcut).is_none() {
            return Err(ConfigError::Validation(
                "global_shortcut needs Ctrl or Alt and a letter, digit, or F1–F11",
            ));
        }
        if !(50..=2000).contains(&self.visibility.hide_debounce_ms)
            || !(50..=2000).contains(&self.visibility.show_debounce_ms)
            || !(3..=60).contains(&self.visibility.peek_seconds)
        {
            return Err(ConfigError::Validation(
                "visibility delays must be 50–2000 ms and peek_seconds must be 3–60",
            ));
        }
        if self.music.idle_delay_secs > 3600
            || self
                .music
                .source_app_id
                .as_ref()
                .is_some_and(|id| id.trim().is_empty())
        {
            return Err(ConfigError::Validation(
                "idle_delay_secs must be <=3600 and source_app_id must not be empty",
            ));
        }
        if self.appearance.theme != "dark" || self.appearance.accent != "dynamic" {
            return Err(ConfigError::Validation(
                "Part 1 supports theme='dark', accent='dynamic'",
            ));
        }
        if self.music.waveform_style != "layered" || self.music.waveform.style != "layered" {
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

    pub fn save_desktop_to(&self, path: &std::path::Path) -> Result<(), ConfigError> {
        use std::{
            fs,
            io::Write,
            sync::atomic::{AtomicU64, Ordering},
        };
        static SERIAL: AtomicU64 = AtomicU64::new(0);
        self.validate()?;
        let original = match fs::read_to_string(path) {
            Ok(text) => Some(text),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => return Err(error.into()),
        };
        let mut table = original
            .as_deref()
            .map(str::parse::<toml::Table>)
            .transpose()?
            .unwrap_or_default();
        table.insert(
            "navigation".into(),
            toml::Value::try_from(&self.navigation)?,
        );
        table.insert(
            "visibility".into(),
            toml::Value::try_from(&self.visibility)?,
        );
        table.insert(
            "interaction".into(),
            toml::Value::try_from(&self.interaction)?,
        );
        table.insert("general".into(), toml::Value::try_from(&self.general)?);
        let appearance = table
            .entry("appearance")
            .or_insert_with(|| toml::Value::Table(toml::Table::new()));
        appearance
            .as_table_mut()
            .ok_or(ConfigError::Validation("appearance must be a TOML table"))?
            .insert(
                "reduced_motion".into(),
                self.appearance.reduced_motion.into(),
            );
        let text = toml::to_string_pretty(&table)?;
        Self::parse(&text)?;

        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(std::path::Path::new("."));
        fs::create_dir_all(parent)?;
        let staging = parent.join(format!(
            ".rimflux-{}-{}.tmp",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        let mut file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&staging)?;
        let result = (|| -> std::io::Result<()> {
            file.write_all(text.as_bytes())?;
            file.sync_all()?;
            drop(file);
            let current = match fs::read_to_string(path) {
                Ok(text) => Some(text),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                Err(error) => return Err(error),
            };
            if current != original {
                return Err(std::io::Error::other(
                    "configuration changed while saving; retry",
                ));
            }
            fs::rename(&staging, path)
        })();
        if result.is_err() {
            let _ = fs::remove_file(staging);
        }
        result.map_err(Into::into)
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
