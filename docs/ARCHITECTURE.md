# RGBStuff Architecture

## 1. Design Goals

1. **Zero idle WebView2 memory:** Chromium/WebView2 processes should only exist while a window (`tray-popover` or `studio`) is visibly open.
2. **Zero idle CPU for static lighting:** Static profiles (`Off`, `Solid`, `Gradient` — including the default `Gaming` profile) must apply once and sleep until configuration or focus state changes.
3. **Strict privilege separation:** Screen capture (`IDXGIOutputDuplication`), audio loopback (`WASAPI`), and UI run unprivileged in the user session (Session 1+); only SMBus RAM writes run inside an optional elevated service (`rgb-smbus-svc`).
4. **Hardware & bus isolation:** Each device driver operates independently so disconnecting a wireless dongle never stalls motherboard or keyboard updates.

---

## 2. Crate Responsibilities

| Crate | Purpose |
| :--- | :--- |
| `rgb-core` | Pure domain types (`Rgb`, `AppConfig`, `LightingProfile`, `DevicePlacement`, `AutomationConfig`), atomic TOML persistence (`config.toml.tmp` -> rename), and read-only `hub_importer`. |
| `rgb-engine` | `Canvas2D` (`128×72` RGB888), bilinear coordinate sampling, pure-Rust effect renderers, `BusFramePacer` per-device rate limiter, `DirtyFrameTracker`, and `LightingEngine`. |
| `rgb-drivers` | Isolated `RgbDevice` & `BatteryDevice` implementations (`AsusAuraUsbDriver`, `AulaF75HeDriver`, `RapooHidDriver`, `MchoseV9Driver`, `EneDramIpcDriver`), `DeviceManager`, and `smbus_safety::validate_smbus_address`. |
| `rgb-capture` | Demand-driven `ScreenSampler` (DXGI Desktop Duplication downsampling BGRA -> `64×36` RGB) and `AudioSpectrumSampler` (WASAPI loopback + `rustfft` 16-band logarithmic spectrum). |
| `rgb-automation` | Message-only Win32 window (`HWND_MESSAGE`) listening for `WM_WTSSESSION_CHANGE` (`Win+L`), `WM_POWERBROADCAST` (sleep/resume), `GetLastInputInfo` (idle), `GetForegroundWindow` (per-app rules), `EnumWindows` (`list_running_windowed_apps`), and local night schedule. |
| `rgb-smbus-svc` | Optional Windows service/console binary loading `PawnIOLib.dll`, acquiring `Global\Access_SMBUS.HTP.Method`, validating addresses against `0x70..=0x77`, and serving `\\.\pipe\rgbstuff-smbus-v1`. |
| `tools/hid-probe` | Standalone CLI diagnostic tool for safe HID enumeration, ASUS Aura `0xEC 0x82`/`0xEC 0xB0` queries, and PawnIO DLL checks. |

---

## 3. Window Lifecycle & IPC

- **Startup:** `tauri.conf.json` defines `"windows": []`. The app registers a `TrayIconBuilder` and spawns two Tokio tasks: `spawn_engine_loop` and `spawn_automation_loop`.
- **Tray Popover (`tray-popover`):** Built with `.visible(false)` -> `window_vibrancy::apply_mica(&win, Some(true))` -> `move_window(Position::TrayCenter)` -> `.show()` to avoid white-flash artifacts. On `WindowEvent::Focused(false)`, calls `win.destroy()`.
- **Studio (`studio`):** Built with custom frameless Mica titlebar. On `WindowEvent::CloseRequested`, prevents default hide and calls `win.destroy()` so WebView2 processes terminate immediately while the Rust tray daemon keeps running.
- **Live Canvas Preview:** Uses a Tauri v2 binary `Channel<Vec<u8>>` (`subscribe_canvas_preview`) only while Studio is open; when Studio closes and `ch.send(...)` fails, `preview_channel` is automatically cleared (`None`).

---

## 4. Named Pipe Protocol (`rgb-smbus-svc`)

Pipe path: `\\.\pipe\rgbstuff-smbus-v1` (inbound message-mode pipe).

Binary packet format (`"RG"` v1):

| Offset | Size | Field | Description |
| :--- | :--- | :--- | :--- |
| `0..2` | 2 bytes | Magic | ASCII `"RG"` (`0x52, 0x47`) |
| `2` | 1 byte | Version | `1` |
| `3` | 1 byte | `dimm_count` | Number of DIMM blocks (`1..=8`) |
| `4..` | repeated | DIMM block | `addr (u8)`, `led_count (u8)`, followed by `led_count * 3` RGB bytes |

Every `addr` byte is checked by `rgb_drivers::validate_smbus_address(addr)` before any PawnIO call. Any address in `0x50..=0x57` (SPD EEPROM) or outside `0x70..=0x77` aborts the entire packet immediately.

---

## 5. Demand-Driven Capture & Focus Rules

`CoreRuntime::sync_capture_providers()` inspects `config.power_on` and the active profile's `EffectKind`:
- `ScreenSampler::start()` runs **only** when `power_on && effect == Ambilight`; otherwise `ScreenSampler::stop()` joins the thread and releases all DXGI/D3D11 COM interfaces.
- `AudioSpectrumSampler::start()` runs **only** when `power_on && effect == AudioSpectrum`; otherwise `AudioSpectrumSampler::stop()` shuts down WASAPI capture.

`AutomationWatcher` filters out transient system/UI executables via `is_ignored_transient_exe()` (including `rgbstuff-desktop.exe`, `searchhost.exe`, `shellexperiencehost.exe`, `lockapp.exe`) so interacting with RGBStuff or the Start menu does not cancel an active rule like `feathertune.exe → ambilight`.
