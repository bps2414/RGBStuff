import * as Slider from "@radix-ui/react-slider";
import { type EffectKind } from "../i18n";
import { hexToRgb, rgbToHex } from "../lib/tauri";
import { useAppStore } from "../store/useAppStore";

const EFFECT_ORDER: Array<{ kind: EffectKind; isStatic: boolean }> = [
  { kind: "solid", isStatic: true },
  { kind: "gradient", isStatic: true },
  { kind: "rainbow_wave", isStatic: false },
  { kind: "breathing", isStatic: false },
  { kind: "dual_pulse", isStatic: false },
  { kind: "starlight", isStatic: false },
  { kind: "ambilight", isStatic: false },
  { kind: "audio_spectrum", isStatic: false },
  { kind: "off", isStatic: true },
];

export function EffectsView() {
  const { state, t, updateEffect } = useAppStore();
  if (!state) return null;

  const activeProfile =
    state.config.profiles.find((p) => p.id === state.config.active_profile_id) ??
    state.config.profiles[0];
  if (!activeProfile) return null;

  return (
    <div className="flex h-full flex-col gap-4 overflow-y-auto pr-1">
      <div className="flex items-end justify-between">
        <div>
          <h2 className="text-base font-semibold text-zinc-100">{t.effects.title}</h2>
          <p className="text-xs text-zinc-400">{t.effects.subtitle}</p>
        </div>
      </div>

      {/* Effect Grid */}
      <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
        {EFFECT_ORDER.map(({ kind, isStatic }) => {
          const active = activeProfile.effect.kind === kind;
          const copy = t.effects.items[kind];
          return (
            <button
              key={kind}
              type="button"
              onClick={() => updateEffect({ ...activeProfile.effect, kind })}
              className={`studio-card flex flex-col justify-between rounded-2xl p-4 text-left ${
                active ? "border-zinc-100/60 bg-white/10" : ""
              }`}
            >
              <div className="flex items-start justify-between gap-2">
                <span className="text-sm font-medium text-zinc-100">{copy.name}</span>
                <span
                  className={`rounded-md px-1.5 py-0.5 text-[10px] font-medium ${
                    isStatic
                      ? "border border-emerald-500/30 bg-emerald-500/10 text-emerald-300"
                      : "border border-sky-400/30 bg-sky-400/10 text-sky-300"
                  }`}
                >
                  {isStatic ? t.engine.zeroCpuBadge : t.engine.liveCanvasBadge}
                </span>
              </div>
              <p className="mt-2 text-xs leading-relaxed text-zinc-400">{copy.description}</p>
            </button>
          );
        })}
      </div>

      {/* Parameters */}
      <div className="studio-panel grid grid-cols-1 gap-4 rounded-2xl p-4 md:grid-cols-4">
        <label className="flex flex-col gap-1.5 text-xs text-zinc-300">
          <span>{t.effects.primaryColor}</span>
          <div className="flex items-center gap-2 rounded-lg border border-white/10 bg-black/25 px-2.5 py-2">
            <input
              type="color"
              value={rgbToHex(activeProfile.effect.primary_color)}
              onChange={(e) =>
                updateEffect({
                  ...activeProfile.effect,
                  primary_color: hexToRgb(e.target.value),
                })
              }
              className="h-5 w-6 cursor-pointer border-0 bg-transparent p-0"
            />
            <span className="font-mono text-xs">{rgbToHex(activeProfile.effect.primary_color)}</span>
          </div>
        </label>

        <label className="flex flex-col gap-1.5 text-xs text-zinc-300">
          <span>{t.effects.secondaryColor}</span>
          <div className="flex items-center gap-2 rounded-lg border border-white/10 bg-black/25 px-2.5 py-2">
            <input
              type="color"
              value={rgbToHex(activeProfile.effect.secondary_color)}
              onChange={(e) =>
                updateEffect({
                  ...activeProfile.effect,
                  secondary_color: hexToRgb(e.target.value),
                })
              }
              className="h-5 w-6 cursor-pointer border-0 bg-transparent p-0"
            />
            <span className="font-mono text-xs">{rgbToHex(activeProfile.effect.secondary_color)}</span>
          </div>
        </label>

        <div className="flex flex-col justify-center">
          <div className="mb-1.5 flex justify-between text-xs text-zinc-300">
            <span>{t.effects.angle}</span>
            <span className="tabular-nums">{Math.round(activeProfile.effect.angle_deg)}°</span>
          </div>
          <Slider.Root
            className="relative flex h-5 w-full items-center"
            value={[activeProfile.effect.angle_deg]}
            min={0}
            max={360}
            onValueChange={([v]) => updateEffect({ ...activeProfile.effect, angle_deg: v })}
          >
            <Slider.Track className="relative h-1.5 w-full grow rounded-full bg-white/10">
              <Slider.Range className="absolute h-full rounded-full bg-zinc-100" />
            </Slider.Track>
            <Slider.Thumb className="block h-3.5 w-3.5 rounded-full bg-zinc-100" />
          </Slider.Root>
        </div>

        <div className="flex flex-col justify-center">
          <div className="mb-1.5 flex justify-between text-xs text-zinc-300">
            <span>{t.effects.scale}</span>
            <span className="tabular-nums">{activeProfile.effect.scale}%</span>
          </div>
          <Slider.Root
            className="relative flex h-5 w-full items-center"
            value={[activeProfile.effect.scale]}
            min={20}
            max={200}
            onValueChange={([v]) => updateEffect({ ...activeProfile.effect, scale: v })}
          >
            <Slider.Track className="relative h-1.5 w-full grow rounded-full bg-white/10">
              <Slider.Range className="absolute h-full rounded-full bg-zinc-100" />
            </Slider.Track>
            <Slider.Thumb className="block h-3.5 w-3.5 rounded-full bg-zinc-100" />
          </Slider.Root>
        </div>
      </div>
    </div>
  );
}
