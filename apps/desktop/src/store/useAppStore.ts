import { create } from "zustand";
import { getDictionary, type Locale, type TranslationDictionary } from "../i18n";
import {
  apiGetBootstrapState,
  apiImportHubPresets,
  apiRefreshDevices,
  apiSelectProfile,
  apiSetBrightness,
  apiSetLocale,
  apiSetPower,
  apiSubscribeCanvasPreview,
  apiUpdateAuraLedCounts,
  apiUpdateAutomation,
  apiUpdateEffect,
  apiUpdateKeyboardFx,
  apiUpdatePlacement,
  apiUpdateUiPrefs,
  onStateChanged,
  type AutomationConfig,
  type BootstrapState,
  type DevicePlacement,
  type EffectParams,
  type HubImportReport,
  type KeyboardHwEffect,
  type UiPrefs,
} from "../lib/tauri";
import { setLiveAccent } from "../lib/visuals";

// Preview frames stay out of React state: storing them re-rendered every
// subscriber of the store at preview rate. The canvas draws them directly.
let latestPixels: Uint8Array | null = null;
const pixelListeners = new Set<(pixels: Uint8Array) => void>();
let lastAccentAt = 0;

function publishPixels(pixels: Uint8Array) {
  latestPixels = pixels;
  for (const fn of pixelListeners) fn(pixels);
  // The UI accent follows what the LEDs show, a few times per second.
  const now = performance.now();
  if (now - lastAccentAt > 180) {
    lastAccentAt = now;
    setLiveAccent(dominantColor(pixels));
  }
}

/**
 * Saturation-weighted average, normalized to full value: a rainbow gives its
 * strongest hue instead of mud grey, and dark frames don't dim the accent.
 */
function dominantColor(px: Uint8Array): [number, number, number] | null {
  let r = 0;
  let g = 0;
  let b = 0;
  let w = 0;
  for (let i = 0; i + 2 < px.length; i += 3 * 7) {
    const pr = px[i];
    const pg = px[i + 1];
    const pb = px[i + 2];
    const max = Math.max(pr, pg, pb);
    if (max < 12) continue;
    const min = Math.min(pr, pg, pb);
    const weight = (max / 255) * (0.25 + (max - min) / max);
    r += pr * weight;
    g += pg * weight;
    b += pb * weight;
    w += weight;
  }
  if (w <= 0) return null;
  r /= w;
  g /= w;
  b /= w;
  const k = 255 / Math.max(r, g, b, 1);
  return [Math.round(r * k), Math.round(g * k), Math.round(b * k)];
}

export function subscribeCanvasPixels(fn: (pixels: Uint8Array) => void): () => void {
  pixelListeners.add(fn);
  if (latestPixels) fn(latestPixels);
  return () => {
    pixelListeners.delete(fn);
  };
}

/**
 * At most one request in flight; while it runs only the newest argument is kept.
 * Intermediate responses are dropped so they can't snap sliders/drags back.
 */
function latestWins<A>(send: (arg: A) => Promise<BootstrapState>, apply: (s: BootstrapState) => void) {
  let inFlight = false;
  let pending: { arg: A } | null = null;
  return async (arg: A) => {
    if (inFlight) {
      pending = { arg };
      return;
    }
    inFlight = true;
    try {
      let next = await send(arg);
      while (pending) {
        const p: { arg: A } = pending;
        pending = null;
        next = await send(p.arg);
      }
      apply(next);
    } finally {
      inFlight = false;
      pending = null;
    }
  };
}

interface AppStore {
  state: BootstrapState | null;
  loading: boolean;
  importingHubs: boolean;
  lastImportReport: HubImportReport | null;
  t: TranslationDictionary;
  bootstrap: (subscribePreview: boolean) => Promise<void>;
  setPower: (on: boolean) => Promise<void>;
  setBrightness: (value: number) => Promise<void>;
  selectProfile: (id: string) => Promise<void>;
  updateEffect: (effect: EffectParams, profileBrightness?: number) => Promise<void>;
  updateKeyboardFx: (fx: KeyboardHwEffect | null) => Promise<void>;
  updatePlacement: (placement: DevicePlacement) => Promise<void>;
  updateAuraCounts: (counts: [number, number]) => Promise<void>;
  setLocale: (locale: Locale) => Promise<void>;
  updateAutomation: (automation: AutomationConfig) => Promise<void>;
  updateUiPrefs: (prefs: UiPrefs) => Promise<void>;
  refreshDevices: () => Promise<void>;
  importHubPresets: () => Promise<void>;
}

export const useAppStore = create<AppStore>((set, get) => {
  const applyState = (next: BootstrapState) => set({ state: next });

  const sendBrightness = latestWins(apiSetBrightness, applyState);
  const sendEffect = latestWins(
    (a: { effect: EffectParams; profileBrightness?: number }) => apiUpdateEffect(a.effect, a.profileBrightness),
    applyState,
  );
  const sendPlacement = latestWins(apiUpdatePlacement, applyState);
  const sendKeyboardFx = latestWins(apiUpdateKeyboardFx, applyState);
  // Night-brightness slider fires per pixel; only the newest value matters.
  const sendAutomation = latestWins(apiUpdateAutomation, applyState);

  return {
    state: null,
    loading: true,
    importingHubs: false,
    lastImportReport: null,
    t: getDictionary("pt-BR"),

    bootstrap: async (subscribePreview: boolean) => {
      const data = await apiGetBootstrapState();
      set({
        state: data,
        loading: false,
        t: getDictionary(data.config.locale),
      });
      if (data.canvas_pixels.length) publishPixels(Uint8Array.from(data.canvas_pixels));
      // Tray menu, automations or the other window changed something.
      await onStateChanged((next) => set({ state: next, t: getDictionary(next.config.locale) }));
      if (subscribePreview) {
        await apiSubscribeCanvasPreview(publishPixels);
      }
    },

    setPower: async (on: boolean) => {
      const prev = get().state;
      if (prev) {
        set({
          state: {
            ...prev,
            config: { ...prev.config, power_on: on },
          },
        });
      }
      const next = await apiSetPower(on);
      set({ state: next });
    },

    setBrightness: async (value: number) => {
      const prev = get().state;
      if (prev) {
        set({
          state: {
            ...prev,
            config: { ...prev.config, global_brightness: value },
          },
        });
      }
      await sendBrightness(value);
    },

    selectProfile: async (id: string) => {
      const prev = get().state;
      if (prev) {
        set({
          state: {
            ...prev,
            config: { ...prev.config, active_profile_id: id, power_on: id !== "off" },
          },
        });
      }
      const next = await apiSelectProfile(id);
      set({ state: next });
    },

    updateEffect: async (effect: EffectParams, profileBrightness?: number) => {
      const prev = get().state;
      if (prev) {
        // Mirrors the backend: editing while "Off" is active moves to a real profile.
        let activeId = prev.config.active_profile_id;
        if (activeId === "off" && effect.kind !== "off") {
          activeId = prev.config.profiles.find((p) => p.id !== "off")?.id ?? activeId;
        }
        set({
          state: {
            ...prev,
            config: {
              ...prev.config,
              power_on: true,
              active_profile_id: activeId,
              profiles: prev.config.profiles.map((p) =>
                p.id === activeId
                  ? { ...p, effect, brightness: profileBrightness ?? p.brightness }
                  : p,
              ),
            },
          },
        });
      }
      await sendEffect({ effect, profileBrightness });
    },

    updateKeyboardFx: async (fx: KeyboardHwEffect | null) => {
      const prev = get().state;
      if (prev) {
        set({
          state: {
            ...prev,
            config: {
              ...prev.config,
              profiles: prev.config.profiles.map((p) =>
                p.id === prev.config.active_profile_id ? { ...p, keyboard_fx: fx } : p,
              ),
            },
          },
        });
      }
      await sendKeyboardFx(fx);
    },

    updatePlacement: async (placement: DevicePlacement) => {
      const prev = get().state;
      if (prev) {
        set({
          state: {
            ...prev,
            config: {
              ...prev.config,
              placements: prev.config.placements.map((p) =>
                p.device_id === placement.device_id ? placement : p,
              ),
            },
          },
        });
      }
      await sendPlacement(placement);
    },

    updateAuraCounts: async (counts: [number, number]) => {
      const next = await apiUpdateAuraLedCounts(counts);
      set({ state: next });
    },

    setLocale: async (locale: Locale) => {
      set({ t: getDictionary(locale) });
      const next = await apiSetLocale(locale);
      set({ state: next, t: getDictionary(next.config.locale) });
    },

    updateAutomation: async (automation: AutomationConfig) => {
      const prev = get().state;
      if (prev) set({ state: { ...prev, config: { ...prev.config, automation } } });
      await sendAutomation(automation);
    },

    updateUiPrefs: async (prefs: UiPrefs) => {
      const prev = get().state;
      if (prev) set({ state: { ...prev, config: { ...prev.config, ui: prefs } } });
      const next = await apiUpdateUiPrefs(prefs);
      set({ state: next });
    },

    refreshDevices: async () => {
      const next = await apiRefreshDevices();
      set({ state: next });
    },

    importHubPresets: async () => {
      set({ importingHubs: true });
      try {
        const res = await apiImportHubPresets();
        set({ state: res.state, lastImportReport: res.report, importingHubs: false });
      } catch {
        set({ importingHubs: false });
      }
    },
  };
});
