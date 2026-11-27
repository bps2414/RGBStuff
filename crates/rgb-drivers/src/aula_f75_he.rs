use hidapi::{HidApi, HidDevice};
use rgb_core::{
    DeviceKind, DeviceTransport, DriverError, HardwareEffect, LedPoint, LedZone, Rgb, RgbDevice,
};

pub const AULA_VID: u16 = 0x38A6;
pub const AULA_PID: u16 = 0x2729;
pub const AULA_USAGE_PAGE_PRIMARY: u16 = 0xFF68;

/// HFD SDK opcodes extracted from Aula Hub (`hfd-sdk.es`).
pub const HFD_CMD_GET_DEVICE_INFO: u8 = 16; // 0x10
pub const HFD_CMD_GET_LED_EFFECT: u8 = 19; // 0x13
pub const HFD_CMD_SET_LED_EFFECT: u8 = 35; // 0x23
pub const HFD_CMD_SET_CUSTOM_LED_DATA: u8 = 36; // 0x24
pub const HFD_CMD_LIVE_LED_STREAM: u8 = 50; // 0x32 (GET_LED_DATA used by Fe/setMusicData on 64B wired)

const HFD_MODE_OFF: u8 = 0;
const HFD_MODE_STATIC: u8 = 1;
const HFD_MODE_CUSTOM: u8 = 20;

/// Builds a single Windows `hidapi` output report (`report_len + 1` bytes including Report ID `0x00`)
/// matching HFD SDK's `U(cmd, len, addr, data, report_len, otherHeader, customHeader, isLast)`.
pub fn build_hfd_packet(
    report_len: usize,
    cmd: u8,
    addr: u16,
    payload: &[u8],
    other0: u8,
    is_last: bool,
) -> Vec<u8> {
    let mut buf = vec![0u8; report_len + 1];
    buf[0] = 0x00; // HID Report ID 0
    buf[1] = 0xAA; // HFD header magic (170)
    buf[2] = cmd;
    buf[3] = payload.len() as u8;
    buf[4] = (addr & 0xFF) as u8;
    buf[5] = ((addr >> 8) & 0xFF) as u8;
    buf[6] = other0;
    buf[7] = if is_last { 1 } else { 0 };
    buf[8] = 0x00;
    let max_copy = payload.len().min(report_len.saturating_sub(8));
    buf[9..9 + max_copy].copy_from_slice(&payload[..max_copy]);
    buf
}

/// Builds the 16-byte `SET_LED_EFFECT` payload for hardware static/off lighting.
pub fn build_led_effect_payload(mode: u8, primary: Rgb, secondary: Rgb, brightness: u8) -> [u8; 16] {
    let mut data = build_hw_effect_payload(mode, primary, false, brightness, 2, false);
    data[5] = secondary.r;
    data[6] = secondary.g;
    data[7] = secondary.b;
    data
}

/// Builds the 16-byte `SET_LED_EFFECT` payload for any firmware mode, laid out as
/// the vendor hub writes it: `[mode, r, g, b, 255, r2, g2, b2, colorMode,
/// brightness, speed, direction, effectModeType, 0, 0xAA, 0x55]`.
pub fn build_hw_effect_payload(
    mode: u8,
    color: Rgb,
    colorful: bool,
    brightness: u8,
    speed: u8,
    reverse: bool,
) -> [u8; 16] {
    let mut data = [0u8; 16];
    data[0] = mode;
    data[1] = color.r;
    data[2] = color.g;
    data[3] = color.b;
    data[4] = 255;
    data[8] = colorful as u8; // colorMode: 0 = fixed RGB, 1 = random colors
    data[9] = brightness;
    data[10] = speed;
    data[11] = reverse as u8; // direction
    data[12] = 0; // effectModeType
    data[14] = 0xAA;
    data[15] = 0x55;
    data
}

/// Builds the 512-byte `[led_id, r, g, b] * 128` payload for per-key lighting.
pub fn build_per_key_payload(colors: &[Rgb]) -> Vec<u8> {
    let mut data = vec![0u8; 512];
    for i in 0..128 {
        let c = if colors.is_empty() {
            Rgb::BLACK
        } else {
            let idx = (i * colors.len()) / 128;
            colors[idx.min(colors.len() - 1)]
        };
        let base = i * 4;
        data[base] = i as u8;
        data[base + 1] = c.r;
        data[base + 2] = c.g;
        data[base + 3] = c.b;
    }
    data
}

pub struct AulaF75HeDriver {
    device: Option<HidDevice>,
    report_len: usize,
    max_brightness: u8,
    custom_mode_active: bool,
    last_uniform: Option<Rgb>,
    /// Uniform color streamed live but not yet committed as the hardware effect.
    settle_color: Option<Rgb>,
    /// A firmware animation owns the LEDs; the next frame must take them back.
    hw_effect_active: bool,
    status: String,
}

impl Default for AulaF75HeDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl AulaF75HeDriver {
    pub fn new() -> Self {
        let mut drv = Self {
            device: None,
            report_len: 64,
            max_brightness: 4,
            custom_mode_active: false,
            last_uniform: None,
            settle_color: None,
            hw_effect_active: false,
            status: "Initializing".into(),
        };
        let _ = drv.reconnect();
        drv
    }

    fn drain_stale(dev: &HidDevice) {
        let mut buf = [0u8; 65];
        for _ in 0..4 {
            match dev.read_timeout(&mut buf, 2) {
                Ok(n) if n > 0 => continue,
                _ => break,
            }
        }
    }

    fn probe_report_len(dev: &HidDevice) -> (usize, Option<u8>) {
        Self::drain_stale(dev);
        for candidate in [64usize, 32usize] {
            let pkt = build_hfd_packet(candidate, HFD_CMD_GET_LED_EFFECT, 0, &[], 0, true);
            if dev.write(&pkt).is_ok() {
                let mut resp = [0u8; 65];
                if let Ok(n) = dev.read_timeout(&mut resp, 120)
                    && n >= 10
                {
                    // Response starts with 0x55 (or 0x00, 0x55)
                    let off = if resp[0] == 0x55 {
                        0
                    } else if resp[1] == 0x55 {
                        1
                    } else {
                        continue;
                    };
                    if resp.get(off + 1) == Some(&HFD_CMD_GET_LED_EFFECT) {
                        let bright = resp.get(off + 8 + 9).copied();
                        return (candidate, bright);
                    }
                }
            }
        }
        (64, None)
    }

    fn send_chunked(&self, dev: &HidDevice, cmd: u8, payload: &[u8], other0: u8) -> Result<(), DriverError> {
        let chunk_cap = self.report_len.saturating_sub(8).max(1);
        let total_chunks = payload.len().div_ceil(chunk_cap).max(1);
        for idx in 0..total_chunks {
            let start = idx * chunk_cap;
            let end = (start + chunk_cap).min(payload.len());
            let slice = if start < payload.len() {
                &payload[start..end]
            } else {
                &[]
            };
            let is_last = idx + 1 == total_chunks;
            let pkt = build_hfd_packet(self.report_len, cmd, start as u16, slice, other0, is_last);
            dev.write(&pkt)
                .map_err(|e| DriverError::Hid(format!("Aula HID write cmd=0x{cmd:02X} failed: {e}")))?;
            let mut ack = [0u8; 65];
            let _ = dev.read_timeout(&mut ack, 8);
        }
        Ok(())
    }

    fn send_static_or_off(&mut self, color: Rgb) -> Result<(), DriverError> {
        let dev = self
            .device
            .as_ref()
            .ok_or_else(|| DriverError::Disconnected("Aula F75 HE (38A6:2729)".into()))?;
        let is_black = color == Rgb::BLACK;
        let mode = if is_black { HFD_MODE_OFF } else { HFD_MODE_STATIC };
        let payload = build_led_effect_payload(mode, color, Rgb::BLACK, self.max_brightness);
        self.send_chunked(dev, HFD_CMD_SET_LED_EFFECT, &payload, 0)?;
        self.custom_mode_active = false;
        self.last_uniform = Some(color);
        Ok(())
    }

    fn send_per_key_frame(&mut self, colors: &[Rgb]) -> Result<(), DriverError> {
        let dev = self
            .device
            .as_ref()
            .ok_or_else(|| DriverError::Disconnected("Aula F75 HE (38A6:2729)".into()))?;
        let payload = build_per_key_payload(colors);
        if self.report_len == 64 {
            // Live RAM per-key stream (HFD `Fe` / `setMusicData` wired path -> cmd 50, zero flash wear)
            self.send_chunked(dev, HFD_CMD_LIVE_LED_STREAM, &payload, 0)?;
        } else {
            if !self.custom_mode_active {
                let eff = build_led_effect_payload(
                    HFD_MODE_CUSTOM,
                    colors[0],
                    Rgb::BLACK,
                    self.max_brightness,
                );
                self.send_chunked(dev, HFD_CMD_SET_LED_EFFECT, &eff, 0)?;
                self.custom_mode_active = true;
            }
            self.send_chunked(dev, HFD_CMD_SET_CUSTOM_LED_DATA, &payload, 0)?;
        }
        self.last_uniform = None;
        Ok(())
    }
}

impl RgbDevice for AulaF75HeDriver {
    fn id(&self) -> &str {
        "aula-f75-he"
    }

    fn name(&self) -> &str {
        "Aula F75 HE"
    }

    fn kind(&self) -> DeviceKind {
        DeviceKind::Keyboard
    }

    fn transport(&self) -> DeviceTransport {
        DeviceTransport::UsbHid
    }

    fn vid_pid(&self) -> &str {
        "38A6:2729"
    }

    fn led_count(&self) -> usize {
        80
    }

    fn local_led_points(&self) -> Vec<LedPoint> {
        let cols = 16usize;
        let rows = 5usize;
        let mut pts = Vec::with_capacity(cols * rows);
        for r in 0..rows {
            for c in 0..cols {
                pts.push(LedPoint {
                    x: c as f32 / (cols - 1) as f32,
                    y: r as f32 / (rows - 1) as f32,
                });
            }
        }
        pts
    }

    fn is_connected(&self) -> bool {
        self.device.is_some()
    }

    fn zones(&self) -> Vec<LedZone> {
        vec![LedZone {
            id: "per-key-75".into(),
            name: "75% Key Matrix (80 LEDs)".into(),
            led_count: 80,
        }]
    }

    fn status_note(&self) -> String {
        self.status.clone()
    }

    fn apply_frame(&mut self, colors: &[Rgb]) -> Result<(), DriverError> {
        if self.device.is_none() && !self.reconnect() {
            return Err(DriverError::Disconnected("Aula F75 HE (38A6:2729)".into()));
        }
        if colors.is_empty() {
            return self.send_static_or_off(Rgb::BLACK);
        }

        let first = colors[0];
        let uniform = colors.iter().all(|&c| c == first);

        // Leaving a firmware animation: switch the keyboard back to a plain
        // static mode first, otherwise the animation keeps drawing over the stream.
        if self.hw_effect_active {
            self.hw_effect_active = false;
            if let Err(e) = self.send_static_or_off(first) {
                self.device = None;
                return Err(e);
            }
            if uniform {
                return Ok(());
            }
        }

        // SET_LED_EFFECT is slow to show (the firmware applies and stores it), so
        // a fade of uniform colors sent that way lagged far behind the other
        // devices. Every changing frame goes through the live RAM stream; a
        // uniform color is committed as the hardware effect only once it holds
        // still (see `settle`), so it also survives the app closing.
        let res = if uniform && self.last_uniform == Some(first) {
            Ok(())
        } else if self.report_len == 64 {
            self.settle_color = uniform.then_some(first);
            self.send_per_key_frame(colors)
        } else if uniform {
            self.send_static_or_off(first)
        } else {
            self.send_per_key_frame(colors)
        };

        if res.is_err() {
            self.device = None;
            self.custom_mode_active = false;
            self.last_uniform = None;
            self.settle_color = None;
        }
        res
    }

    fn settle(&mut self) -> Result<(), DriverError> {
        match self.settle_color.take() {
            Some(c) if self.device.is_some() && self.last_uniform != Some(c) => {
                self.send_static_or_off(c)
            }
            _ => Ok(()),
        }
    }

    fn apply_hardware_effect(&mut self, fx: &HardwareEffect) -> Result<(), DriverError> {
        if self.device.is_none() && !self.reconnect() {
            return Err(DriverError::Disconnected("Aula F75 HE (38A6:2729)".into()));
        }
        let brightness =
            ((fx.brightness_pct.min(100) as u32 * self.max_brightness as u32 + 50) / 100) as u8;
        let (mode, brightness) = if brightness == 0 {
            (HFD_MODE_OFF, 0)
        } else {
            (fx.mode.clamp(2, 19), brightness)
        };
        let payload = build_hw_effect_payload(
            mode,
            fx.color,
            fx.colorful,
            brightness,
            fx.speed.clamp(1, 5),
            fx.reverse,
        );
        let res = {
            let dev = self.device.as_ref().expect("checked above");
            self.send_chunked(dev, HFD_CMD_SET_LED_EFFECT, &payload, 0)
        };
        self.custom_mode_active = false;
        self.last_uniform = None;
        self.settle_color = None;
        self.hw_effect_active = res.is_ok();
        if res.is_err() {
            self.device = None;
        }
        res
    }

    fn reconnect(&mut self) -> bool {
        let Ok(api) = HidApi::new() else {
            self.device = None;
            self.status = "HidApi unavailable".into();
            return false;
        };

        let found = api
            .device_list()
            .find(|d| {
                d.vendor_id() == AULA_VID
                    && d.product_id() == AULA_PID
                    && d.usage_page() == AULA_USAGE_PAGE_PRIMARY
            })
            .or_else(|| {
                api.device_list().find(|d| {
                    d.vendor_id() == AULA_VID
                        && d.product_id() == AULA_PID
                        && d.usage_page() == 0xFF67
                })
            });

        match found {
            Some(info) => match info.open_device(&api) {
                Ok(dev) => {
                    let (report_len, current_bright) = Self::probe_report_len(&dev);
                    self.report_len = report_len;
                    // Aula F75 HE HFD brightness scale is 0..=5 (or 0..=100 on newer revs)
                    self.max_brightness = match current_bright {
                        Some(b) if b > 10 => 100,
                        _ => 5,
                    };
                    self.device = Some(dev);
                    self.custom_mode_active = false;
                    self.last_uniform = None;
                    self.status = format!(
                        "HFD HID iface={} ({}B · 0x{:04X}) · Active",
                        info.interface_number(),
                        self.report_len,
                        info.usage_page()
                    );
                    true
                }
                Err(e) => {
                    self.device = None;
                    self.status = format!("HID busy: {e}");
                    false
                }
            },
            None => {
                self.device = None;
                self.status = "38A6:2729 disconnected".into();
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_hfd_64b_packet_with_report_id() {
        let payload = build_led_effect_payload(HFD_MODE_STATIC, Rgb::new(10, 20, 30), Rgb::BLACK, 4);
        let pkt = build_hfd_packet(64, HFD_CMD_SET_LED_EFFECT, 0, &payload, 0, true);
        assert_eq!(pkt.len(), 65);
        assert_eq!(pkt[0], 0x00);
        assert_eq!(pkt[1], 0xAA);
        assert_eq!(pkt[2], HFD_CMD_SET_LED_EFFECT);
        assert_eq!(pkt[3], 16);
        assert_eq!(pkt[7], 1); // is_last
        assert_eq!(pkt[9], HFD_MODE_STATIC);
        assert_eq!(pkt[10], 10);
        assert_eq!(pkt[11], 20);
        assert_eq!(pkt[12], 30);
        assert_eq!(pkt[13], 255);
        assert_eq!(pkt[23], 0xAA);
        assert_eq!(pkt[24], 0x55);
    }

    #[test]
    fn builds_512b_per_key_payload() {
        let data = build_per_key_payload(&[Rgb::new(255, 0, 0), Rgb::new(0, 0, 255)]);
        assert_eq!(data.len(), 512);
        assert_eq!(&data[0..4], &[0, 255, 0, 0]);
        assert_eq!(&data[508..512], &[127, 0, 0, 255]);
    }
}
