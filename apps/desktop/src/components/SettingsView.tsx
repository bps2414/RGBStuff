import { useState } from "react";
import { BatteryMedium, Download, LogOut, RefreshCw } from "lucide-react";
import { apiCheckForUpdate, apiInstallUpdate, apiQuitApp } from "../lib/tauri";
import { useAppStore } from "../store/useAppStore";
import { Panel, Segmented, SettingRow, Toggle } from "./ui";

export function SettingsView() {
  const { state, t, updateAutomation, updateUiPrefs, setLocale } = useAppStore();
  const [updStatus, setUpdStatus] = useState<"idle" | "checking" | "latest" | "error" | "installing">("idle");
  if (!state) return null;
  const { config } = state;
  const batteryDevices = state.devices.filter((d) => d.capabilities.includes("battery"));
  const update = state.available_update;

  const toggleTrayBattery = (id: string, on: boolean) => {
    const rest = config.ui.tray_battery_devices.filter((d) => d !== id);
    updateUiPrefs({ ...config.ui, tray_battery_devices: on ? [...rest, id] : rest });
  };
  const checkNow = async () => {
    setUpdStatus("checking");
    try {
      const found = await apiCheckForUpdate();
      setUpdStatus(found ? "idle" : "latest");
    } catch {
      setUpdStatus("error");
    }
  };
  const install = async () => {
    setUpdStatus("installing");
    try {
      await apiInstallUpdate();
    } catch {
      setUpdStatus("error");
    }
  };
  const btn =
    "studio-btn flex items-center gap-1.5 rounded-lg border border-white/10 bg-white/[0.05] px-3 py-1.5 text-xs font-medium text-zinc-200 hover:bg-white/[0.09] disabled:opacity-50";

  return (
    <div className="fade-in flex h-full flex-col gap-4 overflow-y-auto pr-1">
      <div>
        <h2 className="font-display text-lg font-semibold text-zinc-50">{t.settings.title}</h2>
        <p className="text-xs text-zinc-400">{t.settings.subtitle}</p>
      </div>

      <Panel title={t.settings.startup}>
        <div className="flex flex-col gap-3">
          <SettingRow first title={t.settings.startWithWindows} desc={t.settings.startWithWindowsDesc}>
            <Toggle
              checked={config.automation.start_with_windows}
              onChange={(v) => updateAutomation({ ...config.automation, start_with_windows: v })}
            />
          </SettingRow>
          <SettingRow title={t.settings.closeAction} desc={t.settings.closeActionDesc}>
            <Segmented
              value={config.ui.close_action}
              options={[
                { value: "tray", label: t.settings.closeToTray },
                { value: "quit", label: t.settings.closeQuit },
              ]}
              onChange={(v) => updateUiPrefs({ ...config.ui, close_action: v })}
            />
          </SettingRow>
        </div>
      </Panel>

      <Panel title={t.settings.interface}>
        <div className="flex flex-col gap-3">
          <SettingRow first title={t.settings.languageLabel} desc={t.settings.languageDesc}>
            <Segmented
              value={config.locale}
              options={[
                { value: "pt-BR", label: "Português (BR)" },
                { value: "en-US", label: "English (US)" },
              ]}
              onChange={(v) => setLocale(v)}
            />
          </SettingRow>
          <SettingRow title={t.settings.shortcuts} desc={t.settings.shortcutsDesc}>
            <span />
          </SettingRow>
          <SettingRow title={t.settings.trayBattery} desc={t.settings.trayBatteryDesc}>
            <div className="flex flex-col items-end gap-2">
              {batteryDevices.length === 0 && (
                <span className="text-[11px] text-zinc-500">{t.settings.noBatteryDevices}</span>
              )}
              {batteryDevices.map((d) => {
                const tel = d.telemetry;
                const pct = tel?.connected && tel.percent != null ? `${tel.percent}%` : "—";
                return (
                  <label key={d.id} className="flex cursor-pointer items-center gap-2 text-xs text-zinc-300">
                    <BatteryMedium className="h-3.5 w-3.5 text-zinc-500" />
                    <span>{d.name}</span>
                    <span className="font-mono text-[11px] text-zinc-500">{pct}</span>
                    <Toggle
                      small
                      checked={config.ui.tray_battery_devices.includes(d.id)}
                      onChange={(v) => toggleTrayBattery(d.id, v)}
                    />
                  </label>
                );
              })}
            </div>
          </SettingRow>
        </div>
      </Panel>

      <Panel title={t.settings.updates}>
        <div className="flex flex-col gap-3">
          <SettingRow first title={t.settings.autoUpdate} desc={t.settings.autoUpdateDesc}>
            <Toggle
              checked={config.ui.auto_update_check}
              onChange={(v) => updateUiPrefs({ ...config.ui, auto_update_check: v })}
            />
          </SettingRow>
          <SettingRow
            title={update ? t.settings.updateAvailable.replace("{v}", update.version) : `RGBStuff ${state.app_version}`}
            desc={
              updStatus === "installing"
                ? t.settings.installing
                : updStatus === "error"
                  ? t.settings.updateError
                  : update
                    ? (update.notes ?? "").split("\n").find((l) => l.trim() && !l.startsWith("!["))?.replace(/^#+\s*/, "") ?? ""
                    : updStatus === "latest"
                      ? t.settings.upToDate
                      : ""
            }
          >
            {update ? (
              <button
                type="button"
                disabled={updStatus === "installing"}
                onClick={install}
                className="studio-btn flex items-center gap-1.5 rounded-lg border border-emerald-400/30 bg-emerald-500/10 px-3 py-1.5 text-xs font-medium text-emerald-200 hover:bg-emerald-500/20 disabled:opacity-50"
              >
                <Download className="h-3.5 w-3.5" />
                {updStatus === "installing" ? t.settings.installing : t.settings.installUpdate}
              </button>
            ) : (
              <button type="button" disabled={updStatus === "checking"} onClick={checkNow} className={btn}>
                <RefreshCw className={`h-3.5 w-3.5 ${updStatus === "checking" ? "animate-spin" : ""}`} />
                {updStatus === "checking" ? t.settings.checking : t.settings.checkNow}
              </button>
            )}
          </SettingRow>
        </div>
      </Panel>

      <Panel title={t.settings.about}>
        <div className="flex flex-col gap-3">
          <SettingRow first title={`RGBStuff ${state.app_version}`} desc={t.settings.aboutDesc}>
            <span className="rounded-md border border-white/10 bg-white/[0.04] px-2 py-1 font-mono text-[11px] text-zinc-300">
              {t.settings.version} {state.app_version}
            </span>
          </SettingRow>
          <SettingRow title={t.app.quit} desc={t.settings.quitDesc}>
            <button
              type="button"
              onClick={() => apiQuitApp()}
              className="studio-btn flex items-center gap-1.5 rounded-lg border border-rose-400/30 bg-rose-500/10 px-3 py-1.5 text-xs font-medium text-rose-200 hover:bg-rose-500/20"
            >
              <LogOut className="h-3.5 w-3.5" />
              {t.app.quit}
            </button>
          </SettingRow>
        </div>
      </Panel>
    </div>
  );
}
