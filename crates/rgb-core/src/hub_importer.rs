use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportedHubItem {
    pub hub: String,
    pub device: String,
    pub preset_name: String,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HubImportReport {
    pub rapoo_found: bool,
    pub aula_found: bool,
    pub mchose_found: bool,
    pub items: Vec<ImportedHubItem>,
    pub imported_at_unix: u64,
}

/// Parses a Rapoo A HUB JSON config (e.g. `Factory Config.json`) into a human-readable summary.
pub fn parse_rapoo_config_json(preset_name: &str, raw_json: &str) -> Option<ImportedHubItem> {
    if !raw_json.contains("deviceName") && !raw_json.contains("returnRate") {
        return None;
    }

    let device = extract_json_string(raw_json, "deviceName").unwrap_or_else(|| "Rapoo Mouse".into());
    let return_rate = extract_json_number(raw_json, "returnRate");
    let sensor_mode = extract_json_number(raw_json, "modeID");

    let rate_hz = match return_rate {
        Some(0) => "125Hz",
        Some(1) => "250Hz",
        Some(2) => "500Hz",
        Some(3) => "1000Hz",
        Some(4) => "2000Hz",
        Some(5) => "4000Hz",
        Some(6) => "8000Hz",
        _ => "Auto",
    };

    let mut summary_parts = vec![format!("Polling {rate_hz}")];
    if let Some(m) = sensor_mode {
        summary_parts.push(format!("SensorMode #{m}"));
    }

    Some(ImportedHubItem {
        hub: "A HUB (Rapoo)".into(),
        device,
        preset_name: preset_name.to_string(),
        summary: summary_parts.join(" · "),
    })
}

/// Extracts non-sensitive Aula F75 HE metadata from Aula Hub's LevelDB `.log`/`.ldb` bytes.
pub fn extract_aula_metadata_from_bytes(data: &[u8]) -> Vec<ImportedHubItem> {
    let text = String::from_utf8_lossy(data);
    let mut items = Vec::new();

    if text.contains("AULA F75 HE") {
        let fw = extract_json_string(&text, "binVersion").unwrap_or_else(|| "Detected".into());
        let theme = extract_json_string(&text, "themeColor").unwrap_or_else(|| "Default".into());
        items.push(ImportedHubItem {
            hub: "Aula Hub".into(),
            device: "Aula F75 HE".into(),
            preset_name: format!("Device Profile ({theme})"),
            summary: format!("FW {fw} · Theme {theme}"),
        });
    } else if let Some(theme) = extract_json_string(&text, "themeColor") {
        items.push(ImportedHubItem {
            hub: "Aula Hub".into(),
            device: "Aula F75 HE".into(),
            preset_name: format!("Hub Theme ({theme})"),
            summary: format!("Theme {theme}"),
        });
    }

    items
}

fn extract_json_string(text: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\"");
    let key_pos = text.find(&needle)?;
    let after_key = text[key_pos + needle.len()..].trim_start();
    let after_colon = after_key.strip_prefix(':')?.trim_start();
    let rest = after_colon.strip_prefix('"')?;
    let end = rest.find('"')?;
    let val = &rest[..end];
    if val.is_empty() || val.len() > 64 {
        None
    } else {
        Some(val.to_string())
    }
}

fn extract_json_number(text: &str, key: &str) -> Option<u32> {
    let needle = format!("\"{key}\"");
    let key_pos = text.find(&needle)?;
    let after_key = text[key_pos + needle.len()..].trim_start();
    let rest = after_key.strip_prefix(':')?.trim_start();
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse::<u32>().ok()
}

fn collect_files_with_ext(root: &Path, exts: &[&str], out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_files_with_ext(&path, exts, out);
        } else if let Some(ext) = path.extension().and_then(|e| e.to_str())
            && exts.iter().any(|wanted| wanted.eq_ignore_ascii_case(ext))
        {
            out.push(path);
        }
    }
}

/// Strictly read-only import of local peripheral presets/configs from A HUB, Aula Hub, and MCHOSE HUB.
/// Never reads cookies, tokens, or auth databases.
pub fn import_local_hub_presets() -> HubImportReport {
    let mut items = Vec::new();
    let local_app = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
    let roaming_app = std::env::var_os("APPDATA").map(PathBuf::from);

    let mut rapoo_found = false;
    if let Some(local) = &local_app {
        let rapoo_configs = local.join("A HUB").join("configs");
        if rapoo_configs.exists() {
            rapoo_found = true;
            let mut json_files = Vec::new();
            collect_files_with_ext(&rapoo_configs, &["json"], &mut json_files);
            for file in json_files {
                if file.file_name().and_then(|n| n.to_str()) == Some("config.json") {
                    continue;
                }
                if let Ok(raw) = fs::read_to_string(&file) {
                    let stem = file
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("Profile");
                    if let Some(item) = parse_rapoo_config_json(stem, &raw) {
                        items.push(item);
                    }
                }
            }
        }
    }

    let mut aula_found = false;
    if let Some(roaming) = &roaming_app {
        let aula_dir = roaming.join("aula-hub");
        if aula_dir.exists() {
            aula_found = true;
            let leveldb_dir = aula_dir.join("Local Storage").join("leveldb");
            let mut files = Vec::new();
            collect_files_with_ext(&leveldb_dir, &["log", "ldb"], &mut files);
            for file in files {
                if let Ok(bytes) = fs::read(&file) {
                    for item in extract_aula_metadata_from_bytes(&bytes) {
                        if !items.iter().any(|x| x.hub == item.hub && x.summary == item.summary) {
                            items.push(item);
                        }
                    }
                }
            }
            if !items.iter().any(|x| x.hub == "Aula Hub") {
                items.push(ImportedHubItem {
                    hub: "Aula Hub".into(),
                    device: "Aula F75 HE".into(),
                    preset_name: "Onboard Default".into(),
                    summary: "Aula Hub installed · Ready".into(),
                });
            }
        }
    }

    let mut mchose_found = false;
    if let Some(roaming) = &roaming_app {
        let mchose_dir = roaming.join("MCHOSEHUB");
        if mchose_dir.exists() {
            mchose_found = true;
            let mut files = Vec::new();
            collect_files_with_ext(&mchose_dir.join("Local Storage"), &["log", "ldb"], &mut files);
            collect_files_with_ext(&mchose_dir.join("WebStorage"), &["log", "ldb"], &mut files);
            let mut found_eq = false;
            for file in files {
                if let Ok(bytes) = fs::read(&file) {
                    let text = String::from_utf8_lossy(&bytes);
                    if text.contains("V9 Pro") || text.contains("equalizer") {
                        found_eq = true;
                        break;
                    }
                }
            }
            items.push(ImportedHubItem {
                hub: "MCHOSE HUB".into(),
                device: "MCHOSE V9 Pro 2".into(),
                preset_name: if found_eq {
                    "Synced Headset Profile".into()
                } else {
                    "Default EQ / Factory".into()
                },
                summary: if found_eq {
                    "EQ & status metadata detected".into()
                } else {
                    "MCHOSE HUB installed · Factory EQ".into()
                },
            });
        }
    }

    let imported_at_unix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    HubImportReport {
        rapoo_found,
        aula_found,
        mchose_found,
        items,
        imported_at_unix,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_rapoo_factory_json() {
        let raw = r#"{
            "chipType": 6,
            "deviceName": "VT0Air MAX",
            "returnRate": 3,
            "sensorMode": { "id": 6, "modeID": 5 }
        }"#;
        let item = parse_rapoo_config_json("Factory Config", raw).expect("parsed");
        assert_eq!(item.hub, "A HUB (Rapoo)");
        assert_eq!(item.device, "VT0Air MAX");
        assert!(item.summary.contains("1000Hz"));
        assert!(item.summary.contains("SensorMode #5"));
    }

    #[test]
    fn extracts_aula_metadata() {
        let bytes = br#"prefix {"pName":"AULA F75 HE","binVersion":"V1.55","themeColor":"paleClouds"} suffix"#;
        let items = extract_aula_metadata_from_bytes(bytes);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].device, "Aula F75 HE");
        assert!(items[0].summary.contains("V1.55"));
    }
}
