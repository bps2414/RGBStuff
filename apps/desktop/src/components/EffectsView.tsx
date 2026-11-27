import { Keyboard, Zap } from "lucide-react";
import type { EffectKind } from "../i18n";
import type { KeyboardHwEffect } from "../lib/tauri";
import { EFFECT_GROUPS, EFFECT_META, KEYBOARD_MODES, effectBackground } from "../lib/visuals";
import { useAppStore } from "../store/useAppStore";
import { EffectControls, SceneGrid, useActiveProfile } from "./EffectWidgets";
import { ColorField, Panel, Slider, Toggle } from "./ui";

export function EffectsView() {
  const { state, t, updateEffect } = useAppStore();
  const profile = useActiveProfile();
  if (!state || !profile) return null;
  const on = state.config.power_on;
  const profileName = t.presets[profile.id] ?? profile.name;
  const hasKeyboard = state.devices.some((d) => d.kind === "keyboard" && d.connected);

  const card = (kind: EffectKind) => {
    const active = on && profile.effect.kind === kind;
    const isStatic = EFFECT_META[kind].isStatic;
    const copy = t.effects.items[kind];
    return (
      <button
        key={kind}
        type="button"
        data-active={active}
        onClick={() => updateEffect({ ...profile.effect, kind })}
        title={copy.description}
        className="studio-card studio-btn flex flex-col overflow-hidden rounded-xl text-left"
      >
        <span
          className="fx-thumb h-12 w-full"
          data-live={!isStatic}
          style={{ background: effectBackground({ ...profile.effect, kind }) }}
        />
        <span className="flex items-center justify-between gap-2 px-3 py-2">
          <span className="truncate text-xs font-medium text-zinc-100">{copy.name}</span>
          <span
            className={`shrink-0 rounded px-1.5 py-0.5 text-[9px] font-semibold uppercase tracking-wide ${
              isStatic ? "bg-emerald-500/12 text-emerald-300" : "bg-white/8 text-zinc-300"
            }`}
          >
            {isStatic ? t.engine.zeroCpuBadge : t.engine.liveCanvasBadge}
          </span>
        </span>
      </button>
    );
  };

  return (
    <div className="fade-in flex h-full flex-col gap-4 overflow-y-auto pr-1">
      <div>
        <h2 className="font-display text-lg font-semibold text-zinc-50">{t.effects.title}</h2>
        <p className="text-xs text-zinc-400">{t.effects.subtitle}</p>
      </div>

      <Panel title={t.effects.scenesTitle} desc={t.effects.scenesDesc}>
        <SceneGrid />
      </Panel>

      <div className="grid min-h-0 grid-cols-1 gap-4 xl:grid-cols-[1fr_340px]">
        <Panel title={t.effects.allEffects}>
          <div className="flex flex-col gap-4">
            {EFFECT_GROUPS.map((g) => (
              <div key={g.id}>
                <div className="mb-2 text-[10px] font-semibold uppercase tracking-[0.08em] text-zinc-500">
                  {t.effects.groups[g.id]}
                </div>
                <div className="grid grid-cols-2 gap-2.5 md:grid-cols-3 2xl:grid-cols-4">{g.kinds.map(card)}</div>
              </div>
            ))}
          </div>
        </Panel>

        <div className="flex flex-col gap-4">
          <Panel
            title={t.effects.parameters}
            desc={
              <>
                {t.effects.editing}: <span className="font-medium text-zinc-200">{profileName}</span>
              </>
            }
          >
            <div className="mb-4 rounded-xl border border-white/8 bg-black/25 p-3">
              <div className="text-sm font-medium text-zinc-100">{t.effects.items[profile.effect.kind].name}</div>
              <p className="mt-1 text-xs leading-relaxed text-zinc-400">{t.effects.items[profile.effect.kind].description}</p>
            </div>
            <EffectControls columns={1} />
          </Panel>

          {hasKeyboard && <KeyboardFxPanel />}
        </div>
      </div>
    </div>
  );
}

const DEFAULT_KB_FX: KeyboardHwEffect = {
  mode: 15,
  color: { r: 56, g: 189, b: 248 },
  colorful: true,
  speed: 3,
  reverse: false,
};

/** Hands the keyboard over to one of its own firmware animations. */
function KeyboardFxPanel() {
  const { t, updateKeyboardFx } = useAppStore();
  const profile = useActiveProfile();
  if (!profile) return null;
  const fx = profile.keyboard_fx ?? null;
  const meta = KEYBOARD_MODES.find((m) => m.mode === fx?.mode);
  const kb = t.effects.keyboard;

  const pick = (mode: number | null) => {
    if (mode === null) return updateKeyboardFx(null);
    updateKeyboardFx({ ...(fx ?? DEFAULT_KB_FX), mode });
  };

  return (
    <Panel
      title={
        <span className="flex items-center gap-1.5">
          <Keyboard className="h-3.5 w-3.5" />
          {kb.title}
        </span>
      }
      desc={kb.desc}
      actions={
        fx && (
          <span className="shrink-0 rounded bg-amber-400/12 px-1.5 py-0.5 text-[9px] font-semibold uppercase tracking-wide text-amber-300">
            {kb.activeBadge}
          </span>
        )
      }
    >
      <div className="grid grid-cols-2 gap-1.5">
        <button
          type="button"
          data-active={fx === null}
          onClick={() => pick(null)}
          className="studio-card studio-btn col-span-2 rounded-lg px-3 py-2 text-left"
        >
          <div className="text-xs font-medium text-zinc-100">{kb.followCanvas}</div>
          <div className="text-[11px] text-zinc-500">{kb.followCanvasDesc}</div>
        </button>
        {KEYBOARD_MODES.map((m) => (
          <button
            key={m.mode}
            type="button"
            data-active={fx?.mode === m.mode}
            onClick={() => pick(m.mode)}
            title={m.reactive ? kb.reactiveHint : undefined}
            className="studio-card studio-btn flex items-center justify-between gap-1 rounded-lg px-2.5 py-1.5 text-left"
          >
            <span className="truncate text-[11px] font-medium text-zinc-200">{kb.modes[m.mode]}</span>
            {m.reactive && <Zap className="h-3 w-3 shrink-0 text-amber-300" />}
          </button>
        ))}
      </div>

      {fx && (
        <div className="mt-4 flex flex-col gap-4">
          {!meta?.noColor && (
            <>
              <label className="flex items-center justify-between gap-3 text-xs text-zinc-300">
                {kb.colorful}
                <Toggle checked={fx.colorful} onChange={(v) => updateKeyboardFx({ ...fx, colorful: v })} />
              </label>
              {!fx.colorful && (
                <ColorField label={kb.color} value={fx.color} onChange={(c) => updateKeyboardFx({ ...fx, color: c })} />
              )}
            </>
          )}
          <Slider
            label={kb.speed}
            value={fx.speed}
            min={1}
            max={5}
            display={`${fx.speed}/5`}
            onChange={(v) => updateKeyboardFx({ ...fx, speed: v })}
          />
          {meta?.direction && (
            <label className="flex items-center justify-between gap-3 text-xs text-zinc-300">
              {kb.reverse}
              <Toggle checked={fx.reverse} onChange={(v) => updateKeyboardFx({ ...fx, reverse: v })} />
            </label>
          )}
        </div>
      )}
    </Panel>
  );
}
