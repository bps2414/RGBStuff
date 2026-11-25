import * as Slider from "@radix-ui/react-slider";
import { useEffect, useRef, useState } from "react";
import { hexToRgb, rgbToHex, type DevicePlacement } from "../lib/tauri";
import { useAppStore } from "../store/useAppStore";

const DEVICE_LABELS: Record<string, string> = {
  "asus-aura-0b05-19af": "ASUS Aura · Fans ARGB",
  "asgard-ddr4-smbus": "Asgard DDR4 · 2x8GB",
  "aula-f75-he": "Aula F75 HE · 75%",
};

export function CanvasView() {
  const { state, canvasPixels, t, updatePlacement, updateEffect } = useAppStore();
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const stageRef = useRef<HTMLDivElement | null>(null);
  const [selectedId, setSelectedId] = useState<string>("aula-f75-he");
  const [dragging, setDragging] = useState<{
    id: string;
    startX: number;
    startY: number;
    origX: number;
    origY: number;
  } | null>(null);

  useEffect(() => {
    const el = canvasRef.current;
    if (!el || !state || canvasPixels.length < state.canvas_width * state.canvas_height * 3) return;
    const ctx = el.getContext("2d");
    if (!ctx) return;

    const w = state.canvas_width;
    const h = state.canvas_height;
    const img = ctx.createImageData(w, h);
    for (let i = 0, p = 0; i < w * h; i++, p += 3) {
      const dst = i * 4;
      img.data[dst] = canvasPixels[p] ?? 0;
      img.data[dst + 1] = canvasPixels[p + 1] ?? 0;
      img.data[dst + 2] = canvasPixels[p + 2] ?? 0;
      img.data[dst + 3] = 255;
    }
    ctx.putImageData(img, 0, 0);
  }, [canvasPixels, state]);

  if (!state) return null;
  const { config, engine_status } = state;
  const activeProfile = config.profiles.find((p) => p.id === config.active_profile_id) ?? config.profiles[0];
  const selectedPlacement =
    config.placements.find((p) => p.device_id === selectedId) ?? config.placements[0];

  const handlePointerMove = (e: React.PointerEvent<HTMLDivElement>) => {
    if (!dragging || !stageRef.current || !selectedPlacement) return;
    const rect = stageRef.current.getBoundingClientRect();
    const dx = (e.clientX - dragging.startX) / rect.width;
    const dy = (e.clientY - dragging.startY) / rect.height;
    const target = config.placements.find((p) => p.device_id === dragging.id);
    if (!target) return;

    const nx = Math.max(0, Math.min(1 - target.width, dragging.origX + dx));
    const ny = Math.max(0, Math.min(1 - target.height, dragging.origY + dy));
    void updatePlacement({ ...target, x: Number(nx.toFixed(3)), y: Number(ny.toFixed(3)) });
  };

  return (
    <div className="grid h-full grid-cols-1 gap-4 lg:grid-cols-[1fr_300px]">
      {/* Left: Spatial 2D Stage */}
      <div className="studio-panel flex flex-col rounded-2xl p-4">
        <div className="mb-3 flex items-center justify-between">
          <div>
            <h2 className="text-sm font-semibold text-zinc-100">{t.canvas.title}</h2>
            <p className="text-xs text-zinc-400">{t.canvas.subtitle}</p>
          </div>
          <span className="rounded-md border border-white/10 bg-white/[0.04] px-2 py-1 text-[11px] font-medium text-zinc-300 tabular-nums">
            {engine_status.mode === "dynamic" ? t.canvas.previewLive : t.canvas.previewStatic}
          </span>
        </div>

        <div
          ref={stageRef}
          onPointerMove={handlePointerMove}
          onPointerUp={() => setDragging(null)}
          onPointerLeave={() => setDragging(null)}
          className="relative aspect-video w-full overflow-hidden rounded-xl border border-white/10 bg-zinc-950"
        >
          <canvas
            ref={canvasRef}
            width={state.canvas_width}
            height={state.canvas_height}
            className="h-full w-full opacity-85"
            style={{ imageRendering: "auto" }}
          />
          <div
            className="pointer-events-none absolute inset-0"
            style={{
              backgroundImage:
                "linear-gradient(to right, rgba(255,255,255,0.04) 1px, transparent 1px), linear-gradient(to bottom, rgba(255,255,255,0.04) 1px, transparent 1px)",
              backgroundSize: "32px 32px",
            }}
          />

          {config.placements.map((pl: DevicePlacement) => {
            const isSelected = pl.device_id === selectedPlacement?.device_id;
            return (
              <button
                key={pl.device_id}
                type="button"
                onPointerDown={(e) => {
                  setSelectedId(pl.device_id);
                  setDragging({
                    id: pl.device_id,
                    startX: e.clientX,
                    startY: e.clientY,
                    origX: pl.x,
                    origY: pl.y,
                  });
                }}
                style={{
                  left: `${pl.x * 100}%`,
                  top: `${pl.y * 100}%`,
                  width: `${pl.width * 100}%`,
                  height: `${pl.height * 100}%`,
                }}
                className={`absolute flex cursor-grab flex-col justify-between rounded-xl border p-2.5 text-left backdrop-blur-xs active:cursor-grabbing ${
                  isSelected
                    ? "border-zinc-100/80 bg-black/45 ring-2 ring-sky-400/35"
                    : "border-white/25 bg-black/35 hover:border-white/45"
                }`}
              >
                <div className="flex items-center justify-between gap-2">
                  <span className="truncate text-xs font-medium text-zinc-100">
                    {DEVICE_LABELS[pl.device_id] ?? pl.device_id}
                  </span>
                  <span className="rounded bg-white/10 px-1.5 py-0.5 text-[10px] text-zinc-300 tabular-nums">
                    {pl.led_count} {t.canvas.leds}
                  </span>
                </div>
                <span className="text-[10px] text-zinc-400 tabular-nums">
                  {Math.round(pl.x * 100)}%, {Math.round(pl.y * 100)}%
                </span>
              </button>
            );
          })}
        </div>

        <p className="mt-2.5 text-[11px] text-zinc-500">{t.canvas.dragHint}</p>
      </div>

      {/* Right: Inspector */}
      <div className="flex flex-col gap-4">
        {selectedPlacement && (
          <div className="studio-panel rounded-2xl p-4">
            <div className="mb-3 text-xs font-semibold uppercase tracking-wider text-zinc-400">
              {t.canvas.selectedDevice}
            </div>
            <div className="mb-3 text-sm font-medium text-zinc-100">
              {DEVICE_LABELS[selectedPlacement.device_id] ?? selectedPlacement.device_id}
            </div>
            <div className="grid grid-cols-2 gap-2 text-xs">
              <div className="rounded-lg border border-white/8 bg-black/25 p-2">
                <div className="text-[10px] text-zinc-400">{t.canvas.position}</div>
                <div className="mt-0.5 font-medium text-zinc-200 tabular-nums">
                  {Math.round(selectedPlacement.x * 100)}% / {Math.round(selectedPlacement.y * 100)}%
                </div>
              </div>
              <div className="rounded-lg border border-white/8 bg-black/25 p-2">
                <div className="text-[10px] text-zinc-400">{t.canvas.size}</div>
                <div className="mt-0.5 font-medium text-zinc-200 tabular-nums">
                  {Math.round(selectedPlacement.width * 100)}% × {Math.round(selectedPlacement.height * 100)}%
                </div>
              </div>
            </div>
          </div>
        )}

        {activeProfile && (
          <div className="studio-panel flex flex-col gap-4 rounded-2xl p-4">
            <div className="text-xs font-semibold uppercase tracking-wider text-zinc-400">
              {t.canvas.inspectorTitle}
            </div>

            <div className="grid grid-cols-2 gap-2">
              <label className="flex flex-col gap-1.5 text-xs text-zinc-300">
                <span>{t.effects.primaryColor}</span>
                <div className="flex items-center gap-2 rounded-lg border border-white/10 bg-black/25 px-2 py-1.5">
                  <input
                    type="color"
                    value={rgbToHex(activeProfile.effect.primary_color)}
                    onChange={(e) =>
                      updateEffect({
                        ...activeProfile.effect,
                        primary_color: hexToRgb(e.target.value),
                      })
                    }
                    className="h-5 w-6 cursor-pointer rounded border-0 bg-transparent p-0"
                  />
                  <span className="font-mono text-[11px] text-zinc-300">
                    {rgbToHex(activeProfile.effect.primary_color)}
                  </span>
                </div>
              </label>

              <label className="flex flex-col gap-1.5 text-xs text-zinc-300">
                <span>{t.effects.secondaryColor}</span>
                <div className="flex items-center gap-2 rounded-lg border border-white/10 bg-black/25 px-2 py-1.5">
                  <input
                    type="color"
                    value={rgbToHex(activeProfile.effect.secondary_color)}
                    onChange={(e) =>
                      updateEffect({
                        ...activeProfile.effect,
                        secondary_color: hexToRgb(e.target.value),
                      })
                    }
                    className="h-5 w-6 cursor-pointer rounded border-0 bg-transparent p-0"
                  />
                  <span className="font-mono text-[11px] text-zinc-300">
                    {rgbToHex(activeProfile.effect.secondary_color)}
                  </span>
                </div>
              </label>
            </div>

            <div>
              <div className="mb-1.5 flex justify-between text-xs text-zinc-300">
                <span>{t.effects.speed}</span>
                <span className="tabular-nums">{activeProfile.effect.speed}%</span>
              </div>
              <Slider.Root
                className="relative flex h-5 w-full touch-none items-center select-none"
                value={[activeProfile.effect.speed]}
                min={1}
                max={100}
                onValueChange={([v]) =>
                  updateEffect({ ...activeProfile.effect, speed: v })
                }
              >
                <Slider.Track className="relative h-1.5 w-full grow rounded-full bg-white/10">
                  <Slider.Range className="absolute h-full rounded-full bg-zinc-100" />
                </Slider.Track>
                <Slider.Thumb className="block h-3.5 w-3.5 rounded-full bg-zinc-100" />
              </Slider.Root>
            </div>

            <div>
              <div className="mb-1.5 flex justify-between text-xs text-zinc-300">
                <span>{t.effects.profileBrightness}</span>
                <span className="tabular-nums">{activeProfile.brightness}%</span>
              </div>
              <Slider.Root
                className="relative flex h-5 w-full touch-none items-center select-none"
                value={[activeProfile.brightness]}
                min={0}
                max={100}
                onValueChange={([v]) => updateEffect(activeProfile.effect, v)}
              >
                <Slider.Track className="relative h-1.5 w-full grow rounded-full bg-white/10">
                  <Slider.Range className="absolute h-full rounded-full bg-zinc-100" />
                </Slider.Track>
                <Slider.Thumb className="block h-3.5 w-3.5 rounded-full bg-zinc-100" />
              </Slider.Root>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
