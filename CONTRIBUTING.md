# Contributing to RGBStuff

Thank you for helping grow **RGBStuff**! Every device driver and subsystem is modular so community hardware support can be added cleanly without bloating the core daemon.

## Development Workflow

1. **Frontend checks** (uses **Bun**):
   ```powershell
   cd apps\desktop
   bun install
   bunx tsc --noEmit
   bun run build
   ```
2. **Rust workspace tests**:
   ```powershell
   cargo test --workspace
   ```
3. **Hardware probing**:
   Use `tools/hid-probe` before writing any new driver:
   ```powershell
   cargo run -p hid-probe -- list --all
   ```

## Adding a New Hardware Driver

1. Add your driver module under `crates/rgb-drivers/src/<vendor_device>.rs`.
2. Implement `rgb_core::RgbDevice` (for lighting) and/or `rgb_core::BatteryDevice` (for wireless telemetry).
3. Keep device handles isolated:
   - Never panic or block the global engine thread if a device is unplugged, asleep, or busy.
   - Use short HID read timeouts (`<= 15ms` for passive polling) and recover cleanly in `reconnect()`.
   - Drain stale HID input reports before reading handshake responses when a controller buffers previous frames.
4. Register the driver in `crates/rgb-drivers/src/manager.rs` (`DeviceManager`) and add a unit test for any binary report/packet parser.

## Mandatory SMBus / I2C Safety Rules

- **Never** bypass `rgb_drivers::validate_smbus_address`.
- **Never** write to SPD EEPROM addresses (`0x50..=0x57`) or unverified I2C devices (e.g., GPUs without verified protocol specs).
- Always acquire `Global\Access_SMBUS.HTP.Method` around SMBus transactions.
- **Never** bundle `WinRing0`; use `PawnIO` (`PawnIOLib.dll`) via `rgb-smbus-svc`.
