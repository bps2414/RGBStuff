import { Battery, BatteryCharging, Cpu, Download, Fan, Headphones, Info, Keyboard, MemoryStick, Mouse, RefreshCw } from "lucide-react";
import { useState } from "react";
import { useAppStore } from "../store/useAppStore";
import { Panel, Slider } from "./ui";

export function iconForKind(kind: string) {
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
}

export function DevicesView() {
  const { state, t, refreshDevices, updateAuraCounts, updatePlacement, importHubPresets, importingHubs } = useAppStore();
  const [scanning, setScanning] = useState(false);
  if (!state) return null;
  const { config, devices } = state;

  const rescan = async () => {
    setScanning(true);
    try {
      await refreshDevices();
    } finally {
      setScanning(false);
    }
  };

  // Connected first, then RGB before telemetry-only.
  const sorted = [...devices].sort(
    (a, b) => Number(b.connected) - Number(a.connected) || Number(b.capabilities.includes("rgb")) - Number(a.capabilities.includes("rgb")),
  );
  const aura = devices.find((d) => d.kind === "motherboard_aura");

  return (
    <div className="fade-in flex h-full flex-col gap-4 overflow-y-auto pr-1">
      <div className="flex items-end justify-between">
        <div>
          <h2 className="font-display text-lg font-semibold text-zinc-50">{t.devices.title}</h2>
          <p className="text-xs text-zinc-400">{t.devices.subtitle}</p>
        </div>
        <button
          type="button"
          onClick={rescan}
          disabled={scanning}
          className="studio-btn flex items-center gap-1.5 rounded-lg border border-white/12 bg-white/5 px-3 py-1.5 text-xs font-medium text-zinc-200 hover:bg-white/10 disabled:opacity-60"
        >
          <RefreshCw className={`h-3.5 w-3.5 ${scanning ? "animate-spin" : ""}`} />
          {t.devices.refresh}
        </button>
      </div>

      <div className="grid grid-cols-1 gap-3 lg:grid-cols-2">
        {sorted.map((dev) => {
          const Icon = iconForKind(dev.kind);
          const hasRgb = dev.capabilities.includes("rgb");
          const placement = config.placements.find((p) => p.device_id === dev.id);
          const tel = dev.telemetry;
          const leds = dev.zones.reduce((acc, z) => acc + z.led_count, 0);
          return (
            <div key={dev.id} className={`studio-card flex flex-col gap-3 rounded-2xl p-4 ${dev.connected ? "" : "opacity-60"}`}>
              <div className="flex items-start justify-between gap-3">
                <div className="flex min-w-0 items-start gap-3">
                  <div
                    className={`flex h-10 w-10 shrink-0 items-center justify-center rounded-xl border ${
                      dev.connected && hasRgb ? "live-ring border-transparent bg-white/[0.06]" : "border-white/10 bg-white/5"
                    } text-zinc-100`}
                  >
                    <Icon className="h-4.5 w-4.5" />
                  </div>
                  <div className="min-w-0">
                    <div className="truncate text-sm font-medium text-zinc-100">{dev.name}</div>
                    <div className="mt-0.5 flex flex-wrap items-center gap-x-2 text-[11px] text-zinc-500">
                      <span className="font-mono">{dev.vid_pid}</span>
                      <span>·</span>
                      <span>{dev.transport === "usb_hid" ? t.devices.transportUsb : t.devices.transportSmbus}</span>
                      {hasRgb && leds > 0 && (
                        <>
                          <span>·</span>
                          <span className="tabular-nums">{leds} LEDs</span>
                        </>
                      )}
                    </div>
                  </div>
                </div>
                <span
                  className={`shrink-0 rounded-full px-2 py-0.5 text-[10px] font-medium ${
                    dev.connected ? "bg-emerald-500/12 text-emerald-300" : "bg-zinc-800/80 text-zinc-400"
                  }`}
                >
                  {dev.connected ? t.devices.connected : t.devices.disconnected}
                </span>
              </div>

              {hasRgb && placement && dev.connected && (
                <Slider
                  label={t.devices.intensity}
                  value={placement.brightness ?? 100}
                  min={0}
                  max={200}
                  step={5}
                  marker={100}
                  display={
                    (placement.brightness ?? 100) > 100 ? (
                      <span className="live-text">
                        {t.devices.boost} {placement.brightness}%
                      </span>
                    ) : (
                      `${placement.brightness ?? 100}%`
                    )
                  }
                  onChange={(v) => updatePlacement({ ...placement, brightness: v })}
                />
              )}

              {!hasRgb && tel && (
                <div className="flex items-center gap-2 text-xs text-zinc-300">
                  {tel.charging ? <BatteryCharging className="h-4 w-4 text-emerald-300" /> : <Battery className="h-4 w-4 text-zinc-400" />}
                  {tel.percent != null ? (
                    <>
                      <div className="h-1.5 w-24 overflow-hidden rounded-full bg-white/10">
                        <div
                          className={`h-full rounded-full ${tel.percent <= 15 ? "bg-rose-400" : tel.percent <= 30 ? "bg-amber-300" : "bg-emerald-400"}`}
                          style={{ width: `${tel.percent}%` }}
                        />
                      </div>
                      <span className="tabular-nums">{tel.percent}%</span>
                      {tel.dpi != null && <span className="text-zinc-500">· {tel.dpi} DPI</span>}
                    </>
                  ) : (
                    <span className="text-zinc-500">{t.devices.noRgbTelemetryOnly}</span>
                  )}
                </div>
              )}

              <div className="flex items-start gap-1.5 border-t border-white/6 pt-2.5 text-[11px] text-zinc-500">
                <span className="min-w-0 flex-1 break-words">{dev.status_note}</span>
              </div>
              {dev.kind === "ram" && !dev.connected && (
                <p className="flex items-start gap-1.5 text-[11px] text-amber-200/80">
                  <Info className="mt-px h-3.5 w-3.5 shrink-0" />
                  {t.devices.ramHint}
                </p>
              )}
            </div>
          );
        })}
      </div>

      {aura?.connected && (
        <Panel title={t.devices.auraCalibration}>
          <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
            {[0, 1].map((idx) => (
              <label key={idx} className="flex items-center justify-between gap-3 rounded-xl border border-white/8 bg-black/20 px-3 py-2.5 text-xs">
                <span className="text-zinc-300">{idx === 0 ? t.devices.header1Leds : t.devices.header2Leds}</span>
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
                  className="w-20 rounded-lg border border-white/12 bg-zinc-900 px-2 py-1 text-right text-zinc-100 tabular-nums focus:border-white/30 focus:outline-none"
                />
              </label>
            ))}
          </div>
        </Panel>
      )}

      <Panel
        title={t.devices.hubImporterTitle}
        desc={t.devices.hubImporterDesc}
        actions={
          <button
            type="button"
            disabled={importingHubs}
            onClick={() => void importHubPresets()}
            className="studio-btn btn-primary flex shrink-0 items-center gap-1.5 rounded-lg px-3 py-1.5 text-xs font-semibold disabled:opacity-60"
          >
            <Download className="h-3.5 w-3.5" />
            {importingHubs ? t.devices.importingHubs : t.devices.importHubs}
          </button>
        }
      >
        {config.imported_hub_presets.length === 0 ? (
          <p className="text-xs text-zinc-500">{t.devices.noImportedPresets}</p>
        ) : (
          <div className="grid grid-cols-1 gap-2 sm:grid-cols-3">
            {config.imported_hub_presets.map((item, idx) => (
              <div key={`${item.hub}-${idx}`} className="rounded-xl border border-white/8 bg-black/25 p-3 text-xs">
                <div className="font-medium text-zinc-100">{item.hub}</div>
                <div className="mt-0.5 text-[11px] text-zinc-300">{item.preset_name}</div>
                <div className="mt-1 text-[11px] text-zinc-400">{item.summary}</div>
              </div>
            ))}
          </div>
        )}
      </Panel>
    </div>
  );
}
