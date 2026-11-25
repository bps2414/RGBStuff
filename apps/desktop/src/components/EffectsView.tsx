import { EFFECT_META, EFFECT_ORDER, effectBackground } from "../lib/visuals";
import { useAppStore } from "../store/useAppStore";
import { EffectControls, SceneGrid, useActiveProfile } from "./EffectWidgets";
import { Panel } from "./ui";

export function EffectsView() {
  const { state, t, updateEffect } = useAppStore();
  const profile = useActiveProfile();
  if (!state || !profile) return null;
  const on = state.config.power_on;
  const profileName = t.presets[profile.id] ?? profile.name;

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
          <div className="grid grid-cols-2 gap-2.5 md:grid-cols-3">
            {EFFECT_ORDER.map((kind) => {
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
            })}
          </div>
        </Panel>

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
      </div>
    </div>
  );
}
