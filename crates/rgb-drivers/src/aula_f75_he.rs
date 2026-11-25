use hidapi::HidApi;
use rgb_core::{DeviceKind, DeviceTransport, DriverError, LedPoint, LedZone, Rgb, RgbDevice};

pub const AULA_VID: u16 = 0x38A6;
pub const AULA_PID: u16 = 0x2729;

pub struct AulaF75HeDriver {
    connected: bool,
    active_interface: Option<i32>,
    last_frame: Vec<Rgb>,
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
            connected: false,
            active_interface: None,
            last_frame: vec![Rgb::BLACK; 80],
            status: "Initializing".into(),
        };
        let _ = drv.reconnect();
        drv
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
        self.connected
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
        if !self.connected && !self.reconnect() {
            return Err(DriverError::Disconnected("Aula F75 HE (38A6:2729)".into()));
        }
        self.last_frame = colors.to_vec();
        // Phase 1B: wire verified SinoWealth/Telink 0xFF68 per-key streaming packet.
        // We intentionally avoid blind writes to keyboard flash before packet capture verification.
        Ok(())
    }

    fn reconnect(&mut self) -> bool {
        let Ok(api) = HidApi::new() else {
            self.connected = false;
            self.status = "HidApi unavailable".into();
            return false;
        };

        let found = api.device_list().find(|d| {
            d.vendor_id() == AULA_VID
                && d.product_id() == AULA_PID
                && (d.usage_page() == 0xFF68 || d.usage_page() == 0xFF67)
        });

        match found {
            Some(info) => {
                self.connected = info.open_device(&api).is_ok();
                self.active_interface = Some(info.interface_number());
                self.status = if self.connected {
                    format!(
                        "HID iface={} (0x{:04X}) · Ready",
                        info.interface_number(),
                        info.usage_page()
                    )
                } else {
                    "HID detected · Open busy".into()
                };
                self.connected
            }
            None => {
                self.connected = false;
                self.active_interface = None;
                self.status = "38A6:2729 disconnected".into();
                false
            }
        }
    }
}
