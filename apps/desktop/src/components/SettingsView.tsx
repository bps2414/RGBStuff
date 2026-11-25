import { LogOut } from "lucide-react";
import { apiQuitApp } from "../lib/tauri";
import { useAppStore } from "../store/useAppStore";
import { Panel, Segmented, SettingRow, Toggle } from "./ui";

export function SettingsView() {
  const { state, t, updateAutomation, updateUiPrefs, setLocale } = useAppStore();
  if (!state) return null;
  const { config } = state;

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
