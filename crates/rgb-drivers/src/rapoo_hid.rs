use hidapi::{HidApi, HidDevice};
use rgb_core::{BatteryDevice, BatteryTelemetry, DeviceKind, DriverError};

pub const RAPOO_VID: u16 = 0x24AE;
pub const RAPOO_PID: u16 = 0x14A1;

const RAPOO_REPORT_TIMEOUT_MS: i32 = 15;
const RAPOO_MAX_DRAIN_REPORTS: usize = 4;

/// Parses a Rapoo vendor HID status report (`0x07`).
pub fn parse_rapoo_status_report(buf: &[u8]) -> Option<BatteryTelemetry> {
    if buf.len() < 9 || buf[0] != 0x07 {
        return None;
    }

    let dpi_raw = u16::from_le_bytes([buf[3], buf[4]]);
    let dpi = if (100..=30000).contains(&dpi_raw) {
        Some(dpi_raw)
    } else {
        None
    };
    let charging = buf[7] == 2;
    let percent = if buf[8] <= 100 { Some(buf[8]) } else { None };

    Some(BatteryTelemetry {
        percent,
        charging,
        connected: true,
        sleeping: false,
        dpi,
    })
}

pub struct RapooHidDriver {
    device: Option<HidDevice>,
    fallback_device: Option<HidDevice>,
    connected: bool,
    telemetry: BatteryTelemetry,
    status: String,
}

impl Default for RapooHidDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl RapooHidDriver {
    pub fn new() -> Self {
        let mut drv = Self {
            device: None,
            fallback_device: None,
            connected: false,
            telemetry: BatteryTelemetry::default(),
            status: "Initializing".into(),
        };
        let _ = drv.reconnect();
        drv
    }

    fn drain_reports(&mut self) {
        let mut buf = [0u8; 65];
        let mut got_report = false;
        let mut disconnected = false;

        if let Some(dev) = self.device.as_ref() {
            for _ in 0..RAPOO_MAX_DRAIN_REPORTS {
                match dev.read_timeout(&mut buf, RAPOO_REPORT_TIMEOUT_MS) {
                    Ok(n) if n > 0 => {
                        if let Some(parsed) = parse_rapoo_status_report(&buf[..n]) {
                            self.telemetry = parsed;
                            got_report = true;
                        }
                    }
                    Ok(_) => break,
                    Err(_) => {
                        disconnected = true;
                        break;
                    }
                }
            }
        }

        if disconnected {
            self.device = None;
            self.fallback_device = None;
            self.connected = false;
            self.telemetry.connected = false;
            self.status = "24AE:14A1 dongle disconnected".into();
            return;
        }

        if !got_report {
            if let Some(fallback) = self.fallback_device.as_ref() {
                for _ in 0..RAPOO_MAX_DRAIN_REPORTS {
                    match fallback.read_timeout(&mut buf, RAPOO_REPORT_TIMEOUT_MS) {
                        Ok(n) if n > 0 => {
                            if let Some(parsed) = parse_rapoo_status_report(&buf[..n]) {
                                self.telemetry = parsed;
                                got_report = true;
                            }
                        }
                        Ok(_) => break,
                        Err(_) => {
                            self.fallback_device = None;
                            break;
                        }
                    }
                }
            }
        }

        if got_report {
            self.status = "HID iface=2 (0xFF00) · Live telemetry".into();
        }
    }
}

impl BatteryDevice for RapooHidDriver {
    fn id(&self) -> &str {
        "rapoo-vt-24ae-14a1"
    }

    fn name(&self) -> &str {
        "Rapoo Wireless Mouse"
    }

    fn kind(&self) -> DeviceKind {
        DeviceKind::Mouse
    }

    fn vid_pid(&self) -> &str {
        "24AE:14A1"
    }

    fn status_note(&self) -> String {
        self.status.clone()
    }

    fn poll_telemetry(&mut self) -> Result<BatteryTelemetry, DriverError> {
        if !self.connected && !self.reconnect() {
            return Ok(BatteryTelemetry {
                connected: false,
                ..Default::default()
            });
        }

        self.drain_reports();
        Ok(self.telemetry.clone())
    }

    fn reconnect(&mut self) -> bool {
        let Ok(api) = HidApi::new() else {
            self.device = None;
            self.fallback_device = None;
            self.connected = false;
            self.telemetry.connected = false;
            self.status = "HidApi unavailable".into();
            return false;
        };

        let primary_info = api.device_list().find(|d| {
            d.vendor_id() == RAPOO_VID
                && d.product_id() == RAPOO_PID
                && d.interface_number() == 2
                && d.usage_page() == 0xFF00
        });

        let fallback_info = api.device_list().find(|d| {
            d.vendor_id() == RAPOO_VID
                && d.product_id() == RAPOO_PID
                && d.interface_number() == 1
                && d.usage_page() == 0xFF00
        });

        if primary_info.is_none() && fallback_info.is_none() {
            self.device = None;
            self.fallback_device = None;
            self.connected = false;
            self.telemetry.connected = false;
            self.status = "24AE:14A1 dongle disconnected".into();
            return false;
        }

        self.device = primary_info.and_then(|info| info.open_device(&api).ok());
        self.fallback_device = fallback_info.and_then(|info| info.open_device(&api).ok());

        self.connected = self.device.is_some() || self.fallback_device.is_some();
        self.telemetry.connected = self.connected;

        self.status = if self.device.is_some() {
            "HID iface=2 (0xFF00) · Passive telemetry".into()
        } else if self.fallback_device.is_some() {
            "HID iface=1 (0xFF00) · Passive telemetry".into()
        } else {
            "HID detected · Open busy".into()
        };

        self.connected
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_rapoo_status_report_valid() {
        let report = [0x07, 0x20, 0x01, 0x40, 0x06, 0x40, 0x06, 0x01, 0x5D];
        let parsed = parse_rapoo_status_report(&report).expect("valid Rapoo 0x07 report");
        assert_eq!(parsed.percent, Some(93));
        assert!(!parsed.charging);
        assert!(parsed.connected);
        assert!(!parsed.sleeping);
        assert_eq!(parsed.dpi, Some(1600));
    }

    #[test]
    fn rejects_invalid_or_short_rapoo_reports() {
        assert!(parse_rapoo_status_report(&[0x07, 0x20, 0x01]).is_none());
        assert!(parse_rapoo_status_report(&[0x08, 0x20, 0x01, 0x40, 0x06, 0, 0, 1, 50]).is_none());
    }
}
