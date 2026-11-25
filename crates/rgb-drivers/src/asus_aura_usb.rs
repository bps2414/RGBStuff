use hidapi::{HidApi, HidDevice};
use rgb_core::{DeviceKind, DeviceTransport, DriverError, LedPoint, LedZone, Rgb, RgbDevice};

pub const ASUS_VID: u16 = 0x0B05;
pub const AURA_PID: u16 = 0x19AF;
pub const AURA_USAGE_PAGE: u16 = 0xFF72;

pub struct AsusAuraUsbDriver {
    device: Option<HidDevice>,
    firmware: Option<String>,
    header_counts: [usize; 2],
    direct_mode_active: bool,
    last_error: Option<String>,
}

impl AsusAuraUsbDriver {
    pub fn new(header_counts: [usize; 2]) -> Self {
        let mut drv = Self {
            device: None,
            firmware: None,
            header_counts: [header_counts[0].clamp(1, 120), header_counts[1].clamp(1, 120)],
            direct_mode_active: false,
            last_error: None,
        };
        let _ = drv.reconnect();
        drv
    }

    pub fn set_header_counts(&mut self, counts: [usize; 2]) {
        self.header_counts = [counts[0].clamp(1, 120), counts[1].clamp(1, 120)];
    }

    fn drain_stale(dev: &HidDevice) {
        let mut buf = [0u8; 65];
        for _ in 0..6 {
            match dev.read_timeout(&mut buf, 4) {
                Ok(n) if n > 0 => continue,
                _ => break,
            }
        }
    }

    fn query_firmware(dev: &HidDevice) -> Option<String> {
        Self::drain_stale(dev);
        let mut out = [0u8; 65];
        out[0] = 0xEC;
        out[1] = 0x82;
        dev.write(&out).ok()?;
        let mut inp = [0u8; 65];
        let n = dev.read_timeout(&mut inp, 250).ok()?;
        if n < 4 || inp[0] != 0xEC || inp[1] != 0x02 {
            return None;
        }
        let s: String = inp[2..18]
            .iter()
            .take_while(|&&b| b != 0)
            .map(|&b| b as char)
            .collect();
        if s.is_empty() { None } else { Some(s) }
    }

    fn ensure_direct_mode(&mut self) -> Result<(), DriverError> {
        if self.direct_mode_active {
            return Ok(());
        }
        let dev = self
            .device
            .as_ref()
            .ok_or_else(|| DriverError::Disconnected("ASUS Aura 0B05:19AF".into()))?;
        for ch in 0u8..2u8 {
            let mut pkt = [0u8; 65];
            pkt[0] = 0xEC;
            pkt[1] = 0x35;
            pkt[2] = ch;
            pkt[5] = 0xFF; // Direct mode
            dev.write(&pkt)
                .map_err(|e| DriverError::Hid(format!("Aura direct mode write failed: {e}")))?;
        }
        self.direct_mode_active = true;
        Ok(())
    }

    fn write_channel_leds(
        dev: &HidDevice,
        channel: u8,
        leds: &[Rgb],
    ) -> Result<(), DriverError> {
        const MAX_LEDS_PER_PACKET: usize = 20;
        let total = leds.len();
        let mut offset = 0usize;
        while offset < total {
            let chunk_end = (offset + MAX_LEDS_PER_PACKET).min(total);
            let chunk = &leds[offset..chunk_end];
            let is_last = chunk_end == total;

            let mut pkt = [0u8; 65];
            pkt[0] = 0xEC;
            pkt[1] = 0x40;
            pkt[2] = if is_last { 0x80 | channel } else { channel };
            pkt[3] = offset as u8;
            pkt[4] = chunk.len() as u8;

            for (i, c) in chunk.iter().enumerate() {
                let base = 5 + i * 3;
                pkt[base] = c.r;
                pkt[base + 1] = c.g;
                pkt[base + 2] = c.b;
            }
            dev.write(&pkt)
                .map_err(|e| DriverError::Hid(format!("Aura write frame failed: {e}")))?;
            offset = chunk_end;
        }
        Ok(())
    }
}

impl RgbDevice for AsusAuraUsbDriver {
    fn id(&self) -> &str {
        "asus-aura-0b05-19af"
    }

    fn name(&self) -> &str {
        "ASUS Aura Controller (Fans ARGB)"
    }

    fn kind(&self) -> DeviceKind {
        DeviceKind::MotherboardAura
    }

    fn transport(&self) -> DeviceTransport {
        DeviceTransport::UsbHid
    }

    fn vid_pid(&self) -> &str {
        "0B05:19AF"
    }

    fn led_count(&self) -> usize {
        self.header_counts[0] + self.header_counts[1]
    }

    fn local_led_points(&self) -> Vec<LedPoint> {
        let mut pts = Vec::with_capacity(self.led_count());
        let h1 = self.header_counts[0].max(1);
        for i in 0..self.header_counts[0] {
            let t = i as f32 / h1 as f32 * std::f32::consts::TAU;
            pts.push(LedPoint {
                x: 0.25 + 0.18 * t.cos(),
                y: 0.5 + 0.36 * t.sin(),
            });
        }
        let h2 = self.header_counts[1].max(1);
        for i in 0..self.header_counts[1] {
            let t = i as f32 / h2 as f32 * std::f32::consts::TAU;
            pts.push(LedPoint {
                x: 0.75 + 0.18 * t.cos(),
                y: 0.5 + 0.36 * t.sin(),
            });
        }
        pts
    }

    fn is_connected(&self) -> bool {
        self.device.is_some()
    }

    fn zones(&self) -> Vec<LedZone> {
        vec![
            LedZone {
                id: "argb-1".into(),
                name: "ARGB Header 1".into(),
                led_count: self.header_counts[0],
            },
            LedZone {
                id: "argb-2".into(),
                name: "ARGB Header 2".into(),
                led_count: self.header_counts[1],
            },
        ]
    }

    fn status_note(&self) -> String {
        if let Some(fw) = &self.firmware {
            format!("FW {fw} · USB HID Direct")
        } else if let Some(err) = &self.last_error {
            err.clone()
        } else {
            "Disconnected".into()
        }
    }

    fn apply_frame(&mut self, colors: &[Rgb]) -> Result<(), DriverError> {
        if self.device.is_none() && !self.reconnect() {
            return Err(DriverError::Disconnected("ASUS Aura 0B05:19AF".into()));
        }
        if let Err(err) = self.ensure_direct_mode() {
            self.device = None;
            self.direct_mode_active = false;
            self.last_error = Some(err.to_string());
            return Err(err);
        }

        let dev = self
            .device
            .as_ref()
            .ok_or_else(|| DriverError::Disconnected("ASUS Aura 0B05:19AF".into()))?;
        let c0 = self.header_counts[0].min(colors.len());
        let slice0 = if c0 > 0 { &colors[..c0] } else { &[] };
        let slice1 = if colors.len() > c0 {
            &colors[c0..colors.len().min(c0 + self.header_counts[1])]
        } else {
            &[]
        };

        if let Err(err) = Self::write_channel_leds(dev, 0, slice0)
            .and_then(|_| Self::write_channel_leds(dev, 1, slice1))
        {
            self.device = None;
            self.direct_mode_active = false;
            self.last_error = Some(err.to_string());
            return Err(err);
        }
        Ok(())
    }

    fn reconnect(&mut self) -> bool {
        let Ok(api) = HidApi::new() else {
            self.last_error = Some("HidApi init failed".into());
            return false;
        };
        let info = api
            .device_list()
            .find(|d| {
                d.vendor_id() == ASUS_VID
                    && d.product_id() == AURA_PID
                    && d.usage_page() == AURA_USAGE_PAGE
            })
            .or_else(|| {
                api.device_list()
                    .find(|d| d.vendor_id() == ASUS_VID && d.product_id() == AURA_PID && d.usage_page() >= 0xFF00)
            });

        let Some(info) = info else {
            self.device = None;
            self.firmware = None;
            self.direct_mode_active = false;
            self.last_error = Some("0B05:19AF not detected".into());
            return false;
        };

        match info.open_device(&api) {
            Ok(dev) => {
                self.firmware = Self::query_firmware(&dev);
                self.device = Some(dev);
                self.direct_mode_active = false;
                self.last_error = None;
                true
            }
            Err(e) => {
                self.device = None;
                self.last_error = Some(format!("Open failed: {e}"));
                false
            }
        }
    }
}
