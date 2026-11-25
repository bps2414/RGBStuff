use hidapi::{HidApi, HidDevice};
use libloading::{Library, Symbol};
use rgb_core::{Rgb, RgbDevice};
use rgb_drivers::aula_f75_he::{
    AULA_PID, AULA_USAGE_PAGE_PRIMARY, AULA_VID, AulaF75HeDriver, HFD_CMD_GET_DEVICE_INFO,
    HFD_CMD_GET_LED_EFFECT, build_hfd_packet,
};
use rgb_drivers::ene_dram_ipc::PawnIoSmbusSession;
use std::env;
use std::process;

const TARGET_VIDS: &[(u16, &str)] = &[
    (0x0B05, "ASUS Aura"),
    (0x38A6, "Aula F75 HE"),
    (0x24AE, "Rapoo Mouse"),
    (0x3837, "MCHOSE V9 Pro 2"),
];

const AURA_VID: u16 = 0x0B05;
const AURA_PID: u16 = 0x19AF;

fn print_usage() {
    eprintln!(
        "Usage: hid-probe <command>
Commands:
  list [--all]          List target HID devices (or all HID devices with --all)
  aura-info             Query ASUS Aura 0B05:19AF firmware & config table (read-only)
  aura-color <RRGGBB>   Send safe Direct Mode color test to 0B05:19AF
  ram-scan              Safe read-only PawnIO SmbusPIIX4 scan across ENE whitelist 0x70..=0x77
  ram-color <RRGGBB>    Test ENE DRAM direct color write on detected Asgard RAM sticks
  aula-info             Query Aula F75 HE (38A6:2729) HFD device info & current LED effect
  aula-color <RRGGBB>   Test Aula F75 HE HFD color write
  pawnio-check          Dynamically load PawnIOLib.dll and check version + open handle
  probe-peripherals     Read-only HID interface check for Aula, Rapoo & MCHOSE"
    );
}

fn format_hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn cmd_list(show_all: bool) -> Result<(), String> {
    let api = HidApi::new().map_err(|e| format!("HidApi::new failed: {e}"))?;
    let mut count = 0usize;
    for info in api.device_list() {
        let vid = info.vendor_id();
        let pid = info.product_id();
        let tag = TARGET_VIDS.iter().find(|(v, _)| *v == vid).map(|(_, n)| *n);
        if !show_all && tag.is_none() {
            continue;
        }
        count += 1;
        println!(
            "[{:04X}:{:04X}] ({}) iface={} usage_page=0x{:04X} usage=0x{:04X} mfg={:?} prod={:?}",
            vid,
            pid,
            tag.unwrap_or("Other"),
            info.interface_number(),
            info.usage_page(),
            info.usage(),
            info.manufacturer_string().unwrap_or(""),
            info.product_string().unwrap_or(""),
        );
        println!("   path={}", info.path().to_string_lossy());
    }
    println!("Total listed: {count}");
    Ok(())
}

fn open_aura_device(api: &HidApi) -> Result<HidDevice, String> {
    let candidate = api
        .device_list()
        .find(|d| d.vendor_id() == AURA_VID && d.product_id() == AURA_PID && d.usage_page() == 0xFF72)
        .or_else(|| {
            api.device_list()
                .find(|d| d.vendor_id() == AURA_VID && d.product_id() == AURA_PID && d.usage_page() >= 0xFF00)
        })
        .ok_or_else(|| "ASUS Aura 0B05:19AF not found".to_string())?;

    println!(
        "Opening Aura 0B05:19AF iface={} usage_page=0x{:04X} usage=0x{:04X}",
        candidate.interface_number(),
        candidate.usage_page(),
        candidate.usage()
    );
    candidate
        .open_device(api)
        .map_err(|e| format!("failed to open 0B05:19AF: {e}"))
}

fn drain_stale(dev: &HidDevice) {
    let mut buf = [0u8; 65];
    for _ in 0..8 {
        match dev.read_timeout(&mut buf, 10) {
            Ok(n) if n > 0 => continue,
            _ => break,
        }
    }
}

fn aura_query(dev: &HidDevice, cmd: u8) -> Result<[u8; 65], String> {
    drain_stale(dev);
    let mut out = [0u8; 65];
    out[0] = 0xEC;
    out[1] = cmd;
    dev.write(&out)
        .map_err(|e| format!("HID write 0xEC 0x{cmd:02X} failed: {e}"))?;
    let mut inp = [0u8; 65];
    let n = dev
        .read_timeout(&mut inp, 500)
        .map_err(|e| format!("HID read 0xEC 0x{cmd:02X} failed: {e}"))?;
    if n == 0 {
        return Err(format!("HID read timeout for 0xEC 0x{cmd:02X}"));
    }
    Ok(inp)
}

fn cmd_aura_info() -> Result<(), String> {
    let api = HidApi::new().map_err(|e| format!("HidApi::new failed: {e}"))?;
    let dev = open_aura_device(&api)?;

    let fw = aura_query(&dev, 0x82)?;
    let fw_str: String = fw[2..18]
        .iter()
        .take_while(|&&b| b != 0)
        .map(|&b| if b.is_ascii_graphic() || b == b' ' { b as char } else { '.' })
        .collect();
    println!("Firmware reply: {}", format_hex(&fw[..32]));
    println!("Firmware string: {fw_str}");

    let cfg = aura_query(&dev, 0xB0)?;
    println!("Config table (65B): {}", format_hex(&cfg[..48]));
    Ok(())
}

fn cmd_aura_color(hex: &str) -> Result<(), String> {
    let rgb = Rgb::from_hex(hex).map_err(|e| e.to_string())?;
    let api = HidApi::new().map_err(|e| format!("HidApi::new failed: {e}"))?;
    let dev = open_aura_device(&api)?;
    drain_stale(&dev);

    for ch in 0u8..2u8 {
        let mut mode_pkt = [0u8; 65];
        mode_pkt[0] = 0xEC;
        mode_pkt[1] = 0x35;
        mode_pkt[2] = ch;
        mode_pkt[5] = 0xFF;
        let _ = dev.write(&mode_pkt);
    }

    for ch in 0u8..2u8 {
        let mut pkt = [0u8; 65];
        pkt[0] = 0xEC;
        pkt[1] = 0x40;
        pkt[2] = 0x80 | ch;
        pkt[3] = 0x00;
        pkt[4] = 20;
        for i in 0..20 {
            let base = 5 + i * 3;
            pkt[base] = rgb.r;
            pkt[base + 1] = rgb.g;
            pkt[base + 2] = rgb.b;
        }
        dev.write(&pkt)
            .map_err(|e| format!("Direct write ch={ch} failed: {e}"))?;
    }

    println!("Sent Direct Mode color {} to ASUS Aura 0B05:19AF", rgb.to_hex());
    Ok(())
}

fn cmd_ram_scan() -> Result<(), String> {
    let session = PawnIoSmbusSession::open()?;
    println!(
        "PawnIO SmbusPIIX4 loaded! DLL version {}.{}.{}",
        session.version.0, session.version.1, session.version.2
    );
    match session.identity() {
        Ok((id, base, pci)) => println!(
            "SMBus Identity: id=0x{id:X} io_base=0x{base:X} pci_ids=0x{pci:016X}"
        ),
        Err(e) => println!("ioctl_identity warning: {e}"),
    }

    let dimms = session.probe_ene_dimms();
    if dimms.is_empty() {
        println!("No ENE DRAM modules detected in whitelist 0x70..=0x77 (ports 0..1).");
        // Print raw first 4 bytes at 0x1000 for 0x70..=0x77 on port 0 to aid diagnosis
        println!("Raw read-only probe of 0x70..=0x77 @ reg 0x1000..0x1004:");
        for addr in 0x70u8..=0x77u8 {
            let mut vals = Vec::new();
            for off in 0u16..4u16 {
                match session.ene_register_read(addr, 0x1000 + off) {
                    Ok(b) => vals.push(format!("{b:02X}")),
                    Err(_) => vals.push("--".into()),
                }
            }
            println!("  0x{addr:02X}: {}", vals.join(" "));
        }
    } else {
        println!("Detected {} ENE DRAM module(s):", dimms.len());
        for d in &dimms {
            println!(
                "  port={} addr=0x{:02X} name={:?} led_count={}",
                d.port, d.addr, d.device_name, d.led_count
            );
        }
    }
    Ok(())
}

fn cmd_ram_color(hex: &str) -> Result<(), String> {
    let rgb = Rgb::from_hex(hex).map_err(|e| e.to_string())?;
    let session = PawnIoSmbusSession::open()?;
    let mut dimms = session.probe_ene_dimms();
    if dimms.is_empty() {
        return Err("No ENE DRAM modules found in 0x70..=0x77".into());
    }
    for dimm in &mut dimms {
        let frame = vec![rgb; dimm.led_count];
        session.write_dimm_direct(dimm, &frame)?;
        println!(
            "Applied {} to ENE DRAM 0x{:02X} ({}, {} LEDs)",
            rgb.to_hex(),
            dimm.addr,
            dimm.device_name,
            dimm.led_count
        );
    }
    Ok(())
}

fn cmd_aula_info() -> Result<(), String> {
    let api = HidApi::new().map_err(|e| format!("HidApi::new failed: {e}"))?;
    let info = api
        .device_list()
        .find(|d| {
            d.vendor_id() == AULA_VID
                && d.product_id() == AULA_PID
                && d.usage_page() == AULA_USAGE_PAGE_PRIMARY
        })
        .ok_or_else(|| "Aula F75 HE (38A6:2729, 0xFF68) not found".to_string())?;
    let dev = info
        .open_device(&api)
        .map_err(|e| format!("open Aula failed: {e}"))?;
    drain_stale(&dev);

    for report_len in [64usize, 32usize] {
        println!("Testing HFD report_len={report_len}...");
        let pkt_info = build_hfd_packet(report_len, HFD_CMD_GET_DEVICE_INFO, 0, &[], 0, true);
        if let Ok(written) = dev.write(&pkt_info) {
            let mut resp = [0u8; 65];
            match dev.read_timeout(&mut resp, 200) {
                Ok(n) if n > 0 => {
                    println!(
                        "  GET_DEVICE_INFO (wrote {written}B, read {n}B): {}",
                        format_hex(&resp[..n.min(32)])
                    );
                }
                Ok(_) => println!("  GET_DEVICE_INFO timeout (wrote {written}B)"),
                Err(e) => println!("  GET_DEVICE_INFO read err: {e}"),
            }
        } else {
            println!("  write({}B) rejected by HID driver", report_len + 1);
            continue;
        }

        let pkt_eff = build_hfd_packet(report_len, HFD_CMD_GET_LED_EFFECT, 0, &[], 0, true);
        if let Ok(written) = dev.write(&pkt_eff) {
            let mut resp = [0u8; 65];
            match dev.read_timeout(&mut resp, 200) {
                Ok(n) if n > 0 => {
                    println!(
                        "  GET_LED_EFFECT (wrote {written}B, read {n}B): {}",
                        format_hex(&resp[..n.min(28)])
                    );
                    let off = if resp[0] == 0x55 { 0 } else { 1 };
                    if n >= off + 20 {
                        println!(
                            "  Parsed -> mode={} rgb=({},{},{}) colorMode={} brightness={} speed={} dir={}",
                            resp[off + 8],
                            resp[off + 9],
                            resp[off + 10],
                            resp[off + 11],
                            resp[off + 16],
                            resp[off + 17],
                            resp[off + 18],
                            resp[off + 19],
                        );
                    }
                    return Ok(());
                }
                Ok(_) => println!("  GET_LED_EFFECT timeout (wrote {written}B)"),
                Err(e) => println!("  GET_LED_EFFECT read err: {e}"),
            }
        }
    }
    Ok(())
}

fn cmd_aula_color(hex: &str) -> Result<(), String> {
    let rgb = Rgb::from_hex(hex).map_err(|e| e.to_string())?;
    let mut drv = AulaF75HeDriver::new();
    println!("Aula driver status: {}", drv.status_note());
    drv.apply_frame(&vec![rgb; 80])
        .map_err(|e| e.to_string())?;
    println!("Sent color {} to Aula F75 HE!", rgb.to_hex());
    Ok(())
}

fn cmd_pawnio_check() -> Result<(), String> {
    let dll_path = r"C:\Program Files\PawnIO\PawnIOLib.dll";
    unsafe {
        let lib = Library::new(dll_path).map_err(|e| format!("Failed to load {dll_path}: {e}"))?;

        type PawnIoVersionFn = unsafe extern "system" fn(*mut u32) -> i32;
        type PawnIoOpenFn = unsafe extern "system" fn(*mut isize) -> i32;
        type PawnIoCloseFn = unsafe extern "system" fn(isize) -> i32;

        let version_fn: Symbol<PawnIoVersionFn> = lib
            .get(b"pawnio_version\0")
            .map_err(|e| format!("missing pawnio_version: {e}"))?;
        let open_fn: Symbol<PawnIoOpenFn> = lib
            .get(b"pawnio_open\0")
            .map_err(|e| format!("missing pawnio_open: {e}"))?;
        let close_fn: Symbol<PawnIoCloseFn> = lib
            .get(b"pawnio_close\0")
            .map_err(|e| format!("missing pawnio_close: {e}"))?;

        let mut ver: u32 = 0;
        let hr = version_fn(&mut ver);
        if hr < 0 {
            return Err(format!("pawnio_version returned HRESULT 0x{hr:08X}"));
        }
        let major = (ver >> 16) & 0xFF;
        let minor = (ver >> 8) & 0xFF;
        let patch = ver & 0xFF;
        println!("PawnIOLib.dll loaded! Version: {major}.{minor}.{patch} (raw=0x{ver:08X})");

        let mut handle: isize = 0;
        let open_hr = open_fn(&mut handle);
        if open_hr >= 0 && handle != 0 {
            println!("pawnio_open succeeded (handle=0x{handle:X}) -> driver access available in current context");
            let _ = close_fn(handle);
        } else {
            println!("pawnio_open returned HRESULT 0x{open_hr:08X}");
        }
    }
    Ok(())
}

fn cmd_probe_peripherals() -> Result<(), String> {
    let api = HidApi::new().map_err(|e| format!("HidApi::new failed: {e}"))?;
    let targets = [
        (0x38A6, 0x2729, "Aula F75 HE"),
        (0x24AE, 0x14A1, "Rapoo Mouse"),
        (0x3837, 0x6011, "MCHOSE V9 Pro 2"),
    ];

    for (vid, pid, label) in targets {
        println!("=== {label} ({vid:04X}:{pid:04X}) ===");
        let matching: Vec<_> = api
            .device_list()
            .filter(|d| d.vendor_id() == vid && d.product_id() == pid)
            .collect();
        if matching.is_empty() {
            println!("  Not connected");
            continue;
        }
        for info in matching {
            let can_open = info.open_device(&api).is_ok();
            println!(
                "  iface={} usage_page=0x{:04X} usage=0x{:04X} open_ok={}",
                info.interface_number(),
                info.usage_page(),
                info.usage(),
                can_open
            );
        }
    }
    Ok(())
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let res = match args.get(1).map(|s| s.as_str()) {
        Some("list") => cmd_list(args.iter().any(|a| a == "--all")),
        Some("aura-info") => cmd_aura_info(),
        Some("aura-color") => match args.get(2) {
            Some(hex) => cmd_aura_color(hex),
            None => Err("missing <RRGGBB> argument".to_string()),
        },
        Some("ram-scan") => cmd_ram_scan(),
        Some("ram-color") => match args.get(2) {
            Some(hex) => cmd_ram_color(hex),
            None => Err("missing <RRGGBB> argument".to_string()),
        },
        Some("aula-info") => cmd_aula_info(),
        Some("aula-color") => match args.get(2) {
            Some(hex) => cmd_aula_color(hex),
            None => Err("missing <RRGGBB> argument".to_string()),
        },
        Some("pawnio-check") => cmd_pawnio_check(),
        Some("probe-peripherals") => cmd_probe_peripherals(),
        _ => {
            print_usage();
            process::exit(1);
        }
    };

    if let Err(err) = res {
        eprintln!("Error: {err}");
        process::exit(1);
    }
}
