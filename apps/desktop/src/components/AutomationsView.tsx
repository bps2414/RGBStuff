import { Moon, Plus, RefreshCw, Trash2, Zap } from "lucide-react";
import { useEffect, useState } from "react";
import { apiListRunningApps, type RunningAppInfo } from "../lib/tauri";
import { useAppStore } from "../store/useAppStore";
import { Panel, SettingRow, Slider, Toggle } from "./ui";

const field =
  "rounded-lg border border-white/12 bg-zinc-900/80 px-2.5 py-1.5 text-xs text-zinc-100 focus:border-white/30 focus:outline-none";

export function AutomationsView() {
  const { state, t, updateAutomation } = useAppStore();
  const [exeInput, setExeInput] = useState("");
  const [ruleProfileId, setRuleProfileId] = useState("gaming");
  const [runningApps, setRunningApps] = useState<RunningAppInfo[]>([]);

  const loadRunningApps = async () => {
    setRunningApps(await apiListRunningApps());
  };

  useEffect(() => {
    void loadRunningApps();
  }, []);

  if (!state) return null;
  const { config, active_rule_app, night_mode_active } = state;
  const auto = config.automation;

  const patchAuto = (partial: Partial<typeof auto>) => {
    void updateAutomation({ ...auto, ...partial });
  };

  const handleAddRule = () => {
    const clean = exeInput.trim().toLowerCase();
    if (!clean) return;
    const normalized = clean.endsWith(".exe") ? clean : `${clean}.exe`;
    if (auto.app_rules.some((r) => r.executable === normalized)) return;
    patchAuto({
      app_rules: [...auto.app_rules, { id: `rule-${Date.now()}`, executable: normalized, profile_id: ruleProfileId, enabled: true }],
    });
    setExeInput("");
  };

  const pct = (v: string) => Math.max(1, Math.min(99, Number(v) || 1));

  return (
    <div className="fade-in flex h-full flex-col gap-4 overflow-y-auto pr-1">
      <div className="flex flex-wrap items-end justify-between gap-2">
        <div>
          <h2 className="font-display text-lg font-semibold text-zinc-50">{t.automations.title}</h2>
          <p className="text-xs text-zinc-400">{t.automations.subtitle}</p>
        </div>
        {active_rule_app && (
          <span className="flex items-center gap-1.5 rounded-lg border border-white/12 bg-white/[0.05] px-2.5 py-1 text-[11px] text-zinc-200">
            <Zap className="h-3 w-3 live-text" />
            {t.automations.activeFocusRule}: <span className="font-mono">{active_rule_app}</span>
          </span>
        )}
      </div>

      <Panel title={t.automations.powerSection}>
        <div className="flex flex-col gap-3">
          <SettingRow first title={t.automations.autoOffOnLock} desc={t.automations.autoOffOnLockDesc}>
            <Toggle checked={auto.auto_off_on_lock} onChange={(v) => patchAuto({ auto_off_on_lock: v })} />
          </SettingRow>

          <SettingRow title={t.automations.idleTimeout} desc={t.automations.idleTimeoutDesc}>
            <select value={auto.idle_timeout_mins} onChange={(e) => patchAuto({ idle_timeout_mins: Number(e.target.value) })} className={field}>
              <option value={0}>{t.automations.disabled}</option>
              {[5, 15, 30, 60].map((m) => (
                <option key={m} value={m}>
                  {m} {t.automations.minutes}
                </option>
              ))}
            </select>
          </SettingRow>

          <SettingRow
            title={
              <span className="flex items-center gap-2">
                {t.automations.nightMode}
                {night_mode_active && <Moon className="h-3.5 w-3.5 text-amber-300" />}
              </span>
            }
            desc={t.automations.nightModeDesc}
          >
            <div className="flex items-center gap-2 text-xs">
              <input type="time" value={auto.night_start} onChange={(e) => patchAuto({ night_start: e.target.value })} className={field} />
              <span className="text-zinc-500">→</span>
              <input type="time" value={auto.night_end} onChange={(e) => patchAuto({ night_end: e.target.value })} className={field} />
              <span className="ml-1">
                <Toggle checked={auto.night_mode_enabled} onChange={(v) => patchAuto({ night_mode_enabled: v })} />
              </span>
            </div>
          </SettingRow>

          {auto.night_mode_enabled && (
            <div className="rounded-xl border border-white/6 bg-black/20 p-3">
              <Slider
                label={t.automations.nightBrightness}
                value={auto.night_brightness}
                min={5}
                max={100}
                display={`${auto.night_brightness}%`}
                onChange={(v) => patchAuto({ night_brightness: v })}
              />
            </div>
          )}
        </div>
      </Panel>

      <Panel
        title={t.automations.appRulesTitle}
        desc={t.automations.appRulesDesc}
        actions={
          <button
            type="button"
            onClick={() => void loadRunningApps()}
            className="studio-btn flex shrink-0 items-center gap-1.5 rounded-lg border border-white/10 bg-white/5 px-2.5 py-1 text-[11px] text-zinc-300 hover:bg-white/10"
          >
            <RefreshCw className="h-3 w-3" />
            {t.automations.refreshApps}
          </button>
        }
      >
        <div className="flex flex-col gap-3">
          <div className="flex flex-wrap items-center gap-1.5">
            <span className="text-[11px] text-zinc-500">{t.automations.pickRunningApp}:</span>
            {runningApps.length === 0 ? (
              <span className="text-[11px] text-zinc-500">{t.automations.noRunningApps}</span>
            ) : (
              runningApps.slice(0, 10).map((app) => (
                <button
                  key={app.executable}
                  type="button"
                  onClick={() => setExeInput(app.executable)}
                  title={app.title}
                  className="studio-btn rounded-md border border-white/10 bg-white/[0.04] px-2 py-0.5 font-mono text-[11px] text-zinc-300 hover:border-white/25 hover:text-zinc-100"
                >
                  {app.executable}
                </button>
              ))
            )}
          </div>

          <div className="flex flex-wrap gap-2">
            <input
              type="text"
              value={exeInput}
              onChange={(e) => setExeInput(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && handleAddRule()}
              placeholder={t.automations.executablePlaceholder}
              className={`min-w-44 flex-1 ${field} placeholder:text-zinc-500`}
            />
            <select value={ruleProfileId} onChange={(e) => setRuleProfileId(e.target.value)} className={field}>
              {config.profiles.map((p) => (
                <option key={p.id} value={p.id}>
                  {t.presets[p.id] ?? p.name}
                </option>
              ))}
            </select>
            <button type="button" onClick={handleAddRule} className="studio-btn btn-primary flex items-center gap-1.5 rounded-lg px-3 py-1.5 text-xs font-semibold">
              <Plus className="h-3.5 w-3.5" />
              {t.automations.addRule}
            </button>
          </div>

          <div className="space-y-1.5">
            {auto.app_rules.map((rule) => (
              <div
                key={rule.id}
                className={`flex items-center justify-between rounded-xl border px-3 py-2 text-xs ${
                  active_rule_app && rule.executable === active_rule_app.toLowerCase()
                    ? "border-white/25 bg-white/[0.06]"
                    : "border-white/8 bg-black/20"
                }`}
              >
                <div className="flex items-center gap-3">
                  <Toggle
                    small
                    checked={rule.enabled}
                    onChange={(v) => patchAuto({ app_rules: auto.app_rules.map((r) => (r.id === rule.id ? { ...r, enabled: v } : r)) })}
                  />
                  <span className="font-mono text-zinc-200">{rule.executable}</span>
                  <span className="text-zinc-500">→</span>
                  <select
                    value={rule.profile_id}
                    onChange={(e) =>
                      patchAuto({ app_rules: auto.app_rules.map((r) => (r.id === rule.id ? { ...r, profile_id: e.target.value } : r)) })
                    }
                    className="rounded-md border border-white/10 bg-white/[0.06] px-1.5 py-0.5 text-zinc-200"
                  >
                    {config.profiles.map((p) => (
                      <option key={p.id} value={p.id}>
                        {t.presets[p.id] ?? p.name}
                      </option>
                    ))}
                  </select>
                </div>
                <button
                  type="button"
                  onClick={() => patchAuto({ app_rules: auto.app_rules.filter((r) => r.id !== rule.id) })}
                  className="text-zinc-500 hover:text-rose-400"
                  aria-label={t.automations.remove}
                >
                  <Trash2 className="h-3.5 w-3.5" />
                </button>
              </div>
            ))}
          </div>
        </div>
      </Panel>

      <Panel title={t.automations.batteryAlertsTitle}>
        <div className="flex flex-wrap gap-3">
          <label className="flex items-center gap-2 text-xs text-zinc-300">
            {t.automations.lowWarn}
            <input
              type="number"
              min={1}
              max={99}
              value={auto.low_battery_warn_pct}
              onChange={(e) => patchAuto({ low_battery_warn_pct: pct(e.target.value) })}
              className={`w-16 text-right tabular-nums ${field}`}
            />
          </label>
          <label className="flex items-center gap-2 text-xs text-zinc-300">
            {t.automations.lowCrit}
            <input
              type="number"
              min={1}
              max={99}
              value={auto.low_battery_crit_pct}
              onChange={(e) => patchAuto({ low_battery_crit_pct: pct(e.target.value) })}
              className={`w-16 text-right tabular-nums ${field}`}
            />
          </label>
        </div>
      </Panel>
    </div>
  );
}
