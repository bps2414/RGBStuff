import { ArrowUpRight, BatteryCharging, Moon, Power, Sun } from "lucide-react";
import { useEffect } from "react";
import { iconForKind } from "../components/DevicesView";
import { SceneGrid, useActiveProfile } from "../components/EffectWidgets";
import { Slider } from "../components/ui";
import { apiOpenStudio } from "../lib/tauri";
import { effectBackground, setLiveAccentFromColor } from "../lib/visuals";
import { useAppStore } from "../store/useAppStore";

export function TrayPopover() {
  const { state, t, setPower, setBrightness, selectProfile } = useAppStore();
  const profile = useActiveProfile();
  const on = state?.config.power_on ?? false;

  // The popover has no live preview stream; tint it with the active profile.
  useEffect(() => {
    setLiveAccentFromColor(on && profile && profile.effect.kind !== "off" ? profile.effect.primary_color : null);
  }, [on, profile]);

  if (!state) return null;
  const { config, engine_status, devices, active_rule_app, night_mode_active, output_brightness } = state;
  const batteryDevices = devices.filter((d) => d.capabilities.includes("battery") && d.connected);

  const statusText =
    engine_status.mode === "off"
      ? t.engine.off
      : engine_status.mode === "static_zero_cpu"
        ? t.engine.staticZeroCpu
        : t.engine.dynamicFps;

  return (
    <div className="studio-shell flex h-full w-full flex-col gap-3.5 overflow-hidden rounded-xl border border-white/10 p-4 text-zinc-100">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div className="min-w-0">
          <div className="flex items-center gap-2">
            <span className={`h-2 w-2 rounded-full ${on ? "live-dot" : "bg-zinc-600"}`} />
            <span className="font-display text-sm font-semibold tracking-tight text-zinc-50">{t.app.name}</span>
            <span className="rounded-md border border-white/10 bg-white/5 px-1.5 py-0.5 text-[10px] font-medium text-zinc-300 tabular-nums">
              {statusText}
            </span>
          </div>
          <p className="mt-0.5 truncate text-[11px] text-zinc-400">
            {profile ? (t.presets[profile.id] ?? profile.name) : ""}
            {profile && profile.id !== "off" ? ` · ${t.effects.items[profile.effect.kind].name}` : ""}
            {active_rule_app ? ` · ${active_rule_app}` : ""}
          </p>
        </div>
        <button
          type="button"
          onClick={() => setPower(!on)}
          className={`studio-btn flex h-9 w-9 shrink-0 items-center justify-center rounded-xl border ${
            on ? "power-on" : "border-white/10 bg-white/5 text-zinc-400 hover:bg-white/10"
          }`}
          aria-label={on ? t.app.powerOff : t.app.powerOn}
        >
          <Power className="h-4 w-4" />
        </button>
      </div>

      {/* Brightness */}
      <div className="studio-panel rounded-xl p-3">
        <Slider
          icon={<Sun className="h-3.5 w-3.5 text-zinc-400" />}
          label={t.app.globalBrightness}
          value={config.global_brightness}
          display={`${config.global_brightness}%`}
          onChange={setBrightness}
        />
        {night_mode_active && (
          <div className="mt-2 flex items-center gap-1.5 text-[10px] text-amber-200">
            <Moon className="h-3 w-3" />
            {t.app.nightActive.replace("{n}", String(config.automation.night_brightness))} · {t.app.output} {output_brightness}%
          </div>
        )}
      </div>

      {/* Profiles */}
      <div>
        <div className="mb-1.5 text-[10px] font-semibold uppercase tracking-[0.08em] text-zinc-500">{t.app.profiles}</div>
        <div className="flex flex-wrap gap-1.5">
          {config.profiles.map((p) => {
            const active = config.active_profile_id === p.id && (on || p.id === "off");
            return (
              <button
                key={p.id}
                type="button"
                onClick={() => selectProfile(p.id)}
                className={`studio-btn flex items-center gap-1.5 rounded-lg border px-2.5 py-1.5 text-[11px] font-medium ${
                  active ? "border-white/30 bg-white/12 text-zinc-50" : "border-white/8 bg-white/[0.03] text-zinc-300 hover:bg-white/[0.07]"
                }`}
              >
                <span className="h-2.5 w-2.5 rounded-sm border border-white/20" style={{ background: effectBackground(p.effect) }} />
                {t.presets[p.id] ?? p.name}
              </button>
            );
          })}
        </div>
      </div>

      {/* Scenes */}
      <div className="min-h-0">
        <div className="mb-1.5 text-[10px] font-semibold uppercase tracking-[0.08em] text-zinc-500">{t.app.scenes}</div>
        <SceneGrid compact limit={6} />
      </div>

      {/* Peripherals (whatever this PC actually has) */}
      {batteryDevices.length > 0 && (
        <div className="grid grid-cols-2 gap-2">
          {batteryDevices.slice(0, 4).map((dev) => {
            const Icon = iconForKind(dev.kind);
            const pct = dev.telemetry?.percent;
            return (
              <div key={dev.id} className="flex items-center justify-between gap-2 rounded-lg border border-white/6 bg-black/20 px-2.5 py-2">
                <span className="flex min-w-0 items-center gap-2">
                  <Icon className="h-3.5 w-3.5 shrink-0 text-zinc-400" />
                  <span className="truncate text-[11px] text-zinc-200">{dev.name}</span>
                </span>
                <span className="flex shrink-0 items-center gap-1 text-[11px] font-medium text-zinc-300 tabular-nums">
                  {dev.telemetry?.charging && <BatteryCharging className="h-3 w-3 text-emerald-300" />}
                  {pct != null ? `${pct}%` : "—"}
                </span>
              </div>
            );
          })}
        </div>
      )}

      <button
        type="button"
        onClick={() => apiOpenStudio()}
        className="studio-btn btn-primary mt-auto flex w-full items-center justify-center gap-2 rounded-xl px-3 py-2 text-xs font-semibold"
      >
        {t.app.openStudio}
        <ArrowUpRight className="h-3.5 w-3.5" />
      </button>
    </div>
  );
}
