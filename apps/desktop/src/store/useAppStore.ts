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
  apiUpdatePlacement,
  type AutomationConfig,
  type BootstrapState,
  type DevicePlacement,
  type EffectParams,
  type HubImportReport,
} from "../lib/tauri";

interface AppStore {
  state: BootstrapState | null;
  canvasPixels: number[];
  loading: boolean;
  importingHubs: boolean;
  lastImportReport: HubImportReport | null;
  t: TranslationDictionary;
  bootstrap: (subscribePreview: boolean) => Promise<void>;
  setPower: (on: boolean) => Promise<void>;
  setBrightness: (value: number) => Promise<void>;
  selectProfile: (id: string) => Promise<void>;
  updateEffect: (effect: EffectParams, profileBrightness?: number) => Promise<void>;
  updatePlacement: (placement: DevicePlacement) => Promise<void>;
  updateAuraCounts: (counts: [number, number]) => Promise<void>;
  setLocale: (locale: Locale) => Promise<void>;
  updateAutomation: (automation: AutomationConfig) => Promise<void>;
  refreshDevices: () => Promise<void>;
  importHubPresets: () => Promise<void>;
}

export const useAppStore = create<AppStore>((set, get) => ({
  state: null,
  canvasPixels: [],
  loading: true,
  importingHubs: false,
  lastImportReport: null,
  t: getDictionary("pt-BR"),

  bootstrap: async (subscribePreview: boolean) => {
    const data = await apiGetBootstrapState();
    set({
      state: data,
      canvasPixels: data.canvas_pixels,
      loading: false,
      t: getDictionary(data.config.locale),
    });
    if (subscribePreview) {
      await apiSubscribeCanvasPreview((pixels) => {
        set({ canvasPixels: pixels });
      });
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
    set({ state: next, canvasPixels: next.canvas_pixels });
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
    const next = await apiSetBrightness(value);
    set({ state: next, canvasPixels: next.canvas_pixels });
  },

  selectProfile: async (id: string) => {
    const next = await apiSelectProfile(id);
    set({ state: next, canvasPixels: next.canvas_pixels });
  },

  updateEffect: async (effect: EffectParams, profileBrightness?: number) => {
    const next = await apiUpdateEffect(effect, profileBrightness);
    set({ state: next, canvasPixels: next.canvas_pixels });
  },

  updatePlacement: async (placement: DevicePlacement) => {
    const next = await apiUpdatePlacement(placement);
    set({ state: next, canvasPixels: next.canvas_pixels });
  },

  updateAuraCounts: async (counts: [number, number]) => {
    const next = await apiUpdateAuraLedCounts(counts);
    set({ state: next, canvasPixels: next.canvas_pixels });
  },

  setLocale: async (locale: Locale) => {
    set({ t: getDictionary(locale) });
    const next = await apiSetLocale(locale);
    set({ state: next, t: getDictionary(next.config.locale) });
  },

  updateAutomation: async (automation: AutomationConfig) => {
    const next = await apiUpdateAutomation(automation);
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
}));
