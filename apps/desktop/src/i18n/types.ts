export type Locale = "pt-BR" | "en-US";

export type EffectKind =
  | "off"
  | "solid"
  | "gradient"
  | "rainbow_wave"
  | "breathing"
  | "dual_pulse"
  | "starlight"
  | "ambilight"
  | "audio_spectrum";

export interface TranslationDictionary {
  app: {
    name: string;
    subtitle: string;
    openStudio: string;
    powerOn: string;
    powerOff: string;
    globalBrightness: string;
    quickPresets: string;
    peripherals: string;
    language: string;
    close: string;
    minimize: string;
    maximize: string;
    save: string;
    saved: string;
  };
  engine: {
    off: string;
    staticZeroCpu: string;
    dynamicFps: string;
    zeroCpuBadge: string;
    liveCanvasBadge: string;
  };
  nav: {
    canvas: string;
    effects: string;
    devices: string;
    automations: string;
  };
  presets: Record<string, string>;
  effects: {
    title: string;
    subtitle: string;
    primaryColor: string;
    secondaryColor: string;
    speed: string;
    angle: string;
    scale: string;
    profileBrightness: string;
    items: Record<EffectKind, { name: string; description: string }>;
  };
  canvas: {
    title: string;
    subtitle: string;
    dragHint: string;
    selectedDevice: string;
    position: string;
    size: string;
    rotation: string;
    leds: string;
    inspectorTitle: string;
    previewLive: string;
    previewStatic: string;
  };
  devices: {
    title: string;
    subtitle: string;
    refresh: string;
    connected: string;
    disconnected: string;
    sleeping: string;
    charging: string;
    battery: string;
    dpi: string;
    noRgbTelemetryOnly: string;
    auraCalibration: string;
    header1Leds: string;
    header2Leds: string;
    phase2Title: string;
    phase2Rapoo: string;
    phase2Aula: string;
    phase2Mchose: string;
    transportUsb: string;
    transportSmbus: string;
    hubImporterTitle: string;
    hubImporterDesc: string;
    importHubs: string;
    importingHubs: string;
    noImportedPresets: string;
  };
  automations: {
    title: string;
    subtitle: string;
    powerSection: string;
    autoOffOnLock: string;
    autoOffOnLockDesc: string;
    idleTimeout: string;
    idleTimeoutDesc: string;
    minutes: string;
    disabled: string;
    nightMode: string;
    nightModeDesc: string;
    nightStart: string;
    nightEnd: string;
    nightBrightness: string;
    appRulesTitle: string;
    appRulesDesc: string;
    activeFocusRule: string;
    pickRunningApp: string;
    refreshApps: string;
    noRunningApps: string;
    executablePlaceholder: string;
    addRule: string;
    remove: string;
    batteryAlertsTitle: string;
    lowWarn: string;
    lowCrit: string;
    systemSection: string;
    startWithWindows: string;
    startWithWindowsDesc: string;
    languageLabel: string;
    languageDesc: string;
  };
}
