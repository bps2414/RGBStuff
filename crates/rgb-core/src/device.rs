use crate::color::Rgb;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DeviceId(pub String);

impl DeviceId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceKind {
    MotherboardAura,
    Ram,
    Keyboard,
    Mouse,
    Headset,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceCapability {
    Rgb,
    Battery,
    DpiRead,
    PeripheralConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LedZone {
    pub id: String,
    pub name: String,
    pub led_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LedPoint {
    /// Normalized coordinate inside the device bounding box (`0.0..=1.0`).
    pub x: f32,
    /// Normalized coordinate inside the device bounding box (`0.0..=1.0`).
    pub y: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DevicePlacement {
    pub device_id: String,
    /// Normalized canvas X (`0.0..=1.0`).
    pub x: f32,
    /// Normalized canvas Y (`0.0..=1.0`).
    pub y: f32,
    /// Normalized canvas width (`0.05..=1.0`).
    pub width: f32,
    /// Normalized canvas height (`0.05..=1.0`).
    pub height: f32,
    pub rotation_deg: f32,
    pub led_count: usize,
    /// Per-device output gain in percent (`0..=200`). Above 100 boosts dim
    /// content up to full LED drive while keeping the hue.
    #[serde(default = "default_device_brightness")]
    pub brightness: u8,
}

fn default_device_brightness() -> u8 {
    100
}

impl DevicePlacement {
    /// Generates normalized 2D coordinates on the global canvas (`0.0..=1.0`) for each LED.
    pub fn sample_points(&self, local_points: &[LedPoint]) -> Vec<LedPoint> {
        let count = self.led_count.max(1);
        let rad = self.rotation_deg.to_radians();
        let (sin_a, cos_a) = rad.sin_cos();
        let cx = self.x + self.width * 0.5;
        let cy = self.y + self.height * 0.5;

        (0..count)
            .map(|i| {
                let local = if let Some(pt) = local_points.get(i) {
                    *pt
                } else if count == 1 {
                    LedPoint { x: 0.5, y: 0.5 }
                } else {
                    LedPoint {
                        x: i as f32 / (count - 1) as f32,
                        y: 0.5,
                    }
                };
                let dx = (local.x - 0.5) * self.width;
                let dy = (local.y - 0.5) * self.height;
                LedPoint {
                    x: (cx + dx * cos_a - dy * sin_a).clamp(0.0, 1.0),
                    y: (cy + dx * sin_a + dy * cos_a).clamp(0.0, 1.0),
                }
            })
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct BatteryTelemetry {
    pub percent: Option<u8>,
    pub charging: bool,
    pub connected: bool,
    pub sleeping: bool,
    pub dpi: Option<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceTransport {
    UsbHid,
    PawnIoSmbus,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeviceSnapshot {
    pub id: String,
    pub name: String,
    pub kind: DeviceKind,
    pub transport: DeviceTransport,
    pub vid_pid: String,
    pub connected: bool,
    pub status_note: String,
    pub capabilities: Vec<DeviceCapability>,
    pub zones: Vec<LedZone>,
    pub telemetry: Option<BatteryTelemetry>,
}

#[derive(Debug, Error)]
pub enum DriverError {
    #[error("device disconnected: {0}")]
    Disconnected(String),
    #[error("hid I/O error: {0}")]
    Hid(String),
    #[error("smbus safety violation: {0}")]
    SmbusSafety(String),
    #[error("service unavailable: {0}")]
    ServiceUnavailable(String),
}

pub trait RgbDevice: Send {
    fn id(&self) -> &str;
    fn name(&self) -> &str;
    fn kind(&self) -> DeviceKind;
    fn transport(&self) -> DeviceTransport;
    fn vid_pid(&self) -> &str;
    fn led_count(&self) -> usize;
    fn local_led_points(&self) -> Vec<LedPoint>;
    fn is_connected(&self) -> bool;
    fn zones(&self) -> Vec<LedZone>;
    fn status_note(&self) -> String;
    fn apply_frame(&mut self, colors: &[Rgb]) -> Result<(), DriverError>;
    fn apply_static_color(&mut self, color: Rgb) -> Result<(), DriverError> {
        let count = self.led_count().max(1);
        self.apply_frame(&vec![color; count])
    }
    fn reconnect(&mut self) -> bool;
}

pub trait BatteryDevice: Send {
    fn id(&self) -> &str;
    fn name(&self) -> &str;
    fn kind(&self) -> DeviceKind;
    fn vid_pid(&self) -> &str;
    fn status_note(&self) -> String;
    fn poll_telemetry(&mut self) -> Result<BatteryTelemetry, DriverError>;
    fn reconnect(&mut self) -> bool;
}
