import { Check } from "lucide-react";
import type { LightingProfile } from "../lib/tauri";
import { EFFECT_META, SCENES, effectBackground, sceneMatches } from "../lib/visuals";
import { useAppStore } from "../store/useAppStore";
import { ColorField, Slider } from "./ui";

export function useActiveProfile(): LightingProfile | null {
  const state = useAppStore((s) => s.state);
  if (!state) return null;
  return state.config.profiles.find((p) => p.id === state.config.active_profile_id) ?? state.config.profiles[0] ?? null;
}

/** One-click looks. `compact` = popover layout. */
export function SceneGrid({ compact, limit }: { compact?: boolean; limit?: number }) {
  const { state, updateEffect } = useAppStore();
  const profile = useActiveProfile();
  if (!state || !profile) return null;
  const locale = state.config.locale;
  const on = state.config.power_on;

  return (
    <div className={`grid gap-2 ${compact ? "grid-cols-3" : "grid-cols-3 sm:grid-cols-4 xl:grid-cols-6"}`}>
      {SCENES.slice(0, limit).map((s) => {
        const active = on && sceneMatches(s, profile.effect);
        const live = !EFFECT_META[s.effect.kind].isStatic;
        return (
          <button
            key={s.id}
            type="button"
            onClick={() => updateEffect(s.effect)}
            data-active={active}
            className={`studio-card studio-btn group relative overflow-hidden rounded-xl text-left ${compact ? "h-14" : "h-20"}`}
          >
            <span
              className="fx-thumb absolute inset-0 opacity-80 transition-opacity group-hover:opacity-100"
              data-live={live}
              style={{ background: effectBackground(s.effect) }}
            />
            <span className="absolute inset-0 bg-gradient-to-t from-black/75 via-black/10 to-transparent" />
            <span className={`absolute bottom-1.5 left-2 right-2 flex items-center justify-between font-medium text-white ${compact ? "text-[11px]" : "text-xs"}`}>
              <span className="truncate drop-shadow">{s.name[locale]}</span>
              {active && <Check className="h-3.5 w-3.5 shrink-0" />}
            </span>
          </button>
        );
      })}
    </div>
  );
}

/** Only the controls that change the current effect. */
export function EffectControls({ columns = 2 }: { columns?: 1 | 2 }) {
  const { t, updateEffect } = useAppStore();
  const profile = useActiveProfile();
  if (!profile) return null;
  const e = profile.effect;
  const meta = EFFECT_META[e.kind];
  const isAmbi = e.kind === "ambilight";
  const hasAny = meta.primary || meta.secondary || meta.speed || meta.angle || meta.scale;

  return (
    <div className="flex flex-col gap-4">
      {(meta.primary || meta.secondary) && (
        <div className="grid grid-cols-2 gap-2">
          {meta.primary && (
            <ColorField label={t.effects.primaryColor} value={e.primary_color} onChange={(c) => updateEffect({ ...e, primary_color: c })} />
          )}
          {meta.secondary && (
            <ColorField label={t.effects.secondaryColor} value={e.secondary_color} onChange={(c) => updateEffect({ ...e, secondary_color: c })} />
          )}
        </div>
      )}

      <div className={`grid gap-4 ${columns === 2 ? "sm:grid-cols-2" : ""}`}>
        {meta.speed && (
          <Slider
            label={isAmbi ? t.effects.responsiveness : t.effects.speed}
            value={e.speed}
            min={1}
            max={100}
            display={`${e.speed}%`}
            onChange={(v) => updateEffect({ ...e, speed: v })}
          />
        )}
        {meta.scale && (
          <Slider
            label={isAmbi ? t.effects.unify : t.effects.scale}
            value={e.scale}
            min={20}
            max={200}
            display={`${e.scale}%`}
            onChange={(v) => updateEffect({ ...e, scale: v })}
          />
        )}
        {meta.angle && (
          <Slider
            label={t.effects.angle}
            value={e.angle_deg}
            min={0}
            max={360}
            display={`${Math.round(e.angle_deg)}°`}
            onChange={(v) => updateEffect({ ...e, angle_deg: v })}
          />
        )}
        {e.kind !== "off" && (
          <Slider
            label={t.effects.profileBrightness}
            value={profile.brightness}
            display={`${profile.brightness}%`}
            onChange={(v) => updateEffect(e, v)}
          />
        )}
      </div>

      {!hasAny && e.kind === "off" && <p className="text-xs text-zinc-500">{t.effects.noParams}</p>}
    </div>
  );
}
