use crate::smbus_safety::validate_smbus_address;
use libloading::{Library, Symbol};
use rgb_core::{DeviceKind, DeviceTransport, DriverError, LedPoint, LedZone, Rgb, RgbDevice};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

pub const PAWNIO_DLL_PATH: &str = r"C:\Program Files\PawnIO\PawnIOLib.dll";
pub const SMBUS_PIPE_NAME: &str = r"\\.\pipe\rgbstuff-smbus-v1";
pub const ASGARD_DIMM_ADDRESSES: [u8; 2] = [0x71, 0x73];

pub struct EneDramIpcDriver {
    pawnio_version: Option<(u32, u32, u32)>,
    service_connected: bool,
    status: String,
    last_frame: Vec<Rgb>,
}

impl Default for EneDramIpcDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl EneDramIpcDriver {
    pub fn new() -> Self {
        let mut drv = Self {
            pawnio_version: None,
            service_connected: false,
            status: "Checking PawnIO".into(),
            last_frame: vec![Rgb::BLACK; 16],
        };
        let _ = drv.reconnect();
        drv
    }

    fn query_pawnio_version() -> Option<(u32, u32, u32)> {
        if !Path::new(PAWNIO_DLL_PATH).exists() {
            return None;
        }
        unsafe {
            let lib = Library::new(PAWNIO_DLL_PATH).ok()?;
            type PawnIoVersionFn = unsafe extern "system" fn(*mut u32) -> i32;
            let func: Symbol<PawnIoVersionFn> = lib.get(b"pawnio_version\0").ok()?;
            let mut ver: u32 = 0;
            if func(&mut ver) >= 0 {
                let major = (ver >> 16) & 0xFF;
                let minor = (ver >> 8) & 0xFF;
                let patch = ver & 0xFF;
                Some((major, minor, patch))
            } else {
                None
            }
        }
    }

    fn send_pipe_frame(&mut self, colors: &[Rgb]) -> Result<(), DriverError> {
        for addr in ASGARD_DIMM_ADDRESSES {
            validate_smbus_address(addr)?;
        }

        let Ok(mut pipe) = OpenOptions::new().write(true).open(SMBUS_PIPE_NAME) else {
            self.service_connected = false;
            return Err(DriverError::ServiceUnavailable(
                "rgbstuff-smbus-svc pipe not running".into(),
            ));
        };

        // Compact binary packet: [magic 'R','G', version=1, dimm_count=2, (addr, len, rgb...)]
        let mut payload = Vec::with_capacity(4 + colors.len() * 3 + 4);
        payload.extend_from_slice(b"RG");
        payload.push(1);
        payload.push(ASGARD_DIMM_ADDRESSES.len() as u8);

        let per_dimm = (colors.len() / 2).max(1).min(8);
        for (idx, addr) in ASGARD_DIMM_ADDRESSES.iter().enumerate() {
            payload.push(*addr);
            payload.push(per_dimm as u8);
            let start = idx * per_dimm;
            for led in 0..per_dimm {
                let c = colors.get(start + led).copied().unwrap_or(Rgb::BLACK);
                payload.push(c.r);
                payload.push(c.g);
                payload.push(c.b);
            }
        }

        pipe.write_all(&payload)
            .map_err(|e| DriverError::ServiceUnavailable(e.to_string()))?;
        self.service_connected = true;
        Ok(())
    }
}

impl RgbDevice for EneDramIpcDriver {
    fn id(&self) -> &str {
        "asgard-ddr4-smbus"
    }

    fn name(&self) -> &str {
        "Asgard DDR4 RGB (2x8GB)"
    }

    fn kind(&self) -> DeviceKind {
        DeviceKind::Ram
    }

    fn transport(&self) -> DeviceTransport {
        DeviceTransport::PawnIoSmbus
    }

    fn vid_pid(&self) -> &str {
        "1022:790B"
    }

    fn led_count(&self) -> usize {
        16
    }

    fn local_led_points(&self) -> Vec<LedPoint> {
        let mut pts = Vec::with_capacity(16);
        for stick in 0..2 {
            let x = if stick == 0 { 0.3 } else { 0.7 };
            for led in 0..8 {
                pts.push(LedPoint {
                    x,
                    y: led as f32 / 7.0,
                });
            }
        }
        pts
    }

    fn is_connected(&self) -> bool {
        self.pawnio_version.is_some()
    }

    fn zones(&self) -> Vec<LedZone> {
        vec![
            LedZone {
                id: "dimm-a2".into(),
                name: "DIMM A2 (Asgard 8GB)".into(),
                led_count: 8,
            },
            LedZone {
                id: "dimm-b2".into(),
                name: "DIMM B2 (Asgard 8GB)".into(),
                led_count: 8,
            },
        ]
    }

    fn status_note(&self) -> String {
        self.status.clone()
    }

    fn apply_frame(&mut self, colors: &[Rgb]) -> Result<(), DriverError> {
        self.last_frame = colors.to_vec();
        // Best effort over Named Pipe; silent no-op if elevated SMBus service isn't installed yet.
        let _ = self.send_pipe_frame(colors);
        Ok(())
    }

    fn reconnect(&mut self) -> bool {
        self.pawnio_version = Self::query_pawnio_version();
        let pipe_ok = OpenOptions::new().write(true).open(SMBUS_PIPE_NAME).is_ok();
        self.service_connected = pipe_ok;

        self.status = match (self.pawnio_version, pipe_ok) {
            (Some((maj, min, pat)), true) => {
                format!("PawnIO {maj}.{min}.{pat} · SMBus IPC active")
            }
            (Some((maj, min, pat)), false) => {
                format!("PawnIO {maj}.{min}.{pat} ready · SMBus service idle")
            }
            (None, _) => "PawnIO driver not found".into(),
        };
        self.pawnio_version.is_some()
    }
}
