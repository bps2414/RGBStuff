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
    Aurora,
    Plasma,
    Ripple,
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
            night_mode_enabled: false,
            night_start: "23:00".to_string(),
            night_end: "07:00".to_string(),
            night_brightness: 25,
            low_battery_warn_pct: 20,
            low_battery_crit_pct: 10,
            start_with_windows: false,
            app_rules: vec![
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

/// What the Studio window's close button does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CloseAction {
    /// Destroy the window (freeing WebView2) and keep running in the tray.
    #[default]
    Tray,
    /// Quit RGBStuff entirely.
    Quit,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct UiPrefs {
    pub close_action: CloseAction,
    /// The "still running in the tray" notice is shown only once.
    pub tray_hint_shown: bool,
    /// Devices whose battery level is listed in the tray right-click menu.
    pub tray_battery_devices: Vec<String>,
    /// Look for a new release on startup and every few hours.
    pub auto_update_check: bool,
}

impl Default for UiPrefs {
    fn default() -> Self {
        Self {
            close_action: CloseAction::Tray,
            tray_hint_shown: false,
            tray_battery_devices: Vec::new(),
            auto_update_check: true,
        }
    }
}

pub const CURRENT_SCHEMA_VERSION: u32 = 2;

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
    #[serde(default)]
    pub ui: UiPrefs,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            schema_version: CURRENT_SCHEMA_VERSION,
            locale: Locale::PtBr,
            power_on: true,
            global_brightness: 100,
            active_profile_id: "work".to_string(),
            aura_argb_led_counts: [16, 16],
            profiles: vec![
                LightingProfile {
                    id: "work".to_string(),
                    name: "Work".to_string(),
                    brightness: 100,
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
                    brightness: 100,
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
                    brightness: 100,
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
                    brightness: 100,
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
                    brightness: 100,
                },
                DevicePlacement {
                    device_id: "asgard-ddr4-smbus".to_string(),
                    x: 0.42,
                    y: 0.16,
                    width: 0.12,
                    height: 0.32,
                    rotation_deg: 0.0,
                    led_count: 16,
                    brightness: 100,
                },
                DevicePlacement {
                    device_id: "aula-f75-he".to_string(),
                    x: 0.16,
                    y: 0.58,
                    width: 0.68,
                    height: 0.28,
                    rotation_deg: 0.0,
                    led_count: 80,
                    brightness: 100,
                },
            ],
            automation: AutomationConfig::default(),
            imported_hub_presets: Vec::new(),
            ui: UiPrefs::default(),
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
        let mut parsed: Self = toml::from_str(&raw)?;
        if parsed.migrate() {
            let _ = parsed.save_atomic(path);
        }
        Ok(parsed)
    }

    /// Upgrades configs written by older versions. Returns true when anything changed.
    ///
    /// v1 shipped night mode on by default (capping everything to 25% from 23:00)
    /// and dimmed the built-in profiles to 70-90%, which read as "the LEDs are weak".
    pub fn migrate(&mut self) -> bool {
        if self.schema_version >= CURRENT_SCHEMA_VERSION {
            return false;
        }
        if self.schema_version < 2 {
            self.automation.night_mode_enabled = false;
            for p in &mut self.profiles {
                if matches!(p.id.as_str(), "work" | "gaming" | "ambilight" | "audio") {
                    p.brightness = 100;
                }
            }
        }
        self.schema_version = CURRENT_SCHEMA_VERSION;
        true
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
