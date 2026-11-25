import * as Slider from "@radix-ui/react-slider";
import {
  Cpu,
  Grid2x2,
  Minus,
  Power,
  SlidersHorizontal,
  Sparkles,
  Square,
  Sun,
  X,
} from "lucide-react";
import { useState } from "react";
import { AutomationsView } from "../components/AutomationsView";
import { CanvasView } from "../components/CanvasView";
import { DevicesView } from "../components/DevicesView";
import { EffectsView } from "../components/EffectsView";
import { apiWindowClose, apiWindowMinimize, apiWindowToggleMaximize, rgbToHex } from "../lib/tauri";
import { useAppStore } from "../store/useAppStore";

type TabId = "canvas" | "effects" | "devices" | "automations";

export function StudioApp() {
  const { state, t, setPower, setBrightness, selectProfile, setLocale } = useAppStore();
  const [tab, setTab] = useState<TabId>("canvas");

  if (!state) return null;
  const { config, engine_status, active_rule_app } = state;

  const statusText =
    engine_status.mode === "off"
      ? t.engine.off
      : engine_status.mode === "static_zero_cpu"
        ? t.engine.staticZeroCpu
        : t.engine.dynamicFps;

  const navItems: Array<{ id: TabId; label: string; Icon: typeof Grid2x2 }> = [
    { id: "canvas", label: t.nav.canvas, Icon: Grid2x2 },
    { id: "effects", label: t.nav.effects, Icon: Sparkles },
    { id: "devices", label: t.nav.devices, Icon: Cpu },
    { id: "automations", label: t.nav.automations, Icon: SlidersHorizontal },
  ];

  return (
    <div className="studio-shell flex h-full w-full flex-col overflow-hidden rounded-xl border border-white/10 text-zinc-100">
      {/* Custom Frameless Mica Titlebar */}
      <header
        data-tauri-drag-region
        className="flex h-11 shrink-0 items-center justify-between border-b border-white/8 px-4"
      >
        <div className="flex items-center gap-2.5" data-tauri-drag-region>
          <div className="h-2.5 w-2.5 rounded-full bg-sky-400" />
          <span className="text-xs font-semibold tracking-tight text-zinc-100">
            {t.app.name} Studio
          </span>
          <span className="rounded-md border border-white/10 bg-white/[0.04] px-2 py-0.5 text-[10px] font-medium text-zinc-300 tabular-nums">
            {statusText}
          </span>
          {active_rule_app && (
            <span className="rounded-md border border-sky-400/30 bg-sky-400/12 px-2 py-0.5 font-mono text-[10px] text-sky-300">
              {active_rule_app}
            </span>
          )}
        </div>

        <div className="flex items-center gap-2">
          <button
            type="button"
            onClick={() => setLocale(config.locale === "pt-BR" ? "en-US" : "pt-BR")}
            className="studio-btn rounded-md border border-white/10 bg-white/[0.04] px-2 py-1 text-[11px] font-medium text-zinc-300 hover:bg-white/10"
          >
            {config.locale === "pt-BR" ? "PT-BR" : "EN-US"}
          </button>

          <button
            type="button"
            onClick={() => setPower(!config.power_on)}
            className={`studio-btn flex items-center gap-1.5 rounded-md border px-2.5 py-1 text-[11px] font-medium ${
              config.power_on
                ? "border-sky-400/40 bg-sky-400/15 text-sky-300"
                : "border-white/10 bg-white/5 text-zinc-400"
            }`}
          >
            <Power className="h-3 w-3" />
            <span>{config.power_on ? t.app.powerOn : t.app.powerOff}</span>
          </button>

          <div className="ml-2 flex items-center gap-1">
            <button
              type="button"
              onClick={() => apiWindowMinimize()}
              className="flex h-7 w-7 items-center justify-center rounded-md text-zinc-400 hover:bg-white/8 hover:text-zinc-100"
              aria-label={t.app.minimize}
            >
              <Minus className="h-3.5 w-3.5" />
            </button>
            <button
              type="button"
              onClick={() => apiWindowToggleMaximize()}
              className="flex h-7 w-7 items-center justify-center rounded-md text-zinc-400 hover:bg-white/8 hover:text-zinc-100"
              aria-label={t.app.maximize}
            >
              <Square className="h-3 w-3" />
            </button>
            <button
              type="button"
              onClick={() => apiWindowClose()}
              className="flex h-7 w-7 items-center justify-center rounded-md text-zinc-400 hover:bg-rose-500/20 hover:text-rose-300"
              aria-label={t.app.close}
            >
              <X className="h-3.5 w-3.5" />
            </button>
          </div>
        </div>
      </header>

      {/* Main Workspace */}
      <div className="flex min-h-0 flex-1">
        {/* Sidebar */}
        <aside className="flex w-56 shrink-0 flex-col justify-between border-r border-white/8 p-3">
          <nav className="space-y-1">
            {navItems.map(({ id, label, Icon }) => {
              const active = tab === id;
              return (
                <button
                  key={id}
                  type="button"
                  onClick={() => setTab(id)}
                  className={`studio-btn flex w-full items-center gap-2.5 rounded-xl px-3 py-2 text-xs font-medium ${
                    active
                      ? "bg-white/12 text-zinc-50"
                      : "text-zinc-400 hover:bg-white/[0.05] hover:text-zinc-200"
                  }`}
                >
                  <Icon className="h-4 w-4" />
                  <span>{label}</span>
                </button>
              );
            })}

            <div className="pt-4">
              <div className="mb-2 px-3 text-[10px] font-semibold uppercase tracking-wider text-zinc-500">
                {t.app.quickPresets}
              </div>
              <div className="space-y-1">
                {config.profiles.map((p) => {
                  const active = config.active_profile_id === p.id;
                  return (
                    <button
                      key={p.id}
                      type="button"
                      onClick={() => selectProfile(p.id)}
                      className={`studio-btn flex w-full items-center gap-2.5 rounded-lg px-3 py-1.5 text-xs ${
                        active
                          ? "bg-white/8 font-medium text-zinc-100"
                          : "text-zinc-400 hover:bg-white/[0.04] hover:text-zinc-200"
                      }`}
                    >
                      <span
                        className="h-2 w-2 rounded-full"
                        style={{
                          backgroundColor:
                            p.id === "off" ? "#3f3f46" : rgbToHex(p.effect.primary_color),
                        }}
                      />
                      <span className="truncate">{t.presets[p.id] ?? p.name}</span>
                    </button>
                  );
                })}
              </div>
            </div>
          </nav>

          {/* Global Brightness Footer */}
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
              <Slider.Track className="relative h-1.5 w-full grow rounded-full bg-white/10">
                <Slider.Range className="absolute h-full rounded-full bg-zinc-100" />
              </Slider.Track>
              <Slider.Thumb className="block h-3.5 w-3.5 rounded-full bg-zinc-100" />
            </Slider.Root>
          </div>
        </aside>

        {/* Active View */}
        <main className="min-w-0 flex-1 p-5">
          {tab === "canvas" && <CanvasView />}
          {tab === "effects" && <EffectsView />}
          {tab === "devices" && <DevicesView />}
          {tab === "automations" && <AutomationsView />}
        </main>
      </div>
    </div>
  );
}
