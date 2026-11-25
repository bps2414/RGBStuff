use crate::pawnio::EneDramSmbusWriter;
use rgb_core::Rgb;
use rgb_drivers::validate_smbus_address;
use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_BROKEN_PIPE, ERROR_MORE_DATA, ERROR_PIPE_CONNECTED, GetLastError, HANDLE,
    INVALID_HANDLE_VALUE,
};
use windows_sys::Win32::Storage::FileSystem::{PIPE_ACCESS_INBOUND, ReadFile};
use windows_sys::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, PIPE_READMODE_MESSAGE,
    PIPE_TYPE_MESSAGE, PIPE_UNLIMITED_INSTANCES, PIPE_WAIT,
};

pub const SMBUS_PIPE_NAME: &str = r"\\.\pipe\rgbstuff-smbus-v1";
const PIPE_MAGIC: [u8; 2] = *b"RG";
const PIPE_PROTOCOL_VERSION: u8 = 1;
const MAX_DIMMS_PER_PACKET: usize = 8;
const MAX_LEDS_PER_DIMM: usize = 32;

struct PipeHandle(HANDLE);

impl Drop for PipeHandle {
    fn drop(&mut self) {
        if !self.0.is_null() && self.0 != INVALID_HANDLE_VALUE {
            unsafe {
                CloseHandle(self.0);
            }
            self.0 = INVALID_HANDLE_VALUE;
        }
    }
}

/// Parses a binary RGB pipe frame: `[b'R', b'G', version=1, dimm_count, (addr, led_count, rgb...)...]`.
/// Validates every SMBus target address through `rgb_drivers::validate_smbus_address`.
pub fn parse_rgb_pipe_packet(bytes: &[u8]) -> Result<Vec<(u8, Vec<Rgb>)>, String> {
    if bytes.len() < 4 {
        return Err(format!("packet too short: {} bytes", bytes.len()));
    }
    if bytes[0..2] != PIPE_MAGIC {
        return Err("invalid magic header (expected 'RG')".to_string());
    }
    if bytes[2] != PIPE_PROTOCOL_VERSION {
        return Err(format!("unsupported protocol version {}", bytes[2]));
    }

    let dimm_count = bytes[3] as usize;
    if dimm_count > MAX_DIMMS_PER_PACKET {
        return Err(format!("dimm_count {dimm_count} exceeds limit {MAX_DIMMS_PER_PACKET}"));
    }

    let mut cursor = 4usize;
    let mut dimms = Vec::with_capacity(dimm_count);

    for _ in 0..dimm_count {
        if cursor + 2 > bytes.len() {
            return Err("truncated DIMM header".to_string());
        }
        let addr = bytes[cursor];
        let led_count = bytes[cursor + 1] as usize;
        cursor += 2;

        let safe_addr = validate_smbus_address(addr).map_err(|e| e.to_string())?;
        if led_count > MAX_LEDS_PER_DIMM {
            return Err(format!("led_count {led_count} exceeds limit {MAX_LEDS_PER_DIMM}"));
        }

        let rgb_bytes = led_count
            .checked_mul(3)
            .ok_or_else(|| "LED byte count overflow".to_string())?;
        let end = cursor
            .checked_add(rgb_bytes)
            .ok_or_else(|| "packet cursor overflow".to_string())?;
        if end > bytes.len() {
            return Err("truncated LED payload".to_string());
        }

        let mut leds = Vec::with_capacity(led_count);
        for chunk in bytes[cursor..end].chunks_exact(3) {
            leds.push(Rgb::new(chunk[0], chunk[1], chunk[2]));
        }
        cursor = end;
        dimms.push((safe_addr, leds));
    }

    if cursor != bytes.len() {
        return Err(format!(
            "unexpected trailing bytes: expected {cursor}, got {}",
            bytes.len()
        ));
    }

    Ok(dimms)
}

fn read_pipe_message(pipe: HANDLE) -> Result<Option<Vec<u8>>, String> {
    let mut message = Vec::new();
    let mut buf = [0u8; 1024];

    loop {
        let mut bytes_read: u32 = 0;
        let ok = unsafe {
            ReadFile(
                pipe,
                buf.as_mut_ptr(),
                buf.len() as u32,
                &mut bytes_read,
                std::ptr::null_mut(),
            )
        };

        if bytes_read > 0 {
            message.extend_from_slice(&buf[..bytes_read as usize]);
        }

        if ok != 0 {
            if message.is_empty() {
                return Ok(None);
            }
            return Ok(Some(message));
        }

        let err = unsafe { GetLastError() };
        if err == ERROR_MORE_DATA {
            continue;
        }
        if err == ERROR_BROKEN_PIPE {
            return Ok(None);
        }
        return Err(format!("ReadFile failed with Win32 error {err}"));
    }
}

/// Runs the blocking Named Pipe server on `\\.\pipe\rgbstuff-smbus-v1`.
pub fn run_pipe_server(writer: &mut EneDramSmbusWriter) -> Result<(), String> {
    let wide_name: Vec<u16> = SMBUS_PIPE_NAME
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();

    let raw_handle = unsafe {
        CreateNamedPipeW(
            wide_name.as_ptr(),
            PIPE_ACCESS_INBOUND,
            PIPE_TYPE_MESSAGE | PIPE_READMODE_MESSAGE | PIPE_WAIT,
            PIPE_UNLIMITED_INSTANCES,
            4096,
            4096,
            0,
            std::ptr::null(),
        )
    };

    if raw_handle == INVALID_HANDLE_VALUE || raw_handle.is_null() {
        let err = unsafe { GetLastError() };
        return Err(format!(
            "CreateNamedPipeW({SMBUS_PIPE_NAME}) failed with Win32 error {err}"
        ));
    }

    let pipe = PipeHandle(raw_handle);
    eprintln!("Listening on {SMBUS_PIPE_NAME}");

    loop {
        let connected = unsafe { ConnectNamedPipe(pipe.0, std::ptr::null_mut()) };
        if connected == 0 {
            let err = unsafe { GetLastError() };
            if err != ERROR_PIPE_CONNECTED {
                return Err(format!("ConnectNamedPipe failed with Win32 error {err}"));
            }
        }

        loop {
            match read_pipe_message(pipe.0) {
                Ok(Some(packet)) => match parse_rgb_pipe_packet(&packet) {
                    Ok(dimms) => {
                        if let Err(err) = writer.apply_dimm_colors(&dimms) {
                            eprintln!("SMBus write skipped/failed: {err}");
                        }
                    }
                    Err(err) => {
                        eprintln!("Rejected invalid RGB pipe packet: {err}");
                    }
                },
                Ok(None) => break,
                Err(err) => {
                    eprintln!("Pipe read error: {err}");
                    break;
                }
            }
        }

        unsafe {
            DisconnectNamedPipe(pipe.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_rg_v1_packet() {
        let packet = vec![
            b'R', b'G', 1, 2, // magic, version, 2 DIMMs
            0x71, 2, // DIMM 1 at 0x71, 2 LEDs
            255, 0, 128, // LED 0
            10, 20, 30, // LED 1
            0x73, 1, // DIMM 2 at 0x73, 1 LED
            0, 255, 64, // LED 0
        ];

        let parsed = parse_rgb_pipe_packet(&packet).expect("valid RG v1 packet");
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].0, 0x71);
        assert_eq!(
            parsed[0].1,
            vec![Rgb::new(255, 0, 128), Rgb::new(10, 20, 30)]
        );
        assert_eq!(parsed[1].0, 0x73);
        assert_eq!(parsed[1].1, vec![Rgb::new(0, 255, 64)]);
    }

    #[test]
    fn rejects_spd_eeprom_addresses() {
        for spd_addr in [0x50u8, 0x52u8] {
            let packet = vec![
                b'R', b'G', 1, 1, // header
                spd_addr, 1, // forbidden SPD address
                255, 255, 255, // RGB
            ];
            let err = parse_rgb_pipe_packet(&packet).expect_err("SPD address must be rejected");
            assert!(
                err.contains("SPD") || err.contains("0x50") || err.contains("0x52"),
                "unexpected error message: {err}"
            );
        }
    }

    #[test]
    fn rejects_out_of_range_and_malformed_packets() {
        let out_of_range = vec![b'R', b'G', 1, 1, 0x78, 1, 1, 2, 3];
        assert!(parse_rgb_pipe_packet(&out_of_range).is_err());

        let bad_magic = vec![b'X', b'Y', 1, 0];
        assert!(parse_rgb_pipe_packet(&bad_magic).is_err());

        let bad_version = vec![b'R', b'G', 2, 0];
        assert!(parse_rgb_pipe_packet(&bad_version).is_err());

        let truncated = vec![b'R', b'G', 1, 1, 0x71, 2, 255, 0, 0];
        assert!(parse_rgb_pipe_packet(&truncated).is_err());
    }
}
