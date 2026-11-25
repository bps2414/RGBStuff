import { Cpu, Grid2x2, Minus, Moon, Power, Settings, SlidersHorizontal, Sparkles, Square, Sun, X, Zap } from "lucide-react";
import { useEffect, useState } from "react";
import { AutomationsView } from "../components/AutomationsView";
import { CanvasView } from "../components/CanvasView";
import { DevicesView } from "../components/DevicesView";
import { EffectsView } from "../components/EffectsView";
import { SettingsView } from "../components/SettingsView";
import { Slider } from "../components/ui";
import { apiWindowClose, apiWindowMinimize, apiWindowToggleMaximize } from "../lib/tauri";
import { effectBackground } from "../lib/visuals";
import { useAppStore } from "../store/useAppStore";

type TabId = "canvas" | "effects" | "devices" | "automations" | "settings";
const TABS: TabId[] = ["canvas", "effects", "devices", "automations", "settings"];

export function StudioApp() {
  const { state, t, setPower, setBrightness, selectProfile } = useAppStore();
  const [tab, setTab] = useState<TabId>("effects");

  // Ctrl+1..5 switches tabs, Ctrl+Shift+L toggles the lights.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!e.ctrlKey) return;
      const n = Number(e.key);
      if (n >= 1 && n <= TABS.length) {
        e.preventDefault();
        setTab(TABS[n - 1]);
      } else if (e.shiftKey && e.key.toLowerCase() === "l") {
        e.preventDefault();
        const s = useAppStore.getState();
        if (s.state) void s.setPower(!s.state.config.power_on);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  if (!state) return null;
  const { config, engine_status, active_rule_app, night_mode_active, output_brightness } = state;

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
    { id: "settings", label: t.nav.settings, Icon: Settings },
  ];

  const capped = output_brightness < config.global_brightness;

  return (
    <div className="studio-shell flex h-full w-full flex-col overflow-hidden rounded-xl border border-white/10 text-zinc-100">
      {/* Frameless Mica titlebar */}
      <header data-tauri-drag-region className="flex h-12 shrink-0 items-center justify-between border-b border-white/[0.06] px-4">
        <div className="flex min-w-0 items-center gap-3" data-tauri-drag-region>
          <span className={`h-2.5 w-2.5 shrink-0 rounded-full ${config.power_on ? "live-dot" : "bg-zinc-600"}`} />
          <span className="font-display text-sm font-semibold tracking-tight text-zinc-50" data-tauri-drag-region>
            {t.app.name}
            <span className="ml-1 font-normal text-zinc-500">Studio</span>
          </span>
          <span className="rounded-md border border-white/10 bg-white/[0.04] px-2 py-0.5 text-[10px] font-medium text-zinc-300 tabular-nums">
            {statusText}
          </span>
          {night_mode_active && (
            <button
              type="button"
              onClick={() => setTab("automations")}
              className="studio-btn flex items-center gap-1 rounded-md border border-amber-400/30 bg-amber-400/10 px-2 py-0.5 text-[10px] font-medium text-amber-200 hover:bg-amber-400/20"
            >
              <Moon className="h-3 w-3" />
              {t.app.nightActive.replace("{n}", String(config.automation.night_brightness))}
            </button>
          )}
          {active_rule_app && (
            <span className="truncate rounded-md border border-white/10 bg-white/[0.04] px-2 py-0.5 font-mono text-[10px] text-zinc-300">
              <Zap className="mr-1 inline h-3 w-3 -translate-y-px" />
              {active_rule_app}
            </span>
          )}
        </div>

        <div className="flex items-center gap-2">
          <button
            type="button"
            onClick={() => setPower(!config.power_on)}
            title="Ctrl+Shift+L"
            className={`studio-btn flex items-center gap-1.5 rounded-lg border px-3 py-1 text-[11px] font-semibold ${
              config.power_on ? "power-on" : "border-white/10 bg-white/5 text-zinc-400 hover:text-zinc-200"
            }`}
          >
            <Power className="h-3.5 w-3.5" />
            <span>{config.power_on ? t.app.powerOn : t.app.powerOff}</span>
          </button>

          <div className="ml-1 flex items-center">
            <button
              type="button"
              onClick={() => apiWindowMinimize()}
              className="flex h-8 w-9 items-center justify-center rounded-md text-zinc-400 hover:bg-white/8 hover:text-zinc-100"
              aria-label={t.app.minimize}
            >
              <Minus className="h-3.5 w-3.5" />
            </button>
            <button
              type="button"
              onClick={() => apiWindowToggleMaximize()}
              className="flex h-8 w-9 items-center justify-center rounded-md text-zinc-400 hover:bg-white/8 hover:text-zinc-100"
              aria-label={t.app.maximize}
            >
              <Square className="h-3 w-3" />
            </button>
            <button
              type="button"
              onClick={() => apiWindowClose()}
              className="flex h-8 w-9 items-center justify-center rounded-md text-zinc-400 hover:bg-rose-500/80 hover:text-white"
              aria-label={t.app.close}
            >
              <X className="h-3.5 w-3.5" />
            </button>
          </div>
        </div>
      </header>

      <div className="flex min-h-0 flex-1">
        {/* Sidebar */}
        <aside className="flex w-60 shrink-0 flex-col gap-4 border-r border-white/[0.06] p-3">
          <nav className="space-y-0.5">
            {navItems.map(({ id, label, Icon }, i) => (
              <button
                key={id}
                type="button"
                onClick={() => setTab(id)}
                data-active={tab === id}
                className="nav-item studio-btn group flex w-full items-center gap-2.5 rounded-lg px-3 py-2 text-xs font-medium text-zinc-400 hover:bg-white/[0.05] hover:text-zinc-200"
              >
                <Icon className="h-4 w-4" />
                <span className="flex-1 text-left">{label}</span>
                <span className="text-[10px] text-zinc-600 opacity-0 transition-opacity group-hover:opacity-100">Ctrl {i + 1}</span>
              </button>
            ))}
          </nav>

          <div className="min-h-0 flex-1 overflow-y-auto">
            <div className="mb-1.5 px-3 text-[10px] font-semibold uppercase tracking-[0.08em] text-zinc-500">{t.app.profiles}</div>
            <div className="space-y-0.5">
              {config.profiles.map((p) => {
                const active = config.active_profile_id === p.id && (config.power_on || p.id === "off");
                return (
                  <button
                    key={p.id}
                    type="button"
                    onClick={() => selectProfile(p.id)}
                    className={`studio-btn flex w-full items-center gap-2.5 rounded-lg px-3 py-1.5 text-xs ${
                      active ? "bg-white/[0.08] font-medium text-zinc-50" : "text-zinc-400 hover:bg-white/[0.04] hover:text-zinc-200"
                    }`}
                  >
                    <span
                      className={`fx-thumb h-4 w-4 shrink-0 rounded-md border border-white/15 ${active ? "live-ring" : ""}`}
                      style={{ background: effectBackground(p.effect) }}
                    />
                    <span className="flex-1 truncate text-left">{t.presets[p.id] ?? p.name}</span>
                    {p.id !== "off" && <span className="text-[10px] text-zinc-500">{t.effects.items[p.effect.kind].name}</span>}
                  </button>
                );
              })}
            </div>
          </div>

          {/* Global brightness */}
          <div className="studio-panel rounded-xl p-3">
            <Slider
              icon={<Sun className="h-3.5 w-3.5 text-zinc-400" />}
              label={t.app.globalBrightness}
              value={config.global_brightness}
              display={`${config.global_brightness}%`}
              onChange={setBrightness}
            />
            <div className="mt-2 flex items-center justify-between text-[10px] text-zinc-500">
              <span>{t.app.output}</span>
              <span className={`tabular-nums ${capped ? "text-amber-300" : "text-zinc-400"}`}>
                {config.power_on ? `${output_brightness}%` : "0%"}
              </span>
            </div>
          </div>
        </aside>

        <main className="min-w-0 flex-1 p-5">
          {tab === "canvas" && <CanvasView onOpenDevices={() => setTab("devices")} />}
          {tab === "effects" && <EffectsView />}
          {tab === "devices" && <DevicesView />}
          {tab === "automations" && <AutomationsView />}
          {tab === "settings" && <SettingsView />}
        </main>
      </div>
    </div>
  );
}
