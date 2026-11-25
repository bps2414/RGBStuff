import { Battery, Cpu, Download, Fan, Headphones, Keyboard, MemoryStick, Mouse, RefreshCw } from "lucide-react";
import { useAppStore } from "../store/useAppStore";

export function DevicesView() {
  const { state, t, refreshDevices, updateAuraCounts, importHubPresets, importingHubs } =
    useAppStore();
  if (!state) return null;

  const { config, devices } = state;

  const iconFor = (kind: string) => {
    switch (kind) {
      case "motherboard_aura":
        return Fan;
      case "ram":
        return MemoryStick;
      case "keyboard":
        return Keyboard;
      case "mouse":
        return Mouse;
      case "headset":
        return Headphones;
      default:
        return Cpu;
    }
  };

  return (
    <div className="flex h-full flex-col gap-4 overflow-y-auto pr-1">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-base font-semibold text-zinc-100">{t.devices.title}</h2>
          <p className="text-xs text-zinc-400">{t.devices.subtitle}</p>
        </div>
        <button
          type="button"
          onClick={() => refreshDevices()}
          className="studio-btn flex items-center gap-1.5 rounded-lg border border-white/12 bg-white/5 px-3 py-1.5 text-xs font-medium text-zinc-200 hover:bg-white/10"
        >
          <RefreshCw className="h-3.5 w-3.5" />
          <span>{t.devices.refresh}</span>
        </button>
      </div>

      {/* Hardware Grid */}
      <div className="grid grid-cols-1 gap-3 md:grid-cols-2">
        {devices.map((dev) => {
          const Icon = iconFor(dev.kind);
          const hasRgb = dev.capabilities.includes("rgb");
          return (
            <div key={dev.id} className="studio-card flex flex-col justify-between rounded-2xl p-4">
              <div className="flex items-start justify-between gap-3">
                <div className="flex items-start gap-3">
                  <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-xl border border-white/10 bg-white/5 text-zinc-200">
                    <Icon className="h-4 w-4" />
                  </div>
                  <div>
                    <div className="text-sm font-medium text-zinc-100">{dev.name}</div>
                    <div className="mt-0.5 flex items-center gap-2 text-[11px] text-zinc-400">
                      <span className="font-mono">{dev.vid_pid}</span>
                      <span>·</span>
                      <span>
                        {dev.transport === "usb_hid"
                          ? t.devices.transportUsb
                          : t.devices.transportSmbus}
                      </span>
                    </div>
                  </div>
                </div>

                <span
                  className={`rounded-full px-2 py-0.5 text-[11px] font-medium ${
                    dev.connected
                      ? "border border-emerald-500/30 bg-emerald-500/10 text-emerald-300"
                      : "border border-zinc-700 bg-zinc-800/70 text-zinc-400"
                  }`}
                >
                  {dev.connected ? t.devices.connected : t.devices.disconnected}
                </span>
              </div>

              <div className="mt-3 flex flex-wrap items-center justify-between gap-2 border-t border-white/6 pt-3 text-xs">
                <span className="text-zinc-400">{dev.status_note}</span>
                {hasRgb ? (
                  <span className="text-zinc-300 tabular-nums">
                    {dev.zones.reduce((acc, z) => acc + z.led_count, 0)} LEDs
                  </span>
                ) : (
                  <span className="flex items-center gap-1.5 text-zinc-300 tabular-nums">
                    <Battery className="h-3.5 w-3.5 text-zinc-400" />
                    {dev.telemetry?.percent != null ? `${dev.telemetry.percent}%` : t.devices.noRgbTelemetryOnly}
                  </span>
                )}
              </div>
            </div>
          );
        })}
      </div>

      {/* ASUS Aura Header LED Calibration */}
      <div className="studio-panel rounded-2xl p-4">
        <div className="mb-3 text-xs font-semibold uppercase tracking-wider text-zinc-400">
          {t.devices.auraCalibration}
        </div>
        <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
          {[0, 1].map((idx) => (
            <label key={idx} className="flex items-center justify-between gap-3 rounded-xl border border-white/8 bg-black/20 px-3 py-2.5 text-xs">
              <span className="text-zinc-300">
                {idx === 0 ? t.devices.header1Leds : t.devices.header2Leds}
              </span>
              <input
                type="number"
                min={1}
                max={120}
                value={config.aura_argb_led_counts[idx]}
                onChange={(e) => {
                  const v = Math.max(1, Math.min(120, Number(e.target.value) || 1));
                  const next: [number, number] = [...config.aura_argb_led_counts] as [number, number];
                  next[idx] = v;
                  void updateAuraCounts(next);
                }}
                className="w-20 rounded-lg border border-white/12 bg-zinc-900 px-2 py-1 text-right text-zinc-100 tabular-nums focus:border-sky-400 focus:outline-none"
              />
            </label>
          ))}
        </div>
      </div>

      {/* Hub Preset Importer */}
      <div className="studio-panel rounded-2xl p-4">
        <div className="mb-3 flex flex-wrap items-center justify-between gap-2">
          <div>
            <div className="text-xs font-semibold uppercase tracking-wider text-zinc-400">
              {t.devices.hubImporterTitle}
            </div>
            <p className="mt-0.5 text-xs text-zinc-400">{t.devices.hubImporterDesc}</p>
          </div>
          <button
            type="button"
            disabled={importingHubs}
            onClick={() => void importHubPresets()}
            className="studio-btn flex items-center gap-1.5 rounded-lg border border-white/12 bg-zinc-100 px-3 py-1.5 text-xs font-semibold text-zinc-950 hover:bg-white disabled:opacity-60"
          >
            <Download className="h-3.5 w-3.5" />
            <span>{importingHubs ? t.devices.importingHubs : t.devices.importHubs}</span>
          </button>
        </div>

        {config.imported_hub_presets.length === 0 ? (
          <p className="text-xs text-zinc-500">{t.devices.noImportedPresets}</p>
        ) : (
          <div className="grid grid-cols-1 gap-2 sm:grid-cols-3">
            {config.imported_hub_presets.map((item, idx) => (
              <div
                key={`${item.hub}-${idx}`}
                className="rounded-xl border border-white/8 bg-black/25 p-3 text-xs"
              >
                <div className="font-medium text-zinc-100">{item.hub}</div>
                <div className="mt-0.5 text-[11px] text-zinc-300">{item.preset_name}</div>
                <div className="mt-1 text-[11px] text-zinc-400">{item.summary}</div>
              </div>
            ))}
          </div>
        )}
      </div>

      {/* Phase 2 Roadmap */}
      <div className="studio-panel rounded-2xl p-4">
        <div className="mb-2 text-xs font-semibold uppercase tracking-wider text-zinc-400">
          {t.devices.phase2Title}
        </div>
        <ul className="space-y-1.5 text-xs text-zinc-400">
          <li>• {t.devices.phase2Rapoo}</li>
          <li>• {t.devices.phase2Aula}</li>
          <li>• {t.devices.phase2Mchose}</li>
        </ul>
      </div>
    </div>
  );
}
