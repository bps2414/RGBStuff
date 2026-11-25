use crate::asus_aura_usb::AsusAuraUsbDriver;
use crate::aula_f75_he::AulaF75HeDriver;
use crate::ene_dram_ipc::EneDramIpcDriver;
use crate::mchose_v9::MchoseV9Driver;
use crate::rapoo_hid::RapooHidDriver;
use rgb_core::{
    BatteryDevice, BatteryTelemetry, DeviceCapability, DeviceSnapshot, DeviceTransport, LedPoint,
    Rgb, RgbDevice,
};
use std::collections::HashMap;

pub struct DeviceManager {
    pub aura: AsusAuraUsbDriver,
    pub ram: EneDramIpcDriver,
    pub aula: AulaF75HeDriver,
    pub rapoo: RapooHidDriver,
    pub mchose: MchoseV9Driver,
    telemetry_cache: HashMap<String, BatteryTelemetry>,
}

impl DeviceManager {
    pub fn new(aura_counts: [usize; 2]) -> Self {
        let mut mgr = Self {
            aura: AsusAuraUsbDriver::new(aura_counts),
            ram: EneDramIpcDriver::new(),
            aula: AulaF75HeDriver::new(),
            rapoo: RapooHidDriver::new(),
            mchose: MchoseV9Driver::new(),
            telemetry_cache: HashMap::new(),
        };
        mgr.poll_telemetry();
        mgr
    }

    pub fn update_aura_led_counts(&mut self, counts: [usize; 2]) {
        self.aura.set_header_counts(counts);
    }

    pub fn reconnect_all(&mut self) {
        self.aura.reconnect();
        self.ram.reconnect();
        self.aula.reconnect();
        self.rapoo.reconnect();
        self.mchose.reconnect();
        self.poll_telemetry();
    }

    pub fn poll_telemetry(&mut self) {
        if let Ok(t) = self.rapoo.poll_telemetry() {
            self.telemetry_cache.insert(self.rapoo.id().to_string(), t);
        }
        if let Ok(t) = self.mchose.poll_telemetry() {
            self.telemetry_cache.insert(self.mchose.id().to_string(), t);
        }
    }

    pub fn local_points_for(&self, device_id: &str) -> Vec<LedPoint> {
        match device_id {
            id if id == self.aura.id() => self.aura.local_led_points(),
            id if id == self.ram.id() => self.ram.local_led_points(),
            id if id == self.aula.id() => self.aula.local_led_points(),
            _ => Vec::new(),
        }
    }

    pub fn apply_device_frame(&mut self, device_id: &str, colors: &[Rgb]) {
        let res = match device_id {
            id if id == self.aura.id() => self.aura.apply_frame(colors),
            id if id == self.ram.id() => self.ram.apply_frame(colors),
            id if id == self.aula.id() => self.aula.apply_frame(colors),
            _ => Ok(()),
        };
        let _ = res;
    }

    pub fn snapshots(&self) -> Vec<DeviceSnapshot> {
        let mut out = Vec::with_capacity(5);
        for dev in [
            &self.aura as &dyn RgbDevice,
            &self.ram as &dyn RgbDevice,
            &self.aula as &dyn RgbDevice,
        ] {
            out.push(DeviceSnapshot {
                id: dev.id().to_string(),
                name: dev.name().to_string(),
                kind: dev.kind(),
                transport: dev.transport(),
                vid_pid: dev.vid_pid().to_string(),
                connected: dev.is_connected(),
                status_note: dev.status_note(),
                capabilities: vec![DeviceCapability::Rgb],
                zones: dev.zones(),
                telemetry: None,
            });
        }

        let rapoo_t = self
            .telemetry_cache
            .get(self.rapoo.id())
            .cloned()
            .unwrap_or_default();
        out.push(DeviceSnapshot {
            id: self.rapoo.id().to_string(),
            name: self.rapoo.name().to_string(),
            kind: self.rapoo.kind(),
            transport: DeviceTransport::UsbHid,
            vid_pid: self.rapoo.vid_pid().to_string(),
            connected: rapoo_t.connected,
            status_note: self.rapoo.status_note(),
            capabilities: vec![
                DeviceCapability::Battery,
                DeviceCapability::DpiRead,
                DeviceCapability::PeripheralConfig,
            ],
            zones: Vec::new(),
            telemetry: Some(rapoo_t),
        });

        let mchose_t = self
            .telemetry_cache
            .get(self.mchose.id())
            .cloned()
            .unwrap_or_default();
        out.push(DeviceSnapshot {
            id: self.mchose.id().to_string(),
            name: self.mchose.name().to_string(),
            kind: self.mchose.kind(),
            transport: DeviceTransport::UsbHid,
            vid_pid: self.mchose.vid_pid().to_string(),
            connected: mchose_t.connected,
            status_note: self.mchose.status_note(),
            capabilities: vec![DeviceCapability::Battery, DeviceCapability::PeripheralConfig],
            zones: Vec::new(),
            telemetry: Some(mchose_t),
        });

        out
    }
}
