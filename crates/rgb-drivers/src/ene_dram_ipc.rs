use crate::smbus_safety::validate_smbus_address;
use libloading::{Library, Symbol};
use rgb_core::{DeviceKind, DeviceTransport, DriverError, LedPoint, LedZone, Rgb, RgbDevice};
use std::ffi::CString;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, WAIT_ABANDONED, WAIT_OBJECT_0};
use windows_sys::Win32::System::Threading::{CreateMutexW, ReleaseMutex, WaitForSingleObject};

pub const PAWNIO_DLL_PATH: &str = r"C:\Program Files\PawnIO\PawnIOLib.dll";
pub const SMBUS_PIPE_NAME: &str = r"\\.\pipe\rgbstuff-smbus-v1";
pub const SMBUS_PIIX4_BLOB: &[u8] = include_bytes!("../assets/SmbusPIIX4.bin");

const I2C_SMBUS_WRITE: u64 = 0;
const I2C_SMBUS_READ: u64 = 1;
const I2C_SMBUS_BYTE_DATA: u64 = 2;
const I2C_SMBUS_WORD_DATA: u64 = 3;
const I2C_SMBUS_BLOCK_DATA: u64 = 5;

const ENE_REG_DEVICE_NAME: u16 = 0x1000;
const ENE_REG_MICRON_CHECK: u16 = 0x1030;
const ENE_REG_CONFIG_TABLE: u16 = 0x1C00;
const ENE_REG_COLORS_DIRECT_V1: u16 = 0x8000;
const ENE_REG_COLORS_DIRECT_V2: u16 = 0x8100;
const ENE_REG_DIRECT: u16 = 0x8020;
const ENE_REG_APPLY: u16 = 0x80A0;
const ENE_APPLY_VAL: u8 = 0x01;

type PawnIoVersionFn = unsafe extern "system" fn(*mut u32) -> i32;
type PawnIoOpenFn = unsafe extern "system" fn(*mut isize) -> i32;
type PawnIoLoadFn = unsafe extern "system" fn(isize, *const u8, usize) -> i32;
type PawnIoExecuteFn = unsafe extern "system" fn(
    isize,
    *const i8,
    *const u64,
    usize,
    *mut u64,
    usize,
    *mut usize,
) -> i32;
type PawnIoCloseFn = unsafe extern "system" fn(isize) -> i32;

/// RAII guard for the official Windows SMBus mutex shared with HWiNFO, OpenRGB, and AIDA64.
pub struct GlobalSmbusMutex {
    handle: HANDLE,
    locked: bool,
}

impl GlobalSmbusMutex {
    pub fn acquire(timeout_ms: u32) -> Result<Self, String> {
        let mut wide: Vec<u16> = "Global\\Access_SMBUS.HTP.Method".encode_utf16().collect();
        wide.push(0);
        let handle = unsafe { CreateMutexW(std::ptr::null(), 0, wide.as_ptr()) };
        if handle.is_null() {
            return Err("CreateMutexW(Global\\Access_SMBUS.HTP.Method) failed".into());
        }
        let wait = unsafe { WaitForSingleObject(handle, timeout_ms) };
        if wait == WAIT_OBJECT_0 || wait == WAIT_ABANDONED {
            Ok(Self {
                handle,
                locked: true,
            })
        } else {
            unsafe { CloseHandle(handle) };
            Err(format!("SMBus mutex wait timeout/error: 0x{wait:08X}"))
        }
    }
}

impl Drop for GlobalSmbusMutex {
    fn drop(&mut self) {
        if self.locked {
            unsafe {
                ReleaseMutex(self.handle);
            }
        }
        unsafe {
            CloseHandle(self.handle);
        }
    }
}

/// Packs SMBus block payload `[len, data...]` into 5 little-endian `u64` cells for `SmbusPIIX4`.
pub fn pack_smbus_block_le(data: &[u8]) -> [u64; 5] {
    let mut bytes = [0u8; 40];
    let len = data.len().min(32);
    bytes[0] = len as u8;
    bytes[1..1 + len].copy_from_slice(&data[..len]);
    let mut cells = [0u64; 5];
    for i in 0..5 {
        let mut chunk = [0u8; 8];
        chunk.copy_from_slice(&bytes[i * 8..(i + 1) * 8]);
        cells[i] = u64::from_le_bytes(chunk);
    }
    cells
}

/// Encodes an RGB color into ENE DRAM's native `[R, B, G]` byte order.
#[inline]
pub const fn encode_ene_rbg(c: Rgb) -> [u8; 3] {
    [c.r, c.b, c.g]
}

#[derive(Debug, Clone)]
pub struct DetectedEneDimm {
    pub addr: u8,
    pub port: i64,
    pub device_name: String,
    pub direct_reg: u16,
    pub led_count: usize,
    pub direct_initialized: bool,
}

pub struct PawnIoSmbusSession {
    lib: Library,
    handle: isize,
    pub version: (u32, u32, u32),
}

// Safety: PawnIoSmbusSession owns its Library + kernel HANDLE exclusively and is only used under &mut self.
unsafe impl Send for PawnIoSmbusSession {}

impl PawnIoSmbusSession {
    pub fn open() -> Result<Self, String> {
        if !Path::new(PAWNIO_DLL_PATH).exists() {
            return Err("PawnIOLib.dll not found".into());
        }
        unsafe {
            let lib = Library::new(PAWNIO_DLL_PATH)
                .map_err(|e| format!("Failed to load PawnIOLib.dll: {e}"))?;
            let ver_fn: Symbol<PawnIoVersionFn> = lib
                .get(b"pawnio_version\0")
                .map_err(|e| format!("missing pawnio_version: {e}"))?;
            let open_fn: Symbol<PawnIoOpenFn> = lib
                .get(b"pawnio_open\0")
                .map_err(|e| format!("missing pawnio_open: {e}"))?;
            let load_fn: Symbol<PawnIoLoadFn> = lib
                .get(b"pawnio_load\0")
                .map_err(|e| format!("missing pawnio_load: {e}"))?;
            let close_fn: Symbol<PawnIoCloseFn> = lib
                .get(b"pawnio_close\0")
                .map_err(|e| format!("missing pawnio_close: {e}"))?;

            let mut raw_ver = 0u32;
            let _ = ver_fn(&mut raw_ver);
            let version = ((raw_ver >> 16) & 0xFF, (raw_ver >> 8) & 0xFF, raw_ver & 0xFF);

            let mut handle: isize = 0;
            let hr = open_fn(&mut handle);
            if hr < 0 || handle == 0 {
                return Err(format!("pawnio_open failed: HRESULT 0x{hr:08X}"));
            }

            let load_hr = load_fn(handle, SMBUS_PIIX4_BLOB.as_ptr(), SMBUS_PIIX4_BLOB.len());
            if load_hr < 0 {
                let _ = close_fn(handle);
                return Err(format!(
                    "pawnio_load(SmbusPIIX4.bin) failed: HRESULT 0x{load_hr:08X}"
                ));
            }

            Ok(Self {
                lib,
                handle,
                version,
            })
        }
    }

    pub fn execute(&self, name: &str, input: &[u64], out_len: usize) -> Result<Vec<u64>, String> {
        let cname = CString::new(name).map_err(|e| e.to_string())?;
        let mut out = vec![0u64; out_len];
        let mut returned: usize = 0;
        unsafe {
            let exec_fn: Symbol<PawnIoExecuteFn> = self
                .lib
                .get(b"pawnio_execute\0")
                .map_err(|e| format!("missing pawnio_execute: {e}"))?;
            let hr = exec_fn(
                self.handle,
                cname.as_ptr(),
                input.as_ptr(),
                input.len(),
                if out_len == 0 {
                    std::ptr::null_mut()
                } else {
                    out.as_mut_ptr()
                },
                out_len,
                &mut returned,
            );
            if hr < 0 {
                return Err(format!("pawnio_execute({name}) failed: HRESULT 0x{hr:08X}"));
            }
        }
        out.truncate(returned.min(out_len));
        Ok(out)
    }

    pub fn identity(&self) -> Result<(u64, u64, u64), String> {
        let out = self.execute("ioctl_identity", &[], 3)?;
        if out.len() < 3 {
            return Err("ioctl_identity returned short response".into());
        }
        Ok((out[0], out[1], out[2]))
    }

    pub fn select_port(&self, port: i64) -> Result<i64, String> {
        let out = self.execute("ioctl_piix4_port_sel", &[port as u64], 1)?;
        Ok(out.first().copied().unwrap_or(0) as i64)
    }

    pub fn write_word_data(&self, addr: u8, cmd: u8, value: u16) -> Result<(), String> {
        validate_smbus_address(addr).map_err(|e| e.to_string())?;
        let input = [
            addr as u64,
            I2C_SMBUS_WRITE,
            cmd as u64,
            I2C_SMBUS_WORD_DATA,
            value as u64,
        ];
        self.execute("ioctl_smbus_xfer", &input, 0)?;
        Ok(())
    }

    pub fn write_byte_data(&self, addr: u8, cmd: u8, value: u8) -> Result<(), String> {
        validate_smbus_address(addr).map_err(|e| e.to_string())?;
        let input = [
            addr as u64,
            I2C_SMBUS_WRITE,
            cmd as u64,
            I2C_SMBUS_BYTE_DATA,
            value as u64,
        ];
        self.execute("ioctl_smbus_xfer", &input, 0)?;
        Ok(())
    }

    pub fn read_byte_data(&self, addr: u8, cmd: u8) -> Result<u8, String> {
        validate_smbus_address(addr).map_err(|e| e.to_string())?;
        let input = [addr as u64, I2C_SMBUS_READ, cmd as u64, I2C_SMBUS_BYTE_DATA];
        let out = self.execute("ioctl_smbus_xfer", &input, 1)?;
        Ok(out.first().copied().unwrap_or(0) as u8)
    }

    pub fn write_block_data(&self, addr: u8, cmd: u8, data: &[u8]) -> Result<(), String> {
        validate_smbus_address(addr).map_err(|e| e.to_string())?;
        let cells = pack_smbus_block_le(data);
        let input = [
            addr as u64,
            I2C_SMBUS_WRITE,
            cmd as u64,
            I2C_SMBUS_BLOCK_DATA,
            cells[0],
            cells[1],
            cells[2],
            cells[3],
            cells[4],
        ];
        self.execute("ioctl_smbus_xfer", &input, 0)?;
        Ok(())
    }

    pub fn ene_register_read(&self, addr: u8, reg: u16) -> Result<u8, String> {
        self.write_word_data(addr, 0x00, reg.swap_bytes())?;
        self.read_byte_data(addr, 0x81)
    }

    pub fn ene_register_write(&self, addr: u8, reg: u16, val: u8) -> Result<(), String> {
        self.write_word_data(addr, 0x00, reg.swap_bytes())?;
        self.write_byte_data(addr, 0x01, val)
    }

    pub fn ene_register_write_block(&self, addr: u8, reg: u16, data: &[u8]) -> Result<(), String> {
        self.write_word_data(addr, 0x00, reg.swap_bytes())?;
        if self.write_block_data(addr, 0x03, data).is_err() {
            for &b in data {
                self.write_byte_data(addr, 0x01, b)?;
            }
        }
        Ok(())
    }

    /// Safely scans only the ENE DRAM whitelist (`0x70..=0x77`) read-only across PIIX4 ports 0..=1.
    pub fn probe_ene_dimms(&self) -> Vec<DetectedEneDimm> {
        let Ok(_mutex) = GlobalSmbusMutex::acquire(300) else {
            return Vec::new();
        };
        let mut found = Vec::new();
        for port in [0i64, 1i64] {
            let prev_port = self.select_port(port).ok();
            for addr in 0x70u8..=0x77u8 {
                if found.iter().any(|d: &DetectedEneDimm| d.addr == addr) {
                    continue;
                }
                let mut name_bytes = [0u8; 16];
                let mut ok = true;
                for (i, slot) in name_bytes.iter_mut().enumerate() {
                    match self.ene_register_read(addr, ENE_REG_DEVICE_NAME + i as u16) {
                        Ok(b) => *slot = b,
                        Err(_) => {
                            ok = false;
                            break;
                        }
                    }
                }
                if !ok {
                    continue;
                }
                let name: String = name_bytes
                    .iter()
                    .take_while(|&&b| b != 0 && b != 0xFF)
                    .filter(|&&b| b.is_ascii_graphic() || b == b' ')
                    .map(|&b| b as char)
                    .collect();

                // Verify genuine ENE DRAM signature (e.g. "AUDA0-E6K5-0101", "AUMA0-E6K5-0106", "DIMM_LED-0102")
                if name.len() < 4
                    || (!name.contains("AUDA")
                        && !name.contains("AUMA")
                        && !name.contains("DIMM")
                        && !name.contains("ENE")
                        && !name.contains("LED"))
                {
                    continue;
                }

                // Skip Micron controllers if detected at 0x1030
                let mut micron_check = [0u8; 6];
                for (i, slot) in micron_check.iter_mut().enumerate() {
                    *slot = self
                        .ene_register_read(addr, ENE_REG_MICRON_CHECK + i as u16)
                        .unwrap_or(0);
                }
                if &micron_check == b"Micron" {
                    continue;
                }

                let is_v2 = name.contains("AUDA")
                    || name.contains("0101")
                    || name.contains("0106")
                    || name.contains("0107")
                    || name.contains("1110")
                    || name.contains("DIMM");
                let direct_reg = if is_v2 {
                    ENE_REG_COLORS_DIRECT_V2
                } else {
                    ENE_REG_COLORS_DIRECT_V1
                };

                let led_c2 = self
                    .ene_register_read(addr, ENE_REG_CONFIG_TABLE + 2)
                    .unwrap_or(0) as usize;
                let led_c3 = self
                    .ene_register_read(addr, ENE_REG_CONFIG_TABLE + 3)
                    .unwrap_or(0) as usize;
                let led_count = if (1..=16).contains(&led_c2) {
                    led_c2
                } else if (1..=16).contains(&led_c3) {
                    led_c3
                } else {
                    8
                };

                found.push(DetectedEneDimm {
                    addr,
                    port,
                    device_name: name,
                    direct_reg,
                    led_count,
                    direct_initialized: false,
                });
            }
            if let Some(old) = prev_port
                && old >= 0
            {
                let _ = self.select_port(old);
            }
            if !found.is_empty() {
                break;
            }
        }
        found
    }

    pub fn write_dimm_direct(&self, dimm: &mut DetectedEneDimm, leds: &[Rgb]) -> Result<(), String> {
        let _mutex = GlobalSmbusMutex::acquire(250)?;
        let prev_port = self.select_port(dimm.port).ok();

        let res = (|| -> Result<(), String> {
            if !dimm.direct_initialized {
                self.ene_register_write(dimm.addr, ENE_REG_DIRECT, 0x01)?;
                self.ene_register_write(dimm.addr, ENE_REG_APPLY, ENE_APPLY_VAL)?;
                dimm.direct_initialized = true;
            }
            let count = dimm.led_count.min(leds.len()).min(16);
            for (i, c) in leds.iter().take(count).enumerate() {
                let rbg = encode_ene_rbg(*c);
                self.ene_register_write_block(
                    dimm.addr,
                    dimm.direct_reg + (i * 3) as u16,
                    &rbg,
                )?;
            }
            Ok(())
        })();

        if let Some(old) = prev_port
            && old >= 0
        {
            let _ = self.select_port(old);
        }
        res
    }
}

impl Drop for PawnIoSmbusSession {
    fn drop(&mut self) {
        if self.handle != 0 {
            unsafe {
                if let Ok(close_fn) = self.lib.get::<PawnIoCloseFn>(b"pawnio_close\0") {
                    let _ = close_fn(self.handle);
                }
            }
            self.handle = 0;
        }
    }
}

pub struct EneDramIpcDriver {
    direct_session: Option<PawnIoSmbusSession>,
    dimms: Vec<DetectedEneDimm>,
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
            direct_session: None,
            dimms: Vec::new(),
            pawnio_version: None,
            service_connected: false,
            status: "Initializing PawnIO SMBus".into(),
            last_frame: vec![Rgb::BLACK; 16],
        };
        let _ = drv.reconnect();
        drv
    }

    fn query_pawnio_version_only() -> Option<(u32, u32, u32)> {
        if !Path::new(PAWNIO_DLL_PATH).exists() {
            return None;
        }
        unsafe {
            let lib = Library::new(PAWNIO_DLL_PATH).ok()?;
            let func: Symbol<PawnIoVersionFn> = lib.get(b"pawnio_version\0").ok()?;
            let mut ver: u32 = 0;
            if func(&mut ver) >= 0 {
                Some(((ver >> 16) & 0xFF, (ver >> 8) & 0xFF, ver & 0xFF))
            } else {
                None
            }
        }
    }

    fn send_pipe_frame(&mut self, colors: &[Rgb]) -> Result<(), DriverError> {
        let addrs: Vec<u8> = if self.dimms.is_empty() {
            vec![0x71, 0x73]
        } else {
            self.dimms.iter().map(|d| d.addr).collect()
        };
        for &addr in &addrs {
            validate_smbus_address(addr)?;
        }

        let Ok(mut pipe) = OpenOptions::new().write(true).open(SMBUS_PIPE_NAME) else {
            self.service_connected = false;
            return Err(DriverError::ServiceUnavailable(
                "rgbstuff-smbus-svc pipe not running".into(),
            ));
        };

        let mut payload = Vec::with_capacity(4 + colors.len() * 3 + addrs.len() * 2);
        payload.extend_from_slice(b"RG");
        payload.push(1);
        payload.push(addrs.len() as u8);

        let per_dimm = (colors.len() / addrs.len().max(1)).clamp(1, 8);
        for (idx, addr) in addrs.iter().enumerate() {
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
        if self.dimms.is_empty() {
            16
        } else {
            self.dimms.iter().map(|d| d.led_count).sum()
        }
    }

    fn local_led_points(&self) -> Vec<LedPoint> {
        let stick_count = if self.dimms.is_empty() {
            2
        } else {
            self.dimms.len()
        };
        let mut pts = Vec::with_capacity(self.led_count());
        for stick in 0..stick_count {
            let leds = self
                .dimms
                .get(stick)
                .map(|d| d.led_count)
                .unwrap_or(8)
                .max(1);
            let x = if stick_count == 1 {
                0.5
            } else {
                0.25 + (stick as f32 / (stick_count - 1) as f32) * 0.5
            };
            for led in 0..leds {
                let y = if leds == 1 {
                    0.5
                } else {
                    led as f32 / (leds - 1) as f32
                };
                pts.push(LedPoint { x, y });
            }
        }
        pts
    }

    fn is_connected(&self) -> bool {
        !self.dimms.is_empty() || self.service_connected || self.pawnio_version.is_some()
    }

    fn zones(&self) -> Vec<LedZone> {
        if self.dimms.is_empty() {
            return vec![
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
            ];
        }
        self.dimms
            .iter()
            .enumerate()
            .map(|(i, d)| LedZone {
                id: format!("dimm-0x{:02x}", d.addr),
                name: format!("DIMM #{} (0x{:02X} · {})", i + 1, d.addr, d.device_name),
                led_count: d.led_count,
            })
            .collect()
    }

    fn status_note(&self) -> String {
        self.status.clone()
    }

    fn apply_frame(&mut self, colors: &[Rgb]) -> Result<(), DriverError> {
        self.last_frame = colors.to_vec();
        if let Some(session) = &self.direct_session
            && !self.dimms.is_empty()
        {
            let mut offset = 0usize;
            for dimm in &mut self.dimms {
                let n = dimm.led_count.max(1);
                let slice: Vec<Rgb> = (0..n)
                    .map(|i| {
                        colors
                            .get(offset + i)
                            .or_else(|| colors.last())
                            .copied()
                            .unwrap_or(Rgb::BLACK)
                    })
                    .collect();
                offset += n;
                let _ = session.write_dimm_direct(dimm, &slice);
            }
            return Ok(());
        }

        // Fallback to elevated Named Pipe service if direct PawnIO handle was unavailable.
        let _ = self.send_pipe_frame(colors);
        Ok(())
    }

    fn reconnect(&mut self) -> bool {
        self.direct_session = None;
        self.dimms.clear();

        match PawnIoSmbusSession::open() {
            Ok(session) => {
                let ver = session.version;
                self.pawnio_version = Some(ver);
                let dimms = session.probe_ene_dimms();
                if !dimms.is_empty() {
                    let addrs: Vec<String> = dimms
                        .iter()
                        .map(|d| format!("0x{:02X}({}LED)", d.addr, d.led_count))
                        .collect();
                    self.status = format!(
                        "PawnIO {}.{}.{} · Direct ENE [{}]",
                        ver.0,
                        ver.1,
                        ver.2,
                        addrs.join(", ")
                    );
                    self.dimms = dimms;
                    self.direct_session = Some(session);
                    return true;
                }
                self.status = format!(
                    "PawnIO {}.{}.{} loaded · No ENE DRAM in 0x70..0x77",
                    ver.0, ver.1, ver.2
                );
                self.direct_session = Some(session);
                return true;
            }
            Err(_) => {
                self.pawnio_version = Self::query_pawnio_version_only();
            }
        }

        let pipe_ok = OpenOptions::new().write(true).open(SMBUS_PIPE_NAME).is_ok();
        self.service_connected = pipe_ok;
        self.status = match (self.pawnio_version, pipe_ok) {
            (Some((maj, min, pat)), true) => {
                format!("PawnIO {maj}.{min}.{pat} · SMBus IPC active")
            }
            (Some((maj, min, pat)), false) => {
                format!("PawnIO {maj}.{min}.{pat} · Run as Admin or start rgb-smbus-svc")
            }
            (None, _) => "PawnIO driver not found".into(),
        };
        self.pawnio_version.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packs_smbus_block_little_endian() {
        let cells = pack_smbus_block_le(&[0xAA, 0xBB, 0xCC]);
        assert_eq!(cells[0], 0x0000_0000_CCBB_AA03);
        assert_eq!(cells[1], 0);
    }

    #[test]
    fn encodes_ene_rbg_order() {
        assert_eq!(encode_ene_rbg(Rgb::new(10, 20, 30)), [10, 30, 20]);
    }
}
