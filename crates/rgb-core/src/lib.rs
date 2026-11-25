pub mod color;
pub mod config;
pub mod device;
pub mod hub_importer;

pub use color::{ColorParseError, Rgb, hsv_to_rgb};
pub use config::{
    AppConfig, AppProfileRule, AutomationConfig, ConfigError, EffectKind, EffectParams,
    LightingProfile, Locale,
};
pub use device::{
    BatteryDevice, BatteryTelemetry, DeviceCapability, DeviceId, DeviceKind, DevicePlacement,
    DeviceSnapshot, DeviceTransport, DriverError, LedPoint, LedZone, RgbDevice,
};
pub use hub_importer::{HubImportReport, ImportedHubItem, import_local_hub_presets};

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn hex_roundtrip_and_brightness() {
        let parsed = Rgb::from_hex("#38BDF8").expect("valid hex");
        assert_eq!(parsed, Rgb::new(56, 189, 248));
        assert_eq!(parsed.to_hex(), "#38BDF8");
        assert_eq!(parsed.scale_brightness(0), Rgb::BLACK);
        assert_eq!(parsed.scale_brightness(100), parsed);
        let half = parsed.scale_brightness(50);
        assert_eq!(half, Rgb::new(28, 95, 124));
    }

    #[test]
    fn hsv_primary_colors() {
        assert_eq!(hsv_to_rgb(0.0, 1.0, 1.0), Rgb::new(255, 0, 0));
        assert_eq!(hsv_to_rgb(120.0, 1.0, 1.0), Rgb::new(0, 255, 0));
        assert_eq!(hsv_to_rgb(240.0, 1.0, 1.0), Rgb::new(0, 0, 255));
    }

    #[test]
    fn config_atomic_roundtrip() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("rgbstuff-test-{unique}"));
        let path = dir.join("config.toml");

        let mut cfg = AppConfig::default();
        cfg.locale = Locale::EnUs;
        cfg.global_brightness = 64;
        cfg.save_atomic(&path).expect("save atomic");

        let loaded = AppConfig::load_or_default(&path).expect("load back");
        assert_eq!(loaded, cfg);

        let _ = fs_cleanup(&dir);
    }

    fn fs_cleanup(dir: &std::path::Path) -> std::io::Result<()> {
        if dir.exists() {
            std::fs::remove_dir_all(dir)?;
        }
        Ok(())
    }
}
