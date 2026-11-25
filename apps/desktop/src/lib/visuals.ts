import type { EffectKind, Locale } from "../i18n";
import { rgbToHex, type EffectParams, type RgbColor } from "./tauri";

/** Sets the UI accent (`--live`) to the color the LEDs are showing. */
export function setLiveAccent(rgb: [number, number, number] | null) {
  const [r, g, b] = rgb ?? [161, 161, 170];
  document.documentElement.style.setProperty("--live", `${r} ${g} ${b}`);
}

export function setLiveAccentFromColor(c: RgbColor | null) {
  setLiveAccent(c ? normalize(c) : null);
}

function normalize(c: RgbColor): [number, number, number] {
  const max = Math.max(c.r, c.g, c.b);
  if (max < 8) return [161, 161, 170];
  const k = 255 / max;
  return [Math.round(c.r * k), Math.round(c.g * k), Math.round(c.b * k)];
}

export interface EffectMeta {
  isStatic: boolean;
  primary: boolean;
  secondary: boolean;
  speed: boolean;
  angle: boolean;
  scale: boolean;
}

const m = (isStatic: boolean, p: string): EffectMeta => ({
  isStatic,
  primary: p.includes("1"),
  secondary: p.includes("2"),
  speed: p.includes("s"),
  angle: p.includes("a"),
  scale: p.includes("z"),
});

/** Which controls actually change each effect (mirrors `rgb-engine/src/effects.rs`). */
export const EFFECT_META: Record<EffectKind, EffectMeta> = {
  solid: m(true, "1"),
  gradient: m(true, "12a"),
  rainbow_wave: m(false, "saz"),
  breathing: m(false, "12s"),
  dual_pulse: m(false, "12saz"),
  starlight: m(false, "12sz"),
  aurora: m(false, "12sz"),
  plasma: m(false, "12sz"),
  ripple: m(false, "12sz"),
  ambilight: m(false, "sz"),
  audio_spectrum: m(false, "12"),
  off: m(true, ""),
};

export const EFFECT_ORDER: EffectKind[] = [
  "solid",
  "gradient",
  "rainbow_wave",
  "breathing",
  "dual_pulse",
  "starlight",
  "aurora",
  "plasma",
  "ripple",
  "ambilight",
  "audio_spectrum",
  "off",
];

/** CSS background that sketches what an effect looks like with these params. */
export function effectBackground(e: Pick<EffectParams, "kind" | "primary_color" | "secondary_color" | "angle_deg">): string {
  const a = rgbToHex(e.primary_color);
  const b = rgbToHex(e.secondary_color);
  const deg = Math.round(e.angle_deg + 90);
  switch (e.kind) {
    case "off":
      return "repeating-linear-gradient(135deg, #18181b 0 6px, #111113 6px 12px)";
    case "solid":
      return a;
    case "gradient":
      return `linear-gradient(${deg}deg, ${a}, ${b})`;
    case "rainbow_wave":
      return `linear-gradient(${deg}deg, #ff3b3b, #ffb800, #3bff6b, #22d3ee, #6366f1, #e93bff, #ff3b3b)`;
    case "breathing":
      return `radial-gradient(circle at 50% 50%, ${a} 0%, ${b}cc 55%, #0a0a0c 100%)`;
    case "dual_pulse":
      return `repeating-linear-gradient(${deg}deg, ${a} 0 18%, ${b} 18% 36%)`;
    case "starlight":
      return `radial-gradient(circle at 20% 30%, #fff 0 2%, transparent 3%), radial-gradient(circle at 70% 60%, ${a} 0 3%, transparent 5%), radial-gradient(circle at 45% 80%, ${b} 0 2%, transparent 4%), radial-gradient(circle at 85% 20%, #fff 0 1.5%, transparent 3%), linear-gradient(160deg, #0b1020, ${b}55)`;
    case "aurora":
      return `radial-gradient(120% 60% at 30% 110%, ${a} 0%, transparent 60%), radial-gradient(90% 60% at 80% 0%, ${b} 0%, transparent 65%), linear-gradient(180deg, #050712, #0b1023)`;
    case "plasma":
      return `radial-gradient(circle at 25% 35%, ${a} 0%, transparent 45%), radial-gradient(circle at 75% 65%, ${b} 0%, transparent 50%), radial-gradient(circle at 70% 20%, ${a}aa 0%, transparent 35%), #0c0a12`;
    case "ripple":
      return `repeating-radial-gradient(circle at 50% 50%, ${a} 0 6%, ${b}66 9%, transparent 14%), #07080c`;
    case "ambilight":
      return "linear-gradient(90deg, #f97316 0 20%, #0ea5e9 20% 45%, #22c55e 45% 70%, #a855f7 70%)";
    case "audio_spectrum":
      return `linear-gradient(0deg, ${a} 0 30%, transparent 30%) 0 0 / 14% 100% repeat-x, linear-gradient(0deg, ${b} 0 65%, transparent 65%) 7% 0 / 14% 100% repeat-x, #08080b`;
  }
}

export interface Scene {
  id: string;
  name: Record<Locale, string>;
  effect: EffectParams;
}

const c = (hex: string): RgbColor => ({
  r: parseInt(hex.slice(1, 3), 16),
  g: parseInt(hex.slice(3, 5), 16),
  b: parseInt(hex.slice(5, 7), 16),
});

const scene = (
  id: string,
  pt: string,
  en: string,
  kind: EffectKind,
  a: string,
  b: string,
  speed = 45,
  angle_deg = 20,
  scale = 100,
): Scene => ({
  id,
  name: { "pt-BR": pt, "en-US": en },
  effect: { kind, primary_color: c(a), secondary_color: c(b), speed, angle_deg, scale },
});

/** One-click looks: a full effect (kind + colors + motion) applied to the active profile. */
export const SCENES: Scene[] = [
  scene("ocean", "Oceano", "Ocean", "aurora", "#00E5FF", "#0047FF", 35, 0, 120),
  scene("synthwave", "Synthwave", "Synthwave", "plasma", "#FF2BD6", "#2B6BFF", 40, 0, 110),
  scene("lava", "Lava", "Lava", "plasma", "#FF3D00", "#FFB300", 30, 0, 90),
  scene("forest", "Floresta", "Forest", "aurora", "#00FF85", "#B6FF00", 25, 0, 130),
  scene("galaxy", "Galáxia", "Galaxy", "starlight", "#8B5CF6", "#22D3EE", 40, 0, 100),
  scene("rainbow", "Arco-íris", "Rainbow", "rainbow_wave", "#FFFFFF", "#FFFFFF", 50, 20, 100),
  scene("sunset", "Pôr do sol", "Sunset", "gradient", "#FF2D55", "#FFB800", 30, 15, 100),
  scene("ice", "Gelo", "Ice", "gradient", "#FFFFFF", "#00B7FF", 30, 25, 100),
  scene("rain", "Chuva", "Rain", "ripple", "#60A5FA", "#A855F7", 45, 0, 100),
  scene("neon", "Neon", "Neon", "dual_pulse", "#FF0080", "#00FFD1", 55, 30, 100),
  scene("calm", "Respirar", "Breathe", "breathing", "#38BDF8", "#6366F1", 25, 0, 100),
  scene("white", "Branco total", "Full white", "solid", "#FFFFFF", "#FFFFFF", 30, 0, 100),
];

const same = (x: RgbColor, y: RgbColor) => x.r === y.r && x.g === y.g && x.b === y.b;

export function sceneMatches(s: Scene, e: EffectParams): boolean {
  return s.effect.kind === e.kind && same(s.effect.primary_color, e.primary_color) && same(s.effect.secondary_color, e.secondary_color);
}
