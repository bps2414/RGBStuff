import * as Switch from "@radix-ui/react-switch";
import { Plus, RefreshCw, Trash2 } from "lucide-react";
import { useEffect, useState } from "react";
import { apiListRunningApps, type RunningAppInfo } from "../lib/tauri";
import { useAppStore } from "../store/useAppStore";

export function AutomationsView() {
  const { state, t, updateAutomation, setLocale } = useAppStore();
  const [exeInput, setExeInput] = useState("");
  const [ruleProfileId, setRuleProfileId] = useState("gaming");
  const [runningApps, setRunningApps] = useState<RunningAppInfo[]>([]);

  const loadRunningApps = async () => {
    const apps = await apiListRunningApps();
    setRunningApps(apps);
  };

  useEffect(() => {
    void loadRunningApps();
  }, []);

  if (!state) return null;
  const { config, active_rule_app } = state;
  const auto = config.automation;

  const patchAuto = (partial: Partial<typeof auto>) => {
    void updateAutomation({ ...auto, ...partial });
  };

  const handleAddRule = () => {
    const clean = exeInput.trim().toLowerCase();
    if (!clean) return;
    const normalized = clean.endsWith(".exe") ? clean : `${clean}.exe`;
    patchAuto({
      app_rules: [
        ...auto.app_rules,
        {
          id: `rule-${Date.now()}`,
          executable: normalized,
          profile_id: ruleProfileId,
          enabled: true,
        },
      ],
    });
    setExeInput("");
  };

  return (
    <div className="flex h-full flex-col gap-4 overflow-y-auto pr-1">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <div>
          <h2 className="text-base font-semibold text-zinc-100">{t.automations.title}</h2>
          <p className="text-xs text-zinc-400">{t.automations.subtitle}</p>
        </div>
        {active_rule_app && (
          <span className="rounded-lg border border-sky-400/35 bg-sky-400/12 px-2.5 py-1 text-[11px] font-medium text-sky-300">
            {t.automations.activeFocusRule}: <span className="font-mono">{active_rule_app}</span>
          </span>
        )}
      </div>

      {/* System & Language */}
      <div className="studio-panel flex flex-col gap-3 rounded-2xl p-4">
        <div className="text-xs font-semibold uppercase tracking-wider text-zinc-400">
          {t.automations.systemSection}
        </div>

        <div className="flex items-center justify-between gap-4">
          <div>
            <div className="text-sm font-medium text-zinc-100">{t.automations.languageLabel}</div>
            <div className="text-xs text-zinc-400">{t.automations.languageDesc}</div>
          </div>
          <div className="flex rounded-lg border border-white/10 bg-black/30 p-0.5">
            {(["pt-BR", "en-US"] as const).map((loc) => (
              <button
                key={loc}
                type="button"
                onClick={() => setLocale(loc)}
                className={`rounded-md px-3 py-1 text-xs font-medium transition-colors ${
                  config.locale === loc
                    ? "bg-zinc-100 text-zinc-950"
                    : "text-zinc-400 hover:text-zinc-200"
                }`}
              >
                {loc === "pt-BR" ? "Português (BR)" : "English (US)"}
              </button>
            ))}
          </div>
        </div>

        <div className="flex items-center justify-between gap-4 border-t border-white/6 pt-3">
          <div>
            <div className="text-sm font-medium text-zinc-100">
              {t.automations.startWithWindows}
            </div>
            <div className="text-xs text-zinc-400">{t.automations.startWithWindowsDesc}</div>
          </div>
          <Switch.Root
            checked={auto.start_with_windows}
            onCheckedChange={(v) => patchAuto({ start_with_windows: v })}
            className="relative h-5 w-9 cursor-pointer rounded-full bg-zinc-700 transition-colors data-[state=checked]:bg-sky-400"
          >
            <Switch.Thumb className="block h-4 w-4 translate-x-0.5 rounded-full bg-white transition-transform data-[state=checked]:translate-x-4.5" />
          </Switch.Root>
        </div>
      </div>

      {/* Power & Night Mode */}
      <div className="studio-panel flex flex-col gap-3 rounded-2xl p-4">
        <div className="text-xs font-semibold uppercase tracking-wider text-zinc-400">
          {t.automations.powerSection}
        </div>

        <div className="flex items-center justify-between gap-4">
          <div>
            <div className="text-sm font-medium text-zinc-100">{t.automations.autoOffOnLock}</div>
            <div className="text-xs text-zinc-400">{t.automations.autoOffOnLockDesc}</div>
          </div>
          <Switch.Root
            checked={auto.auto_off_on_lock}
            onCheckedChange={(v) => patchAuto({ auto_off_on_lock: v })}
            className="relative h-5 w-9 cursor-pointer rounded-full bg-zinc-700 transition-colors data-[state=checked]:bg-sky-400"
          >
            <Switch.Thumb className="block h-4 w-4 translate-x-0.5 rounded-full bg-white transition-transform data-[state=checked]:translate-x-4.5" />
          </Switch.Root>
        </div>

        <div className="flex items-center justify-between gap-4 border-t border-white/6 pt-3">
          <div>
            <div className="text-sm font-medium text-zinc-100">{t.automations.idleTimeout}</div>
            <div className="text-xs text-zinc-400">{t.automations.idleTimeoutDesc}</div>
          </div>
          <select
            value={auto.idle_timeout_mins}
            onChange={(e) => patchAuto({ idle_timeout_mins: Number(e.target.value) })}
            className="rounded-lg border border-white/12 bg-zinc-900 px-2.5 py-1 text-xs text-zinc-100"
          >
            <option value={0}>{t.automations.disabled}</option>
            <option value={5}>5 {t.automations.minutes}</option>
            <option value={15}>15 {t.automations.minutes}</option>
            <option value={30}>30 {t.automations.minutes}</option>
            <option value={60}>60 {t.automations.minutes}</option>
          </select>
        </div>

        <div className="flex flex-wrap items-center justify-between gap-4 border-t border-white/6 pt-3">
          <div>
            <div className="text-sm font-medium text-zinc-100">{t.automations.nightMode}</div>
            <div className="text-xs text-zinc-400">{t.automations.nightModeDesc}</div>
          </div>
          <div className="flex items-center gap-2 text-xs">
            <input
              type="time"
              value={auto.night_start}
              onChange={(e) => patchAuto({ night_start: e.target.value })}
              className="rounded-lg border border-white/12 bg-zinc-900 px-2 py-1 text-zinc-100"
            />
            <span className="text-zinc-500">→</span>
            <input
              type="time"
              value={auto.night_end}
              onChange={(e) => patchAuto({ night_end: e.target.value })}
              className="rounded-lg border border-white/12 bg-zinc-900 px-2 py-1 text-zinc-100"
            />
            <Switch.Root
              checked={auto.night_mode_enabled}
              onCheckedChange={(v) => patchAuto({ night_mode_enabled: v })}
              className="relative ml-2 h-5 w-9 cursor-pointer rounded-full bg-zinc-700 transition-colors data-[state=checked]:bg-sky-400"
            >
              <Switch.Thumb className="block h-4 w-4 translate-x-0.5 rounded-full bg-white transition-transform data-[state=checked]:translate-x-4.5" />
            </Switch.Root>
          </div>
        </div>
      </div>

      {/* App Focus Rules + Running Apps Picker */}
      <div className="studio-panel flex flex-col gap-3 rounded-2xl p-4">
        <div className="flex items-start justify-between gap-3">
          <div>
            <div className="text-xs font-semibold uppercase tracking-wider text-zinc-400">
              {t.automations.appRulesTitle}
            </div>
            <div className="mt-0.5 text-xs text-zinc-400">{t.automations.appRulesDesc}</div>
          </div>
          <button
            type="button"
            onClick={() => void loadRunningApps()}
            className="studio-btn flex shrink-0 items-center gap-1.5 rounded-lg border border-white/10 bg-white/5 px-2.5 py-1 text-[11px] text-zinc-300 hover:bg-white/10"
          >
            <RefreshCw className="h-3 w-3" />
            <span>{t.automations.refreshApps}</span>
          </button>
        </div>

        {/* Running open apps chips */}
        <div className="flex flex-wrap items-center gap-1.5">
          <span className="text-[11px] text-zinc-500">{t.automations.pickRunningApp}:</span>
          {runningApps.length === 0 ? (
            <span className="text-[11px] text-zinc-500">{t.automations.noRunningApps}</span>
          ) : (
            runningApps.slice(0, 8).map((app) => (
              <button
                key={app.executable}
                type="button"
                onClick={() => setExeInput(app.executable)}
                title={app.title}
                className="studio-btn rounded-md border border-white/10 bg-white/[0.04] px-2 py-0.5 font-mono text-[11px] text-zinc-300 hover:border-sky-400/40 hover:text-zinc-100"
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
            placeholder={t.automations.executablePlaceholder}
            className="min-w-44 flex-1 rounded-lg border border-white/12 bg-zinc-900 px-3 py-1.5 text-xs text-zinc-100 placeholder:text-zinc-500 focus:border-sky-400 focus:outline-none"
          />
          <select
            value={ruleProfileId}
            onChange={(e) => setRuleProfileId(e.target.value)}
            className="rounded-lg border border-white/12 bg-zinc-900 px-2.5 py-1.5 text-xs text-zinc-100"
          >
            {config.profiles.map((p) => (
              <option key={p.id} value={p.id}>
                {t.presets[p.id] ?? p.name}
              </option>
            ))}
          </select>
          <button
            type="button"
            onClick={handleAddRule}
            className="studio-btn flex items-center gap-1.5 rounded-lg bg-zinc-100 px-3 py-1.5 text-xs font-semibold text-zinc-950 hover:bg-white"
          >
            <Plus className="h-3.5 w-3.5" />
            <span>{t.automations.addRule}</span>
          </button>
        </div>

        <div className="space-y-1.5">
          {auto.app_rules.map((rule) => (
            <div
              key={rule.id}
              className="flex items-center justify-between rounded-xl border border-white/8 bg-black/20 px-3 py-2 text-xs"
            >
              <div className="flex items-center gap-3">
                <Switch.Root
                  checked={rule.enabled}
                  onCheckedChange={(v) =>
                    patchAuto({
                      app_rules: auto.app_rules.map((r) =>
                        r.id === rule.id ? { ...r, enabled: v } : r,
                      ),
                    })
                  }
                  className="relative h-4 w-7 cursor-pointer rounded-full bg-zinc-700 transition-colors data-[state=checked]:bg-sky-400"
                >
                  <Switch.Thumb className="block h-3 w-3 translate-x-0.5 rounded-full bg-white transition-transform data-[state=checked]:translate-x-3.5" />
                </Switch.Root>
                <span className="font-mono text-zinc-200">{rule.executable}</span>
                <span className="text-zinc-500">→</span>
                <span className="rounded bg-white/8 px-2 py-0.5 text-zinc-200">
                  {t.presets[rule.profile_id] ?? rule.profile_id}
                </span>
              </div>
              <button
                type="button"
                onClick={() =>
                  patchAuto({
                    app_rules: auto.app_rules.filter((r) => r.id !== rule.id),
                  })
                }
                className="text-zinc-500 hover:text-rose-400"
                aria-label={t.automations.remove}
              >
                <Trash2 className="h-3.5 w-3.5" />
              </button>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
