use libloading::{Library, Symbol};
use rgb_core::Rgb;
use rgb_drivers::validate_smbus_address;
use std::ffi::{CStr, c_char};
use std::path::{Path, PathBuf};
use windows_sys::Win32::Foundation::{
    CloseHandle, GetLastError, HANDLE, WAIT_ABANDONED, WAIT_OBJECT_0,
};
use windows_sys::Win32::System::Threading::{CreateMutexW, ReleaseMutex, WaitForSingleObject};

pub const PAWNIO_DLL_PATH: &str = r"C:\Program Files\PawnIO\PawnIOLib.dll";
pub const PAWNIO_SMBUS_MODULE_PATH: &str = r"C:\Program Files\PawnIO\modules\SmbusPIIX4.bin";
pub const GLOBAL_SMBUS_MUTEX_NAME: &str = r"Global\Access_SMBUS.HTP.Method";
pub const SMBUS_MUTEX_TIMEOUT_MS: u32 = 250;

const I2C_SMBUS_WRITE: u64 = 0;
const I2C_SMBUS_BYTE_DATA: u64 = 2;
const I2C_SMBUS_WORD_DATA: u64 = 3;

const ENE_REG_ADDR_CMD: u8 = 0x00;
const ENE_REG_WRITE_BYTE_CMD: u8 = 0x01;
const ENE_REG_MODE_DIRECT: u16 = 0x8020;
const ENE_REG_EFFECT_MODE: u16 = 0x8021;
const ENE_REG_APPLY: u16 = 0x80A0;
const ENE_REG_DIRECT_COLORS_BASE: u16 = 0x8100;

/// RAII guard for the system-wide SMBus hardware mutex (`Global\Access_SMBUS.HTP.Method`).
pub struct GlobalSmbusMutex {
    handle: HANDLE,
}

impl GlobalSmbusMutex {
    pub fn acquire() -> Result<Self, String> {
        let wide_name: Vec<u16> = GLOBAL_SMBUS_MUTEX_NAME
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();

        let handle = unsafe { CreateMutexW(std::ptr::null(), 0, wide_name.as_ptr()) };
        if handle.is_null() {
            let err = unsafe { GetLastError() };
            return Err(format!(
                "CreateMutexW({GLOBAL_SMBUS_MUTEX_NAME}) failed with Win32 error {err}"
            ));
        }

        let wait = unsafe { WaitForSingleObject(handle, SMBUS_MUTEX_TIMEOUT_MS) };
        if wait != WAIT_OBJECT_0 && wait != WAIT_ABANDONED {
            unsafe {
                CloseHandle(handle);
            }
            return Err(format!(
                "Timed out or failed waiting for {GLOBAL_SMBUS_MUTEX_NAME} (status={wait})"
            ));
        }

        Ok(Self { handle })
    }
}

impl Drop for GlobalSmbusMutex {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe {
                ReleaseMutex(self.handle);
                CloseHandle(self.handle);
            }
            self.handle = std::ptr::null_mut();
        }
    }
}

type PawnIoVersionFn = unsafe extern "system" fn(*mut u32) -> i32;
type PawnIoOpenFn = unsafe extern "system" fn(*mut HANDLE) -> i32;
type PawnIoLoadFn = unsafe extern "system" fn(HANDLE, *const u8, usize) -> i32;
type PawnIoExecuteFn = unsafe extern "system" fn(
    HANDLE,
    *const c_char,
    *const u64,
    usize,
    *mut u64,
    usize,
    *mut usize,
) -> i32;
type PawnIoCloseFn = unsafe extern "system" fn(HANDLE) -> i32;

/// Thin dynamic wrapper over `PawnIOLib.dll`.
pub struct PawnIoBridge {
    handle: Option<HANDLE>,
    lib: Library,
}

impl PawnIoBridge {
    pub fn load_default() -> Result<Self, String> {
        Self::load_from_path(Path::new(PAWNIO_DLL_PATH))
    }

    pub fn load_from_path(path: &Path) -> Result<Self, String> {
        if !path.exists() {
            return Err(format!("PawnIOLib.dll not found at {}", path.display()));
        }
        let lib = unsafe { Library::new(path) }
            .map_err(|e| format!("Failed to load {}: {e}", path.display()))?;
        Ok(Self { handle: None, lib })
    }

    pub fn version(&self) -> Result<(u32, u32, u32), String> {
        unsafe {
            let func: Symbol<'_, PawnIoVersionFn> = self
                .lib
                .get(b"pawnio_version\0")
                .map_err(|e| format!("Missing symbol pawnio_version: {e}"))?;
            let mut ver: u32 = 0;
            let hr = func(&mut ver);
            if hr < 0 {
                return Err(format!("pawnio_version failed with HRESULT 0x{hr:08X}"));
            }
            let major = (ver >> 16) & 0xFF;
            let minor = (ver >> 8) & 0xFF;
            let patch = ver & 0xFF;
            Ok((major, minor, patch))
        }
    }

    pub fn open(&mut self) -> Result<(), String> {
        if self.handle.is_some() {
            return Ok(());
        }
        unsafe {
            let func: Symbol<'_, PawnIoOpenFn> = self
                .lib
                .get(b"pawnio_open\0")
                .map_err(|e| format!("Missing symbol pawnio_open: {e}"))?;
            let mut handle: HANDLE = std::ptr::null_mut();
            let hr = func(&mut handle);
            if hr < 0 || handle.is_null() {
                return Err(format!("pawnio_open failed with HRESULT 0x{hr:08X}"));
            }
            self.handle = Some(handle);
            Ok(())
        }
    }

    pub fn load_blob(&self, blob: &[u8]) -> Result<(), String> {
        let handle = self
            .handle
            .ok_or_else(|| "PawnIO executor handle is not open".to_string())?;
        unsafe {
            let func: Symbol<'_, PawnIoLoadFn> = self
                .lib
                .get(b"pawnio_load\0")
                .map_err(|e| format!("Missing symbol pawnio_load: {e}"))?;
            let hr = func(handle, blob.as_ptr(), blob.len());
            if hr < 0 {
                return Err(format!("pawnio_load failed with HRESULT 0x{hr:08X}"));
            }
            Ok(())
        }
    }

    pub fn execute(
        &self,
        name: &CStr,
        input: &[u64],
        output: &mut [u64],
    ) -> Result<usize, String> {
        let handle = self
            .handle
            .ok_or_else(|| "PawnIO executor handle is not open".to_string())?;
        unsafe {
            let func: Symbol<'_, PawnIoExecuteFn> = self
                .lib
                .get(b"pawnio_execute\0")
                .map_err(|e| format!("Missing symbol pawnio_execute: {e}"))?;
            let mut returned: usize = 0;
            let in_ptr = if input.is_empty() {
                std::ptr::null()
            } else {
                input.as_ptr()
            };
            let out_ptr = if output.is_empty() {
                std::ptr::null_mut()
            } else {
                output.as_mut_ptr()
            };
            let hr = func(
                handle,
                name.as_ptr(),
                in_ptr,
                input.len(),
                out_ptr,
                output.len(),
                &mut returned,
            );
            if hr < 0 {
                return Err(format!(
                    "pawnio_execute({}) failed with HRESULT 0x{hr:08X}",
                    name.to_string_lossy()
                ));
            }
            Ok(returned)
        }
    }

    pub fn close(&mut self) {
        if let Some(handle) = self.handle.take() {
            unsafe {
                if let Ok(func) = self.lib.get::<PawnIoCloseFn>(b"pawnio_close\0") {
                    let _ = func(handle);
                }
            }
        }
    }

    pub fn is_open(&self) -> bool {
        self.handle.is_some()
    }
}

impl Drop for PawnIoBridge {
    fn drop(&mut self) {
        self.close();
    }
}

/// Safe ENE DRAM RGB SMBus writer backed by PawnIO and `GlobalSmbusMutex`.
pub struct EneDramSmbusWriter {
    bridge: Option<PawnIoBridge>,
    module_loaded: bool,
}

impl Default for EneDramSmbusWriter {
    fn default() -> Self {
        Self::new()
    }
}

impl EneDramSmbusWriter {
    pub fn new() -> Self {
        let mut writer = Self {
            bridge: None,
            module_loaded: false,
        };
        let _ = writer.try_initialize();
        writer
    }

    pub fn is_ready(&self) -> bool {
        self.bridge
            .as_ref()
            .is_some_and(|b| b.is_open() && self.module_loaded)
    }

    pub fn find_smbus_module_path() -> Option<PathBuf> {
        let system_path = Path::new(PAWNIO_SMBUS_MODULE_PATH);
        if system_path.exists() {
            return Some(system_path.to_path_buf());
        }
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                let local = dir.join("SmbusPIIX4.bin");
                if local.exists() {
                    return Some(local);
                }
            }
        }
        None
    }

    pub fn try_initialize(&mut self) -> Result<(), String> {
        if self.bridge.is_none() {
            self.bridge = Some(PawnIoBridge::load_default()?);
        }
        let bridge = self
            .bridge
            .as_mut()
            .ok_or_else(|| "PawnIO bridge unavailable".to_string())?;
        if !bridge.is_open() {
            bridge.open()?;
        }
        if !self.module_loaded {
            if let Some(module_path) = Self::find_smbus_module_path() {
                let blob = std::fs::read(&module_path).map_err(|e| {
                    format!("Failed to read {}: {e}", module_path.display())
                })?;
                bridge.load_blob(&blob)?;
                self.module_loaded = true;
            }
        }
        Ok(())
    }

    /// Writes RGB colors to validated ENE DRAM addresses (`0x70..=0x77`).
    /// Rejects any SPD EEPROM (`0x50..=0x57`) or non-whitelisted address before touching hardware.
    pub fn apply_dimm_colors(&mut self, dimms: &[(u8, Vec<Rgb>)]) -> Result<(), String> {
        for (addr, _) in dimms {
            validate_smbus_address(*addr).map_err(|e| e.to_string())?;
        }

        if dimms.is_empty() {
            return Ok(());
        }

        self.try_initialize()?;
        if !self.module_loaded {
            return Err(
                "SmbusPIIX4.bin module not found in PawnIO\\modules or beside executable"
                    .to_string(),
            );
        }

        let _mutex = GlobalSmbusMutex::acquire()?;
        let bridge = self
            .bridge
            .as_ref()
            .ok_or_else(|| "PawnIO bridge not initialized".to_string())?;

        for (addr, leds) in dimms {
            let safe_addr = validate_smbus_address(*addr).map_err(|e| e.to_string())?;
            write_ene_dimm_direct(bridge, safe_addr, leds)?;
        }

        Ok(())
    }
}

fn smbus_write_byte_data(
    bridge: &PawnIoBridge,
    addr: u8,
    command: u8,
    value: u8,
) -> Result<(), String> {
    let input = [
        addr as u64,
        I2C_SMBUS_WRITE,
        command as u64,
        I2C_SMBUS_BYTE_DATA,
        value as u64,
    ];
    let mut out = [0u64; 1];
    bridge.execute(c"ioctl_smbus_xfer", &input, &mut out)?;
    Ok(())
}

fn smbus_write_word_data(
    bridge: &PawnIoBridge,
    addr: u8,
    command: u8,
    value: u16,
) -> Result<(), String> {
    let input = [
        addr as u64,
        I2C_SMBUS_WRITE,
        command as u64,
        I2C_SMBUS_WORD_DATA,
        value as u64,
    ];
    let mut out = [0u64; 1];
    bridge.execute(c"ioctl_smbus_xfer", &input, &mut out)?;
    Ok(())
}

fn write_ene_register(bridge: &PawnIoBridge, addr: u8, reg: u16, value: u8) -> Result<(), String> {
    // ENE SMBus interface expects big-endian register address written as a 16-bit word on cmd 0x00.
    let swapped = reg.swap_bytes();
    smbus_write_word_data(bridge, addr, ENE_REG_ADDR_CMD, swapped)?;
    smbus_write_byte_data(bridge, addr, ENE_REG_WRITE_BYTE_CMD, value)
}

fn write_ene_dimm_direct(bridge: &PawnIoBridge, addr: u8, leds: &[Rgb]) -> Result<(), String> {
    write_ene_register(bridge, addr, ENE_REG_MODE_DIRECT, 0x01)?;
    write_ene_register(bridge, addr, ENE_REG_EFFECT_MODE, 0x01)?;

    for (idx, color) in leds.iter().take(16).enumerate() {
        let base = ENE_REG_DIRECT_COLORS_BASE + (idx as u16) * 3;
        write_ene_register(bridge, addr, base, color.r)?;
        write_ene_register(bridge, addr, base + 1, color.g)?;
        write_ene_register(bridge, addr, base + 2, color.b)?;
    }

    write_ene_register(bridge, addr, ENE_REG_APPLY, 0x01)
}
