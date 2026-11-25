# Hardware Probe Report (Captured via `hid-probe`)

Descriptors captured on the test machine. They document the protocols the drivers speak; your device may expose different interfaces.

## 1. ASUS Aura USB Controller (`0B05:19AF`)
- **Interface**: `iface=2`, `usage_page=0xFF72`, `usage=0x00A1`
- **Manufacturer / Product**: `AsusTek Computer Inc.` / `AURA LED Controller`
- **Firmware Query (`0xEC 0x82`)**:
  - Reply: `EC 02 41 55 4C 41 33 2D 41 52 33 32 2D 30 32 32 30 ...`
  - Firmware String: **`AULA3-AR32-0220`**
- **Config Table (`0xEC 0xB0`)**:
  - Reply: `EC 30 00 00 1E 9F 02 01 00 00 78 3C 00 01 00 00 78 3C ...`
  - Addressable Gen2 Headers: **`2`** (max `0x78` = 120 LEDs per header)
  - Onboard LEDs: **`0`**

## 2. PawnIO Kernel Bridge (`Asgard DDR4-3200` via AMD SMBus `1022:790B`)
- **DLL**: `C:\Program Files\PawnIO\PawnIOLib.dll`
- **Loaded Version**: `2.0.0` (`0x00020000`)
- **Signed Module**: `SmbusPIIX4.bin` (`namazso/PawnIO.Modules` v0.2.12, embedded in `crates/rgb-drivers/assets/SmbusPIIX4.bin`)
- **SMBus Identity**: `PIIX4`, `io_base=0x0B00`, `pci_ids=0x87E11043790B1022`
- **Detected ENE DRAM**: Address **`0x77`**, Device Name prefix **`AUDA`** (Gen 2 ENE DRAM -> `ENE_REG_COLORS_DIRECT_V2 = 0x8100`, `[R, B, G]` byte order)
- **Safety Guard**: Whitelist `0x70..=0x77` enforced in `rgb_drivers::smbus_safety`; SPD range `0x50..=0x57` strictly blocked; guarded by `Global\Access_SMBUS.HTP.Method`.

## 3. Peripherals (USB HID)

### Aula F75 HE (`38A6:2729`, `AULA F75 HE`)
- `iface=2`, `usage_page=0xFF68`, `usage=0x0061` (`open_ok=true`) — Primary HFD SDK RGB interface (`report_len=64`, Windows write buffer `65B` with Report ID `0x00`, magic header `0xAA`, response `0x55`)
- Verified `GET_LED_EFFECT (0x13)`: `brightness` scale `0..=5`, `SET_LED_EFFECT (0x23)` for static/off (`mode=1`/`mode=0`), `GET_LED_DATA (0x32)` for live 512B per-key RAM streaming.
- `iface=3`, `usage_page=0xFF67`, `usage=0x0061` (`open_ok=true`) — Secondary vendor interface

### Rapoo Wireless Mouse (`24AE:14A1`, `Rapoo Gaming Device`)
- `iface=2`, `usage_page=0xFF00`, `usage=0x0001` (`open_ok=true`) — Primary WebHID/A HUB vendor interface
- `iface=1`, `usage_page=0xFF00`, `usage=0x0002, 0x000E..0x0012` (`open_ok=true`) — Vendor telemetry/config collections
- **RGB**: None (telemetry/DPI/polling config only)

### MCHOSE V9 Pro 2 (`3837:6011`, `bestechnic` / `MCHOSE V9 Pro 2`)
- `iface=3`, `usage_page=0xFF07`, `usage=0x0222` (`open_ok=true`) — Vendor HID interface (`Col03`)
- `iface=3`, `usage_page=0x000B`, `usage=0x0005` (`open_ok=true`) — Telephony/headset controls (`Col02`)
- **RGB**: None (battery/status/EQ config only)
