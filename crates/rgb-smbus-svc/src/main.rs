pub mod pawnio;
pub mod pipe_server;

use crate::pawnio::{EneDramSmbusWriter, GlobalSmbusMutex, PawnIoBridge};
use crate::pipe_server::run_pipe_server;
use std::process;

fn run_check() -> i32 {
    let mut ok = true;

    match PawnIoBridge::load_default() {
        Ok(mut bridge) => match bridge.version() {
            Ok((maj, min, pat)) => {
                println!("PawnIOLib.dll: OK (version {maj}.{min}.{pat})");
                match bridge.open() {
                    Ok(()) => println!("pawnio_open: OK"),
                    Err(err) => println!("pawnio_open: unavailable in current context ({err})"),
                }
            }
            Err(err) => {
                eprintln!("PawnIOLib.dll version check failed: {err}");
                ok = false;
            }
        },
        Err(err) => {
            eprintln!("PawnIOLib.dll check failed: {err}");
            ok = false;
        }
    }

    match GlobalSmbusMutex::acquire() {
        Ok(_guard) => println!("Global SMBus Mutex: OK"),
        Err(err) => {
            eprintln!("Global SMBus Mutex check failed: {err}");
            ok = false;
        }
    }

    if let Some(path) = EneDramSmbusWriter::find_smbus_module_path() {
        println!("SmbusPIIX4.bin: found at {}", path.display());
    } else {
        println!("SmbusPIIX4.bin: not present (optional until module is installed)");
    }

    if ok { 0 } else { 1 }
}

fn run_foreground_server() -> i32 {
    let mut writer = EneDramSmbusWriter::new();
    match run_pipe_server(&mut writer) {
        Ok(()) => 0,
        Err(err) => {
            eprintln!("rgb-smbus-svc error: {err}");
            1
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = match args.first().map(String::as_str) {
        Some("--check") => run_check(),
        Some("--console") | None => run_foreground_server(),
        Some(other) => {
            eprintln!("Unknown argument: {other}");
            eprintln!("Usage: rgb-smbus-svc [--check | --console]");
            1
        }
    };
    process::exit(code);
}
