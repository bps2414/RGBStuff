use hidapi::{HidApi, HidDevice};
use rgb_core::{BatteryDevice, BatteryTelemetry, DeviceKind, DriverError};

pub const MCHOSE_VID: u16 = 0x3837;
pub const MCHOSE_PID: u16 = 0x6011;

const MCHOSE_REPORT_TIMEOUT_MS: i32 = 15;
const MCHOSE_MAX_DRAIN_REPORTS: usize = 4;

fn is_known_status_cmd(cmd: u8) -> bool {
    matches!(
        cmd,
        0x01 | 0x02 | 0x03 | 0x04 | 0x05 | 0x10 | 0x11 | 0x81 | 0x82
    )
}

/// Parses a 64/65-byte MCHOSE V9 Pro 2 vendor HID status report.
pub fn parse_mchose_status_report(buf: &[u8]) -> Option<BatteryTelemetry> {
    if buf.len() < 64 {
        return None;
    }

    let payload = if buf.len() >= 65 && buf[0] == 0x00 {
        &buf[1..]
    } else {
        buf
    };

    if payload.len() < 64 {
        return None;
    }

    let header = payload[0];
    let cmd = payload[1];

    let header_matches = match header {
        0xAA | 0x11 => is_known_status_cmd(cmd),
        0x00 => cmd != 0x00 && is_known_status_cmd(cmd),
        _ => false,
    };

    if !header_matches {
        return None;
    }

    let battery_pct = payload[2];
    if battery_pct > 100 {
        return None;
    }

    let charging_flag = payload[3];
    if charging_flag > 2 {
        return None;
    }

    Some(BatteryTelemetry {
        percent: Some(battery_pct),
        charging: charging_flag != 0,
        connected: true,
        sleeping: false,
        dpi: None,
    })
}

pub struct MchoseV9Driver {
    device: Option<HidDevice>,
    connected: bool,
    telemetry: BatteryTelemetry,
    status: String,
}

impl Default for MchoseV9Driver {
    fn default() -> Self {
        Self::new()
    }
}

impl MchoseV9Driver {
    pub fn new() -> Self {
        let mut drv = Self {
            device: None,
            connected: false,
            telemetry: BatteryTelemetry::default(),
            status: "Initializing".into(),
        };
        let _ = drv.reconnect();
        drv
    }
}

impl BatteryDevice for MchoseV9Driver {
    fn id(&self) -> &str {
        "mchose-v9-pro-2"
    }

    fn name(&self) -> &str {
        "MCHOSE V9 Pro 2"
    }

    fn kind(&self) -> DeviceKind {
        DeviceKind::Headset
    }

    fn vid_pid(&self) -> &str {
        "3837:6011"
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

        let Some(dev) = self.device.as_ref() else {
            return Ok(self.telemetry.clone());
        };

        let mut buf = [0u8; 65];
        for _ in 0..MCHOSE_MAX_DRAIN_REPORTS {
            match dev.read_timeout(&mut buf, MCHOSE_REPORT_TIMEOUT_MS) {
                Ok(n) if n > 0 => {
                    if let Some(parsed) = parse_mchose_status_report(&buf[..n]) {
                        self.telemetry = parsed;
                        self.status = "HID iface=3 (0xFF07) · Live telemetry".into();
                    }
                }
                Ok(_) => break,
                Err(_) => {
                    self.device = None;
                    self.connected = false;
                    self.telemetry.connected = false;
                    self.status = "3837:6011 receiver disconnected".into();
                    break;
                }
            }
        }

        Ok(self.telemetry.clone())
    }

    fn reconnect(&mut self) -> bool {
        let Ok(api) = HidApi::new() else {
            self.device = None;
            self.connected = false;
            self.telemetry.connected = false;
            self.status = "HidApi unavailable".into();
            return false;
        };

        let found = api.device_list().find(|d| {
            d.vendor_id() == MCHOSE_VID
                && d.product_id() == MCHOSE_PID
                && d.interface_number() == 3
                && d.usage_page() == 0xFF07
        });

        match found {
            Some(info) => {
                self.device = info.open_device(&api).ok();
                self.connected = self.device.is_some();
                self.telemetry.connected = self.connected;
                self.status = if self.connected {
                    "HID iface=3 (0xFF07) · Ready".into()
                } else {
                    "HID detected · Open busy".into()
                };
                self.connected
            }
            None => {
                self.device = None;
                self.connected = false;
                self.telemetry.connected = false;
                self.status = "3837:6011 receiver disconnected".into();
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_mchose_status_reports() {
        let mut pkt64 = [0u8; 64];
        pkt64[0] = 0xAA;
        pkt64[1] = 0x01;
        pkt64[2] = 78;
        pkt64[3] = 1;
        let t64 = parse_mchose_status_report(&pkt64).expect("valid 64-byte 0xAA report");
        assert_eq!(t64.percent, Some(78));
        assert!(t64.charging);
        assert!(t64.connected);
        assert_eq!(t64.dpi, None);

        let mut pkt65 = [0u8; 65];
        pkt65[0] = 0x00;
        pkt65[1] = 0x11;
        pkt65[2] = 0x02;
        pkt65[3] = 100;
        pkt65[4] = 0;
        let t65 = parse_mchose_status_report(&pkt65).expect("valid 65-byte 0x00+0x11 report");
        assert_eq!(t65.percent, Some(100));
        assert!(!t65.charging);
    }

    #[test]
    fn rejects_invalid_or_short_mchose_reports() {
        assert!(parse_mchose_status_report(&[0xAA, 0x01, 50, 0]).is_none());
        assert!(parse_mchose_status_report(&[0u8; 64]).is_none());

        let mut bad_header = [0u8; 64];
        bad_header[0] = 0x55;
        bad_header[1] = 0x01;
        bad_header[2] = 50;
        assert!(parse_mchose_status_report(&bad_header).is_none());

        let mut bad_pct = [0u8; 64];
        bad_pct[0] = 0xAA;
        bad_pct[1] = 0x01;
        bad_pct[2] = 101;
        assert!(parse_mchose_status_report(&bad_pct).is_none());
    }
}
