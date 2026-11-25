use crate::asus_aura_usb::AsusAuraUsbDriver;
use crate::aula_f75_he::AulaF75HeDriver;
use crate::ene_dram_ipc::EneDramIpcDriver;
use crate::mchose_v9::MchoseV9Driver;
use crate::rapoo_hid::RapooHidDriver;
use rgb_core::{
    BatteryDevice, BatteryTelemetry, DeviceCapability, DeviceKind, DeviceSnapshot,
    DeviceTransport, LedPoint, Rgb, RgbDevice,
};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Latest-frame slot: a slow bus never builds a queue, it just skips to the newest frame.
#[derive(Default)]
struct Mailbox {
    frame: Mutex<Option<Vec<Rgb>>>,
    ready: Condvar,
}

/// Owns one RGB driver on its own output thread, so a slow bus (per-key HID with
/// acks, SMBus) never stalls the render loop or the other devices.
pub struct DeviceWorker<T: RgbDevice + 'static> {
    driver: Arc<Mutex<T>>,
    mailbox: Arc<Mailbox>,
    id: String,
    points: Vec<LedPoint>,
}

impl<T: RgbDevice + 'static> DeviceWorker<T> {
    fn spawn(driver: T) -> Self {
        let id = driver.id().to_string();
        let points = driver.local_led_points();
        let driver = Arc::new(Mutex::new(driver));
        let mailbox = Arc::new(Mailbox::default());
        let (drv, mb) = (Arc::clone(&driver), Arc::clone(&mailbox));
        let _ = std::thread::Builder::new()
            .name(format!("rgb-out-{id}"))
            .spawn(move || {
                loop {
                    let frame = {
                        let mut slot = lock(&mb.frame);
                        loop {
                            if let Some(f) = slot.take() {
                                break f;
                            }
                            slot = mb.ready.wait(slot).unwrap_or_else(|e| e.into_inner());
                        }
                    };
                    let failed = lock(&drv).apply_frame(&frame).is_err();
                    if failed {
                        // Unplugged or busy: drivers reconnect inside apply_frame, so
                        // retrying every frame would enumerate HID 60 times a second.
                        std::thread::sleep(std::time::Duration::from_secs(2));
                    }
                }
            });
        Self {
            driver,
            mailbox,
            id,
            points,
        }
    }

    fn submit(&self, colors: &[Rgb]) {
        *lock(&self.mailbox.frame) = Some(colors.to_vec());
        self.mailbox.ready.notify_one();
    }

    fn with<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
        f(&mut lock(&self.driver))
    }

    fn refresh_points(&mut self) {
        self.points = self.with(|d| d.local_led_points());
    }
}

pub struct DeviceManager {
    aura: DeviceWorker<AsusAuraUsbDriver>,
    ram: DeviceWorker<EneDramIpcDriver>,
    aula: DeviceWorker<AulaF75HeDriver>,
    batteries: Arc<Batteries>,
    peripherals: [PeripheralMeta; 2],
}

/// Battery peripherals are polled off the render thread: their HID reads wait
/// for replies and would stall the lighting for a moment on every poll.
struct Batteries {
    rapoo: Mutex<RapooHidDriver>,
    mchose: Mutex<MchoseV9Driver>,
    /// device id -> (telemetry, status note)
    cache: Mutex<HashMap<String, (BatteryTelemetry, String)>>,
    polling: AtomicBool,
}

impl Batteries {
    fn poll(&self, reconnect: bool) {
        fn one<D: BatteryDevice>(
            drv: &Mutex<D>,
            reconnect: bool,
        ) -> (String, Option<(BatteryTelemetry, String)>) {
            let mut d = lock(drv);
            if reconnect {
                d.reconnect();
            }
            let res = d.poll_telemetry().ok().map(|t| (t, d.status_note()));
            (d.id().to_string(), res)
        }
        let results = [one(&self.rapoo, reconnect), one(&self.mchose, reconnect)];
        let mut cache = lock(&self.cache);
        for (id, res) in results {
            if let Some(v) = res {
                cache.insert(id, v);
            }
        }
    }

    fn poll_in_background(self: &Arc<Self>, reconnect: bool) {
        if self.polling.swap(true, Ordering::AcqRel) {
            return;
        }
        let me = Arc::clone(self);
        let spawned = std::thread::Builder::new()
            .name("rgb-battery-poll".into())
            .spawn(move || {
                me.poll(reconnect);
                me.polling.store(false, Ordering::Release);
            });
        if spawned.is_err() {
            self.polling.store(false, Ordering::Release);
        }
    }
}

struct PeripheralMeta {
    id: String,
    name: String,
    kind: DeviceKind,
    vid_pid: String,
    capabilities: Vec<DeviceCapability>,
}

impl PeripheralMeta {
    fn of<D: BatteryDevice>(d: &D, capabilities: Vec<DeviceCapability>) -> Self {
        Self {
            id: d.id().to_string(),
            name: d.name().to_string(),
            kind: d.kind(),
            vid_pid: d.vid_pid().to_string(),
            capabilities,
        }
    }
}

impl DeviceManager {
    pub fn new(aura_counts: [usize; 2]) -> Self {
        let rapoo = RapooHidDriver::new();
        let mchose = MchoseV9Driver::new();
        let peripherals = [
            PeripheralMeta::of(
                &rapoo,
                vec![
                    DeviceCapability::Battery,
                    DeviceCapability::DpiRead,
                    DeviceCapability::PeripheralConfig,
                ],
            ),
            PeripheralMeta::of(
                &mchose,
                vec![DeviceCapability::Battery, DeviceCapability::PeripheralConfig],
            ),
        ];
        let batteries = Arc::new(Batteries {
            rapoo: Mutex::new(rapoo),
            mchose: Mutex::new(mchose),
            cache: Mutex::new(HashMap::new()),
            polling: AtomicBool::new(false),
        });
        batteries.poll(false);
        Self {
            aura: DeviceWorker::spawn(AsusAuraUsbDriver::new(aura_counts)),
            ram: DeviceWorker::spawn(EneDramIpcDriver::new()),
            aula: DeviceWorker::spawn(AulaF75HeDriver::new()),
            batteries,
            peripherals,
        }
    }

    pub fn update_aura_led_counts(&mut self, counts: [usize; 2]) {
        self.aura.with(|d| d.set_header_counts(counts));
        self.aura.refresh_points();
    }

    pub fn reconnect_all(&mut self) {
        self.aura.with(|d| d.reconnect());
        self.ram.with(|d| d.reconnect());
        self.aula.with(|d| d.reconnect());
        self.aura.refresh_points();
        self.ram.refresh_points();
        self.aula.refresh_points();
        self.batteries.poll_in_background(true);
    }

    /// Refreshes battery telemetry in the background; `snapshots` returns the
    /// last values read.
    pub fn poll_telemetry(&mut self) {
        self.batteries.poll_in_background(false);
    }

    pub fn local_points_for(&self, device_id: &str) -> &[LedPoint] {
        match device_id {
            id if id == self.aura.id => &self.aura.points,
            id if id == self.ram.id => &self.ram.points,
            id if id == self.aula.id => &self.aula.points,
            _ => &[],
        }
    }

    /// Hands the frame to the device's output thread and returns immediately.
    pub fn apply_device_frame(&mut self, device_id: &str, colors: &[Rgb]) {
        match device_id {
            id if id == self.aura.id => self.aura.submit(colors),
            id if id == self.ram.id => self.ram.submit(colors),
            id if id == self.aula.id => self.aula.submit(colors),
            _ => {}
        }
    }

    pub fn snapshots(&self) -> Vec<DeviceSnapshot> {
        let mut out = Vec::with_capacity(5);
        let rgb = |dev: &dyn RgbDevice| DeviceSnapshot {
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
        };
        out.push(self.aura.with(|d| rgb(d)));
        out.push(self.ram.with(|d| rgb(d)));
        out.push(self.aula.with(|d| rgb(d)));

        let cache = lock(&self.batteries.cache);
        for meta in &self.peripherals {
            let (telemetry, note) = cache.get(&meta.id).cloned().unwrap_or_default();
            out.push(DeviceSnapshot {
                id: meta.id.clone(),
                name: meta.name.clone(),
                kind: meta.kind,
                transport: DeviceTransport::UsbHid,
                vid_pid: meta.vid_pid.clone(),
                connected: telemetry.connected,
                status_note: note,
                capabilities: meta.capabilities.clone(),
                zones: Vec::new(),
                telemetry: Some(telemetry),
            });
        }
        out
    }
}
