export type Locale = "pt-BR" | "en-US";

export type EffectKind =
  | "off"
  | "solid"
  | "gradient"
  | "rainbow_wave"
  | "breathing"
  | "dual_pulse"
  | "starlight"
  | "aurora"
  | "plasma"
  | "ripple"
  | "color_cycle"
  | "comet"
  | "snowfall"
  | "rain"
  | "fire"
  | "windmill"
  | "gathering"
  | "fountain"
  | "bloom"
  | "mountains"
  | "neon"
  | "cross"
  | "cpu_load"
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
    peripherals: string;
    language: string;
    close: string;
    minimize: string;
    maximize: string;
    profiles: string;
    scenes: string;
    /** `{n}` = night brightness cap. */
    nightActive: string;
    output: string;
    quit: string;
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
    settings: string;
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
    scenesTitle: string;
    scenesDesc: string;
    editing: string;
    allEffects: string;
    parameters: string;
    responsiveness: string;
    unify: string;
    noParams: string;
    items: Record<EffectKind, { name: string; description: string }>;
    groups: { classic: string; motion: string; nature: string; reactive: string };
    keyboard: {
      title: string;
      desc: string;
      followCanvas: string;
      followCanvasDesc: string;
      color: string;
      colorful: string;
      speed: string;
      reverse: string;
      reactiveHint: string;
      activeBadge: string;
      /** Firmware mode number → name. */
      modes: Record<number, string>;
    };
  };
  canvas: {
    title: string;
    subtitle: string;
    dragHint: string;
    selectedDevice: string;
    rotation: string;
    leds: string;
    inspectorTitle: string;
    previewLive: string;
    previewStatic: string;
    width: string;
    height: string;
    intensity: string;
    noDevices: string;
    noDevicesDesc: string;
    notDetected: string;
  };
  devices: {
    title: string;
    subtitle: string;
    refresh: string;
    connected: string;
    disconnected: string;
    charging: string;
    battery: string;
    noRgbTelemetryOnly: string;
    auraCalibration: string;
    header1Leds: string;
    header2Leds: string;
    intensity: string;
    intensityDesc: string;
    boost: string;
    ramHint: string;
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
  };
  settings: {
    title: string;
    subtitle: string;
    startup: string;
    startWithWindows: string;
    startWithWindowsDesc: string;
    closeAction: string;
    closeActionDesc: string;
    closeToTray: string;
    closeQuit: string;
    interface: string;
    languageLabel: string;
    languageDesc: string;
    about: string;
    version: string;
    aboutDesc: string;
    quitDesc: string;
    shortcuts: string;
    shortcutsDesc: string;
    trayBattery: string;
    trayBatteryDesc: string;
    noBatteryDevices: string;
    updates: string;
    autoUpdate: string;
    autoUpdateDesc: string;
    checkNow: string;
    checking: string;
    upToDate: string;
    updateAvailable: string;
    installUpdate: string;
    installing: string;
    updateError: string;
  };
}
