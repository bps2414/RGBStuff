import { Channel, invoke } from "@tauri-apps/api/core";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import type { EffectKind, Locale } from "../i18n";

export interface RgbColor {
  r: number;
  g: number;
  b: number;
}

export interface EffectParams {
  kind: EffectKind;
  primary_color: RgbColor;
  secondary_color: RgbColor;
  speed: number;
  angle_deg: number;
  scale: number;
}

export interface LightingProfile {
  id: string;
  name: string;
  brightness: number;
  effect: EffectParams;
}

export interface DevicePlacement {
  device_id: string;
  x: number;
  y: number;
  width: number;
  height: number;
  rotation_deg: number;
  led_count: number;
  /** Per-device gain 0..=200 (above 100 = hue-preserving boost). */
  brightness: number;
}

export interface AppProfileRule {
  id: string;
  executable: string;
  profile_id: string;
  enabled: boolean;
}

export interface AutomationConfig {
  auto_off_on_lock: boolean;
  idle_timeout_mins: number;
  night_mode_enabled: boolean;
  night_start: string;
  night_end: string;
  night_brightness: number;
  low_battery_warn_pct: number;
  low_battery_crit_pct: number;
  start_with_windows: boolean;
  app_rules: AppProfileRule[];
}

export interface ImportedHubItem {
  hub: string;
  device: string;
  preset_name: string;
  summary: string;
}

export interface HubImportReport {
  rapoo_found: boolean;
  aula_found: boolean;
  mchose_found: boolean;
  items: ImportedHubItem[];
  imported_at_unix: number;
}

export interface RunningAppInfo {
  executable: string;
  title: string;
}

export type CloseAction = "tray" | "quit";

export interface UiPrefs {
  close_action: CloseAction;
  tray_hint_shown: boolean;
}

export interface AppConfig {
  schema_version: number;
  locale: Locale;
  power_on: boolean;
  global_brightness: number;
  active_profile_id: string;
  aura_argb_led_counts: [number, number];
  profiles: LightingProfile[];
  placements: DevicePlacement[];
  automation: AutomationConfig;
  imported_hub_presets: ImportedHubItem[];
  ui: UiPrefs;
}

export type EngineStatus =
  | { mode: "off" }
  | { mode: "static_zero_cpu" }
  | { mode: "dynamic"; fps: number };

export interface BatteryTelemetry {
  percent: number | null;
  charging: boolean;
  connected: boolean;
  sleeping: boolean;
  dpi: number | null;
}

export interface LedZone {
  id: string;
  name: string;
  led_count: number;
}

export interface DeviceSnapshot {
  id: string;
  name: string;
  kind: "motherboard_aura" | "ram" | "keyboard" | "mouse" | "headset";
  transport: "usb_hid" | "pawn_io_smbus";
  vid_pid: string;
  connected: boolean;
  status_note: string;
  capabilities: Array<"rgb" | "battery" | "dpi_read" | "peripheral_config">;
  zones: LedZone[];
  telemetry: BatteryTelemetry | null;
}

export interface BootstrapState {
  config: AppConfig;
  engine_status: EngineStatus;
  devices: DeviceSnapshot[];
  canvas_width: number;
  canvas_height: number;
  canvas_pixels: number[];
  active_rule_app: string | null;
  night_mode_active: boolean;
  output_brightness: number;
  app_version: string;
}

export interface HubImportResponse {
  state: BootstrapState;
  report: HubImportReport;
}

declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown;
  }
}

export const isTauriRuntime = (): boolean => Boolean(window.__TAURI_INTERNALS__);

const DEV_PREVIEW_STATE: BootstrapState = {
  config: {
    schema_version: 2,
    locale: "pt-BR",
    power_on: true,
    global_brightness: 100,
    active_profile_id: "work",
    aura_argb_led_counts: [16, 16],
    profiles: [
      {
        id: "work",
        name: "Work",
        brightness: 100,
        effect: {
          kind: "gradient",
          primary_color: { r: 224, g: 242, b: 254 },
          secondary_color: { r: 56, g: 189, b: 248 },
          speed: 30,
          angle_deg: 20,
          scale: 100,
        },
      },
      {
        id: "gaming",
        name: "Gaming",
        brightness: 100,
        effect: {
          kind: "gradient",
          primary_color: { r: 244, g: 63, b: 94 },
          secondary_color: { r: 56, g: 189, b: 248 },
          speed: 35,
          angle_deg: 35,
          scale: 100,
        },
      },
      {
        id: "ambilight",
        name: "Ambilight",
        brightness: 100,
        effect: {
          kind: "ambilight",
          primary_color: { r: 56, g: 189, b: 248 },
          secondary_color: { r: 245, g: 158, b: 11 },
          speed: 55,
          angle_deg: 0,
          scale: 100,
        },
      },
      {
        id: "audio",
        name: "Audio",
        brightness: 100,
        effect: {
          kind: "audio_spectrum",
          primary_color: { r: 34, g: 197, b: 94 },
          secondary_color: { r: 14, g: 165, b: 233 },
          speed: 75,
          angle_deg: 0,
          scale: 100,
        },
      },
      {
        id: "night",
        name: "Night",
        brightness: 25,
        effect: {
          kind: "solid",
          primary_color: { r: 255, g: 138, b: 61 },
          secondary_color: { r: 120, g: 53, b: 15 },
          speed: 20,
          angle_deg: 0,
          scale: 100,
        },
      },
      {
        id: "off",
        name: "Off",
        brightness: 0,
        effect: {
          kind: "off",
          primary_color: { r: 0, g: 0, b: 0 },
          secondary_color: { r: 0, g: 0, b: 0 },
          speed: 1,
          angle_deg: 0,
          scale: 100,
        },
      },
    ],
    placements: [
      { device_id: "asus-aura-0b05-19af", x: 0.08, y: 0.14, width: 0.28, height: 0.36, rotation_deg: 0, led_count: 32, brightness: 100 },
      { device_id: "asgard-ddr4-smbus", x: 0.42, y: 0.16, width: 0.12, height: 0.32, rotation_deg: 0, led_count: 16, brightness: 100 },
      { device_id: "aula-f75-he", x: 0.16, y: 0.58, width: 0.68, height: 0.28, rotation_deg: 0, led_count: 80, brightness: 100 },
    ],
    automation: {
      auto_off_on_lock: true,
      idle_timeout_mins: 15,
      night_mode_enabled: false,
      night_start: "23:00",
      night_end: "07:00",
      night_brightness: 25,
      low_battery_warn_pct: 20,
      low_battery_crit_pct: 10,
      start_with_windows: false,
      app_rules: [
        { id: "rule-vlc", executable: "vlc.exe", profile_id: "ambilight", enabled: true },
        { id: "rule-cs2", executable: "cs2.exe", profile_id: "gaming", enabled: true },
      ],
    },
    imported_hub_presets: [],
    ui: { close_action: "tray", tray_hint_shown: false },
  },
  engine_status: { mode: "static_zero_cpu" },
  active_rule_app: null,
  night_mode_active: false,
  output_brightness: 100,
  app_version: "dev",
  devices: [
    {
      id: "asus-aura-0b05-19af",
      name: "ASUS Aura Controller (Fans ARGB)",
      kind: "motherboard_aura",
      transport: "usb_hid",
      vid_pid: "0B05:19AF",
      connected: true,
      status_note: "FW AULA3-AR32-0220 · USB HID Direct",
      capabilities: ["rgb"],
      zones: [
        { id: "argb-1", name: "ARGB Header 1", led_count: 16 },
        { id: "argb-2", name: "ARGB Header 2", led_count: 16 },
      ],
      telemetry: null,
    },
    {
      id: "asgard-ddr4-smbus",
      name: "ENE DRAM RGB",
      kind: "ram",
      transport: "pawn_io_smbus",
      vid_pid: "1022:790B",
      connected: true,
      status_note: "PawnIO 2.0.0 ready",
      capabilities: ["rgb"],
      zones: [
        { id: "dimm-1", name: "DIMM 1", led_count: 8 },
        { id: "dimm-2", name: "DIMM 2", led_count: 8 },
      ],
      telemetry: null,
    },
    {
      id: "aula-f75-he",
      name: "Aula F75 HE",
      kind: "keyboard",
      transport: "usb_hid",
      vid_pid: "38A6:2729",
      connected: true,
      status_note: "HID iface=2 (0xFF68)",
      capabilities: ["rgb"],
      zones: [{ id: "per-key-75", name: "75% Key Matrix", led_count: 80 }],
      telemetry: null,
    },
    {
      id: "rapoo-vt-24ae-14a1",
      name: "Rapoo Wireless Mouse",
      kind: "mouse",
      transport: "usb_hid",
      vid_pid: "24AE:14A1",
      connected: true,
      status_note: "HID iface=2 (0xFF00)",
      capabilities: ["battery", "dpi_read", "peripheral_config"],
      zones: [],
      telemetry: { percent: 84, charging: false, connected: true, sleeping: false, dpi: 1600 },
    },
    {
      id: "mchose-v9-pro-2",
      name: "MCHOSE V9 Pro 2",
      kind: "headset",
      transport: "usb_hid",
      vid_pid: "3837:6011",
      connected: true,
      status_note: "HID iface=3 (0xFF07)",
      capabilities: ["battery", "peripheral_config"],
      zones: [],
      telemetry: { percent: 72, charging: false, connected: true, sleeping: false, dpi: null },
    },
  ],
  canvas_width: 128,
  canvas_height: 72,
  canvas_pixels: new Array(128 * 72 * 3).fill(24),
};

let devState = structuredClone(DEV_PREVIEW_STATE);

export async function apiGetBootstrapState(): Promise<BootstrapState> {
  if (!isTauriRuntime()) return structuredClone(devState);
  return invoke<BootstrapState>("get_bootstrap_state");
}

export async function apiSetPower(on: boolean): Promise<BootstrapState> {
  if (!isTauriRuntime()) {
    devState.config.power_on = on;
    devState.engine_status = on ? { mode: "static_zero_cpu" } : { mode: "off" };
    return structuredClone(devState);
  }
  return invoke<BootstrapState>("set_power", { on });
}

export async function apiSetBrightness(brightness: number): Promise<BootstrapState> {
  if (!isTauriRuntime()) {
    devState.config.global_brightness = brightness;
    return structuredClone(devState);
  }
  return invoke<BootstrapState>("set_brightness", { brightness });
}

export async function apiSelectProfile(profileId: string): Promise<BootstrapState> {
  if (!isTauriRuntime()) {
    devState.config.active_profile_id = profileId;
    devState.config.power_on = profileId !== "off";
    return structuredClone(devState);
  }
  return invoke<BootstrapState>("select_profile", { profileId });
}

export async function apiUpdateEffect(
  effect: EffectParams,
  profileBrightness?: number,
): Promise<BootstrapState> {
  if (!isTauriRuntime()) {
    const p = devState.config.profiles.find((x) => x.id === devState.config.active_profile_id);
    if (p) {
      p.effect = effect;
      if (profileBrightness !== undefined) p.brightness = profileBrightness;
    }
    devState.config.power_on = true;
    devState.engine_status = ["off", "solid", "gradient"].includes(effect.kind)
      ? effect.kind === "off" ? { mode: "off" } : { mode: "static_zero_cpu" }
      : { mode: "dynamic", fps: 60 };
    return structuredClone(devState);
  }
  return invoke<BootstrapState>("update_effect", {
    effect,
    profileBrightness: profileBrightness ?? null,
  });
}

export async function apiUpdatePlacement(placement: DevicePlacement): Promise<BootstrapState> {
  if (!isTauriRuntime()) {
    const idx = devState.config.placements.findIndex((p) => p.device_id === placement.device_id);
    if (idx >= 0) devState.config.placements[idx] = placement;
    return structuredClone(devState);
  }
  return invoke<BootstrapState>("update_placement", { placement });
}

export async function apiUpdateAuraLedCounts(counts: [number, number]): Promise<BootstrapState> {
  if (!isTauriRuntime()) {
    devState.config.aura_argb_led_counts = counts;
    return structuredClone(devState);
  }
  return invoke<BootstrapState>("update_aura_led_counts", { counts });
}

export async function apiUpdateUiPrefs(prefs: UiPrefs): Promise<BootstrapState> {
  if (!isTauriRuntime()) {
    devState.config.ui = prefs;
    return structuredClone(devState);
  }
  return invoke<BootstrapState>("update_ui_prefs", { prefs });
}

/** State pushed by the backend when something outside this window changed it. */
export async function onStateChanged(fn: (state: BootstrapState) => void): Promise<() => void> {
  if (!isTauriRuntime()) return () => {};
  return getCurrentWebviewWindow().listen<BootstrapState>("rgb://state", (e) => fn(e.payload));
}

export async function apiQuitApp(): Promise<void> {
  if (!isTauriRuntime()) return;
  await invoke("quit_app_command");
}

export async function apiSetLocale(locale: Locale): Promise<BootstrapState> {
  if (!isTauriRuntime()) {
    devState.config.locale = locale;
    return structuredClone(devState);
  }
  return invoke<BootstrapState>("set_locale", { locale });
}

export async function apiUpdateAutomation(automation: AutomationConfig): Promise<BootstrapState> {
  if (!isTauriRuntime()) {
    devState.config.automation = automation;
    return structuredClone(devState);
  }
  return invoke<BootstrapState>("update_automation", { automation });
}

export async function apiRefreshDevices(): Promise<BootstrapState> {
  if (!isTauriRuntime()) return structuredClone(devState);
  return invoke<BootstrapState>("refresh_devices");
}

export async function apiListRunningApps(): Promise<RunningAppInfo[]> {
  if (!isTauriRuntime()) {
    return [
      { executable: "feathertune.exe", title: "feathertune" },
      { executable: "cs2.exe", title: "Counter-Strike 2" },
    ];
  }
  return invoke<RunningAppInfo[]>("list_running_apps");
}

export async function apiImportHubPresets(): Promise<HubImportResponse> {
  if (!isTauriRuntime()) {
    const sample = [
      {
        hub: "A HUB (Rapoo)",
        device: "VT0Air MAX",
        preset_name: "Factory Config",
        summary: "Polling 1000Hz · SensorMode #5",
      },
      {
        hub: "Aula Hub",
        device: "Aula F75 HE",
        preset_name: "Device Profile (paleClouds)",
        summary: "FW V1.55 · Theme paleClouds",
      },
      {
        hub: "MCHOSE HUB",
        device: "MCHOSE V9 Pro 2",
        preset_name: "Default EQ / Factory",
        summary: "MCHOSE HUB installed · Factory EQ",
      },
    ];
    devState.config.imported_hub_presets = sample;
    return {
      state: structuredClone(devState),
      report: {
        rapoo_found: true,
        aula_found: true,
        mchose_found: true,
        items: sample,
        imported_at_unix: Math.floor(Date.now() / 1000),
      },
    };
  }
  return invoke<HubImportResponse>("import_hub_presets");
}

/** Browser-only stand-in for the Rust engine so the UI can be previewed without hardware. */
function startDevPreview(onFrame: (pixels: Uint8Array) => void) {
  const w = devState.canvas_width;
  const h = devState.canvas_height;
  let last = 0;
  const frame = (ms: number) => {
    requestAnimationFrame(frame);
    if (ms - last < 33) return;
    last = ms;
    const cfg = devState.config;
    const e = cfg.profiles.find((p) => p.id === cfg.active_profile_id)?.effect;
    const buf = new Uint8Array(w * h * 3);
    const t = ms / 1000;
    if (cfg.power_on && e && e.kind !== "off") {
      const a = e.primary_color;
      const b = e.secondary_color;
      for (let y = 0; y < h; y++) {
        for (let x = 0; x < w; x++) {
          let r: number, g: number, bl: number;
          if (e.kind === "rainbow_wave") {
            const hue = ((x / w) * 360 + t * 60) % 360;
            const f = (n: number) => {
              const k = (n + hue / 60) % 6;
              return 255 * (1 - Math.max(0, Math.min(k, 4 - k, 1)));
            };
            [r, g, bl] = [f(5), f(3), f(1)];
          } else {
            const k =
              e.kind === "solid"
                ? 0
                : e.kind === "gradient"
                  ? x / w
                  : 0.5 + 0.5 * Math.sin(x * 0.06 + t * 0.9 + Math.sin(y * 0.08 + t * 0.6) * 1.8);
            const v = e.kind === "solid" || e.kind === "gradient" ? 1 : 0.6 + 0.4 * Math.sin(y * 0.05 - t * 0.8 + x * 0.02);
            r = (a.r + (b.r - a.r) * k) * v;
            g = (a.g + (b.g - a.g) * k) * v;
            bl = (a.b + (b.b - a.b) * k) * v;
          }
          const i = (y * w + x) * 3;
          buf[i] = r;
          buf[i + 1] = g;
          buf[i + 2] = bl;
        }
      }
    }
    onFrame(buf);
  };
  requestAnimationFrame(frame);
}

export async function apiSubscribeCanvasPreview(onFrame: (pixels: Uint8Array) => void): Promise<void> {
  if (!isTauriRuntime()) {
    startDevPreview(onFrame);
    return;
  }
  // Backend sends raw RGB888 bytes (ArrayBuffer), not a JSON number array.
  const channel = new Channel<ArrayBuffer | number[]>();
  channel.onmessage = (msg) => onFrame(Array.isArray(msg) ? Uint8Array.from(msg) : new Uint8Array(msg));
  await invoke("subscribe_canvas_preview", { channel });
}

export async function apiOpenStudio(): Promise<void> {
  if (!isTauriRuntime()) return;
  await invoke("open_studio");
}

export async function apiWindowMinimize(): Promise<void> {
  if (!isTauriRuntime()) return;
  await invoke("window_minimize");
}

export async function apiWindowToggleMaximize(): Promise<void> {
  if (!isTauriRuntime()) return;
  await invoke("window_toggle_maximize");
}

export async function apiWindowClose(): Promise<void> {
  if (!isTauriRuntime()) return;
  await invoke("window_close");
}

export function rgbToHex(c: RgbColor): string {
  const h = (n: number) => Math.max(0, Math.min(255, n)).toString(16).padStart(2, "0");
  return `#${h(c.r)}${h(c.g)}${h(c.b)}`.toUpperCase();
}

export function hexToRgb(hex: string): RgbColor {
  const clean = hex.replace("#", "");
  if (clean.length !== 6) return { r: 56, g: 189, b: 248 };
  return {
    r: parseInt(clean.slice(0, 2), 16) || 0,
    g: parseInt(clean.slice(2, 4), 16) || 0,
    b: parseInt(clean.slice(4, 6), 16) || 0,
  };
}
