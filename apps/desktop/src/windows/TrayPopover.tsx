import * as Slider from "@radix-ui/react-slider";
import { ArrowUpRight, Battery, Headphones, Mouse, Power, Sun } from "lucide-react";
import { apiOpenStudio, rgbToHex } from "../lib/tauri";
import { useAppStore } from "../store/useAppStore";

export function TrayPopover() {
  const { state, t, setPower, setBrightness, selectProfile, setLocale } = useAppStore();
  if (!state) return null;

  const { config, engine_status, devices, active_rule_app } = state;
  const mouse = devices.find((d) => d.kind === "mouse");
  const headset = devices.find((d) => d.kind === "headset");

  const statusText =
    engine_status.mode === "off"
      ? t.engine.off
      : engine_status.mode === "static_zero_cpu"
        ? t.engine.staticZeroCpu
        : t.engine.dynamicFps;

  return (
    <div className="studio-shell flex h-full w-full flex-col justify-between rounded-xl border border-white/10 p-4 text-zinc-100">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <div className="flex items-center gap-2">
            <span className="text-sm font-semibold tracking-tight text-zinc-50">
              {t.app.name}
            </span>
            <span className="rounded-md border border-white/10 bg-white/5 px-1.5 py-0.5 text-[10px] font-medium text-zinc-300 tabular-nums">
              {statusText}
            </span>
          </div>
          <p className="mt-0.5 text-[11px] text-zinc-400">
            {config.power_on ? t.app.powerOn : t.app.powerOff}
            {active_rule_app ? ` · ${active_rule_app}` : ""}
          </p>
        </div>

        <div className="flex items-center gap-1.5">
          <button
            type="button"
            onClick={() => setLocale(config.locale === "pt-BR" ? "en-US" : "pt-BR")}
            className="studio-btn rounded-lg border border-white/10 bg-white/[0.04] px-2 py-1 text-[11px] font-medium text-zinc-300 hover:bg-white/10"
            title={t.app.language}
          >
            {config.locale === "pt-BR" ? "PT" : "EN"}
          </button>

          <button
            type="button"
            onClick={() => setPower(!config.power_on)}
            className={`studio-btn flex h-8 w-8 items-center justify-center rounded-lg border ${
              config.power_on
                ? "border-sky-400/40 bg-sky-400/15 text-sky-300 hover:bg-sky-400/25"
                : "border-white/10 bg-white/5 text-zinc-400 hover:bg-white/10"
            }`}
            aria-label={config.power_on ? t.app.powerOff : t.app.powerOn}
          >
            <Power className="h-4 w-4" />
          </button>
        </div>
      </div>

      {/* Brightness */}
      <div className="studio-panel rounded-xl p-3">
        <div className="mb-2 flex items-center justify-between text-xs">
          <span className="flex items-center gap-1.5 text-zinc-300">
            <Sun className="h-3.5 w-3.5 text-zinc-400" />
            {t.app.globalBrightness}
          </span>
          <span className="font-medium text-zinc-200 tabular-nums">
            {config.global_brightness}%
          </span>
        </div>
        <Slider.Root
          className="relative flex h-5 w-full touch-none items-center select-none"
          value={[config.global_brightness]}
          max={100}
          step={1}
          onValueChange={([v]) => setBrightness(v)}
        >
          <Slider.Track className="relative h-1.5 w-full grow overflow-hidden rounded-full bg-white/10">
            <Slider.Range className="absolute h-full rounded-full bg-zinc-100" />
          </Slider.Track>
          <Slider.Thumb className="block h-3.5 w-3.5 rounded-full border border-zinc-900 bg-zinc-100 shadow-sm focus:outline-none" />
        </Slider.Root>
      </div>

      {/* Quick Presets */}
      <div>
        <div className="mb-2 text-[11px] font-medium uppercase tracking-wider text-zinc-400">
          {t.app.quickPresets}
        </div>
        <div className="grid grid-cols-3 gap-2">
          {config.profiles.slice(0, 6).map((profile) => {
            const active = config.active_profile_id === profile.id && (config.power_on || profile.id === "off");
            const swatch = rgbToHex(profile.effect.primary_color);
            const label = t.presets[profile.id] ?? profile.name;
            return (
              <button
                key={profile.id}
                type="button"
                onClick={() => selectProfile(profile.id)}
                className={`studio-btn flex items-center gap-2 rounded-lg border px-2.5 py-2 text-left text-xs font-medium ${
                  active
                    ? "border-white/25 bg-white/12 text-zinc-50"
                    : "border-white/8 bg-white/[0.03] text-zinc-300 hover:bg-white/[0.07]"
                }`}
              >
                <span
                  className="h-2.5 w-2.5 shrink-0 rounded-full border border-white/20"
                  style={{
                    backgroundColor: profile.id === "off" ? "#27272a" : swatch,
                  }}
                />
                <span className="truncate">{label}</span>
              </button>
            );
          })}
        </div>
      </div>

      {/* Peripheral Telemetry */}
      <div className="studio-panel rounded-xl p-3">
        <div className="mb-2 text-[11px] font-medium uppercase tracking-wider text-zinc-400">
          {t.app.peripherals}
        </div>
        <div className="grid grid-cols-2 gap-2">
          {[
            { dev: mouse, Icon: Mouse, fallback: "Rapoo" },
            { dev: headset, Icon: Headphones, fallback: "MCHOSE V9" },
          ].map(({ dev, Icon, fallback }) => {
            const pct = dev?.telemetry?.percent;
            const connected = dev?.connected ?? false;
            return (
              <div
                key={fallback}
                className="flex items-center justify-between rounded-lg border border-white/6 bg-black/20 px-2.5 py-2"
              >
                <div className="flex items-center gap-2 min-w-0">
                  <Icon className="h-3.5 w-3.5 shrink-0 text-zinc-400" />
                  <div className="min-w-0">
                    <div className="truncate text-[11px] font-medium text-zinc-200">
                      {fallback}
                    </div>
                    <div className="text-[10px] text-zinc-500">
                      {connected ? t.devices.connected : t.devices.disconnected}
                    </div>
                  </div>
                </div>
                <span className="flex items-center gap-1 text-[11px] font-medium text-zinc-300 tabular-nums">
                  <Battery className="h-3 w-3 text-zinc-400" />
                  {pct !== null && pct !== undefined ? `${pct}%` : "—"}
                </span>
              </div>
            );
          })}
        </div>
      </div>

      {/* Footer Action */}
      <button
        type="button"
        onClick={() => apiOpenStudio()}
        className="studio-btn flex w-full items-center justify-center gap-2 rounded-xl border border-white/12 bg-zinc-100 px-3 py-2 text-xs font-semibold text-zinc-950 hover:bg-white"
      >
        <span>{t.app.openStudio}</span>
        <ArrowUpRight className="h-3.5 w-3.5" />
      </button>
    </div>
  );
}
