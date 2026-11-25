use crate::color::Rgb;
use crate::device::DevicePlacement;
use crate::hub_importer::ImportedHubItem;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Locale {
    #[default]
    #[serde(rename = "pt-BR")]
    PtBr,
    #[serde(rename = "en-US")]
    EnUs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectKind {
    Off,
    Solid,
    Gradient,
    RainbowWave,
    Breathing,
    DualPulse,
    Starlight,
    Ambilight,
    AudioSpectrum,
}

impl EffectKind {
    /// Static effects require zero continuous CPU ticks after being applied once.
    #[inline]
    pub const fn is_static(self) -> bool {
        matches!(self, Self::Off | Self::Solid | Self::Gradient)
    }

    pub const fn needs_screen_capture(self) -> bool {
        matches!(self, Self::Ambilight)
    }

    pub const fn needs_audio_capture(self) -> bool {
        matches!(self, Self::AudioSpectrum)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EffectParams {
    pub kind: EffectKind,
    pub primary_color: Rgb,
    pub secondary_color: Rgb,
    /// Animation speed `1..=100`.
    pub speed: u8,
    /// Wave / gradient angle in degrees `0..=360`.
    pub angle_deg: f32,
    /// Spatial scale `10..=200` (percent).
    pub scale: u8,
}

impl Default for EffectParams {
    fn default() -> Self {
        Self {
            kind: EffectKind::RainbowWave,
            primary_color: Rgb::new(56, 189, 248),
            secondary_color: Rgb::new(16, 185, 129),
            speed: 45,
            angle_deg: 25.0,
            scale: 100,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LightingProfile {
    pub id: String,
    pub name: String,
    pub brightness: u8,
    pub effect: EffectParams,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppProfileRule {
    pub id: String,
    pub executable: String,
    pub profile_id: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutomationConfig {
    pub auto_off_on_lock: bool,
    pub idle_timeout_mins: u16,
    pub night_mode_enabled: bool,
    pub night_start: String,
    pub night_end: String,
    pub night_brightness: u8,
    pub low_battery_warn_pct: u8,
    pub low_battery_crit_pct: u8,
    pub start_with_windows: bool,
    pub app_rules: Vec<AppProfileRule>,
}

impl Default for AutomationConfig {
    fn default() -> Self {
        Self {
            auto_off_on_lock: true,
            idle_timeout_mins: 15,
            night_mode_enabled: true,
            night_start: "23:00".to_string(),
            night_end: "07:00".to_string(),
            night_brightness: 25,
            low_battery_warn_pct: 20,
            low_battery_crit_pct: 10,
            start_with_windows: false,
            app_rules: vec![
                AppProfileRule {
                    id: "rule-feathertune".to_string(),
                    executable: "feathertune.exe".to_string(),
                    profile_id: "ambilight".to_string(),
                    enabled: true,
                },
                AppProfileRule {
                    id: "rule-vlc".to_string(),
                    executable: "vlc.exe".to_string(),
                    profile_id: "ambilight".to_string(),
                    enabled: true,
                },
                AppProfileRule {
                    id: "rule-cs2".to_string(),
                    executable: "cs2.exe".to_string(),
                    profile_id: "gaming".to_string(),
                    enabled: true,
                },
            ],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppConfig {
    pub schema_version: u32,
    pub locale: Locale,
    pub power_on: bool,
    pub global_brightness: u8,
    pub active_profile_id: String,
    pub aura_argb_led_counts: [usize; 2],
    pub profiles: Vec<LightingProfile>,
    pub placements: Vec<DevicePlacement>,
    pub automation: AutomationConfig,
    #[serde(default)]
    pub imported_hub_presets: Vec<ImportedHubItem>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            schema_version: 1,
            locale: Locale::PtBr,
            power_on: true,
            global_brightness: 80,
            active_profile_id: "work".to_string(),
            aura_argb_led_counts: [16, 16],
            profiles: vec![
                LightingProfile {
                    id: "work".to_string(),
                    name: "Work".to_string(),
                    brightness: 70,
                    effect: EffectParams {
                        kind: EffectKind::Gradient,
                        primary_color: Rgb::new(224, 242, 254),
                        secondary_color: Rgb::new(56, 189, 248),
                        speed: 30,
                        angle_deg: 20.0,
                        scale: 100,
                    },
                },
                LightingProfile {
                    id: "gaming".to_string(),
                    name: "Gaming".to_string(),
                    brightness: 90,
                    effect: EffectParams {
                        kind: EffectKind::Gradient,
                        primary_color: Rgb::new(244, 63, 94),
                        secondary_color: Rgb::new(56, 189, 248),
                        speed: 35,
                        angle_deg: 35.0,
                        scale: 100,
                    },
                },
                LightingProfile {
                    id: "ambilight".to_string(),
                    name: "Ambilight".to_string(),
                    brightness: 90,
                    effect: EffectParams {
                        kind: EffectKind::Ambilight,
                        primary_color: Rgb::new(56, 189, 248),
                        secondary_color: Rgb::new(245, 158, 11),
                        speed: 55,
                        angle_deg: 0.0,
                        scale: 100,
                    },
                },
                LightingProfile {
                    id: "audio".to_string(),
                    name: "Audio".to_string(),
                    brightness: 90,
                    effect: EffectParams {
                        kind: EffectKind::AudioSpectrum,
                        primary_color: Rgb::new(34, 197, 94),
                        secondary_color: Rgb::new(14, 165, 233),
                        speed: 75,
                        angle_deg: 0.0,
                        scale: 100,
                    },
                },
                LightingProfile {
                    id: "night".to_string(),
                    name: "Night".to_string(),
                    brightness: 25,
                    effect: EffectParams {
                        kind: EffectKind::Solid,
                        primary_color: Rgb::new(255, 138, 61),
                        secondary_color: Rgb::new(120, 53, 15),
                        speed: 20,
                        angle_deg: 0.0,
                        scale: 100,
                    },
                },
                LightingProfile {
                    id: "off".to_string(),
                    name: "Off".to_string(),
                    brightness: 0,
                    effect: EffectParams {
                        kind: EffectKind::Off,
                        primary_color: Rgb::BLACK,
                        secondary_color: Rgb::BLACK,
                        speed: 1,
                        angle_deg: 0.0,
                        scale: 100,
                    },
                },
            ],
            placements: vec![
                DevicePlacement {
                    device_id: "asus-aura-0b05-19af".to_string(),
                    x: 0.08,
                    y: 0.14,
                    width: 0.28,
                    height: 0.36,
                    rotation_deg: 0.0,
                    led_count: 32,
                },
                DevicePlacement {
                    device_id: "asgard-ddr4-smbus".to_string(),
                    x: 0.42,
                    y: 0.16,
                    width: 0.12,
                    height: 0.32,
                    rotation_deg: 0.0,
                    led_count: 16,
                },
                DevicePlacement {
                    device_id: "aula-f75-he".to_string(),
                    x: 0.16,
                    y: 0.58,
                    width: 0.68,
                    height: 0.28,
                    rotation_deg: 0.0,
                    led_count: 80,
                },
            ],
            automation: AutomationConfig::default(),
            imported_hub_presets: Vec::new(),
        }
    }
}

impl AppConfig {
    pub fn active_profile(&self) -> Option<&LightingProfile> {
        self.profiles
            .iter()
            .find(|p| p.id == self.active_profile_id)
            .or_else(|| self.profiles.first())
    }

    pub fn active_profile_mut(&mut self) -> Option<&mut LightingProfile> {
        let id = self.active_profile_id.clone();
        self.profiles.iter_mut().find(|p| p.id == id)
    }

    pub fn default_config_path() -> Result<PathBuf, ConfigError> {
        let dirs = ProjectDirs::from("dev", "rgbstuff", "RGBStuff")
            .ok_or(ConfigError::NoConfigDirectory)?;
        Ok(dirs.config_dir().join("config.toml"))
    }

    pub fn load_or_default(path: &Path) -> Result<Self, ConfigError> {
        if !path.exists() {
            let cfg = Self::default();
            cfg.save_atomic(path)?;
            return Ok(cfg);
        }
        let raw = fs::read_to_string(path)?;
        let parsed: Self = toml::from_str(&raw)?;
        Ok(parsed)
    }

    pub fn save_atomic(&self, path: &Path) -> Result<(), ConfigError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let serialized = toml::to_string_pretty(self)?;
        let tmp_path = path.with_extension("toml.tmp");
        fs::write(&tmp_path, serialized)?;
        fs::rename(&tmp_path, path)?;
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("unable to determine user config directory")]
    NoConfigDirectory,
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("TOML serialize error: {0}")]
    TomlSerialize(#[from] toml::ser::Error),
    #[error("TOML parse error: {0}")]
    TomlDeserialize(#[from] toml::de::Error),
}
