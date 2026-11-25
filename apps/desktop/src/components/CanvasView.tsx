import { Cpu } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import type { DevicePlacement } from "../lib/tauri";
import { subscribeCanvasPixels, useAppStore } from "../store/useAppStore";
import { iconForKind } from "./DevicesView";
import { EffectControls, useActiveProfile } from "./EffectWidgets";
import { Panel, Slider } from "./ui";

const clamp = (v: number, lo: number, hi: number) => Math.max(lo, Math.min(hi, v));
const round3 = (v: number) => Number(v.toFixed(3));

export function CanvasView({ onOpenDevices }: { onOpenDevices: () => void }) {
  const { state, t, updatePlacement } = useAppStore();
  const profile = useActiveProfile();
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const stageRef = useRef<HTMLDivElement | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [dragging, setDragging] = useState<{
    id: string;
    startX: number;
    startY: number;
    origX: number;
    origY: number;
  } | null>(null);

  const canvasW = state?.canvas_width ?? 0;
  const canvasH = state?.canvas_height ?? 0;

  // Draw preview frames straight into the canvas (no React re-render per frame),
  // at most once per animation frame.
  useEffect(() => {
    const el = canvasRef.current;
    const ctx = el?.getContext("2d");
    if (!ctx || canvasW <= 0 || canvasH <= 0) return;
    const img = ctx.createImageData(canvasW, canvasH);
    const n = canvasW * canvasH;
    let pending: Uint8Array | null = null;
    let raf = 0;

    const draw = () => {
      raf = 0;
      const px = pending;
      pending = null;
      if (!px || px.length < n * 3) return;
      const data = img.data;
      for (let i = 0, p = 0, d = 0; i < n; i++, p += 3, d += 4) {
        data[d] = px[p];
        data[d + 1] = px[p + 1];
        data[d + 2] = px[p + 2];
        data[d + 3] = 255;
      }
      ctx.putImageData(img, 0, 0);
    };

    const unsubscribe = subscribeCanvasPixels((px) => {
      pending = px;
      if (!raf) raf = requestAnimationFrame(draw);
    });
    return () => {
      unsubscribe();
      if (raf) cancelAnimationFrame(raf);
    };
  }, [canvasW, canvasH]);

  if (!state) return null;
  const { config, engine_status, devices } = state;

  // Only placements for devices this PC actually has a driver entry for.
  const known = config.placements
    .map((pl) => ({ pl, dev: devices.find((d) => d.id === pl.device_id) }))
    .filter((x) => x.dev !== undefined);
  const anyConnected = known.some((x) => x.dev?.connected);
  const selected =
    known.find((x) => x.pl.device_id === selectedId) ?? known.find((x) => x.dev?.connected) ?? known[0];

  const handlePointerMove = (e: React.PointerEvent<HTMLDivElement>) => {
    if (!dragging || !stageRef.current) return;
    const rect = stageRef.current.getBoundingClientRect();
    const dx = (e.clientX - dragging.startX) / rect.width;
    const dy = (e.clientY - dragging.startY) / rect.height;
    const target = config.placements.find((p) => p.device_id === dragging.id);
    if (!target) return;
    const nx = clamp(dragging.origX + dx, 0, 1 - target.width);
    const ny = clamp(dragging.origY + dy, 0, 1 - target.height);
    void updatePlacement({ ...target, x: round3(nx), y: round3(ny) });
  };

  const patchSelected = (patch: Partial<DevicePlacement>) => {
    if (!selected) return;
    const next = { ...selected.pl, ...patch };
    next.width = clamp(next.width, 0.05, 1);
    next.height = clamp(next.height, 0.05, 1);
    next.x = clamp(next.x, 0, 1 - next.width);
    next.y = clamp(next.y, 0, 1 - next.height);
    void updatePlacement(next);
  };

  return (
    <div className="fade-in grid h-full grid-cols-1 gap-4 overflow-y-auto pr-1 xl:grid-cols-[1fr_320px]">
      <Panel
        className="flex flex-col"
        title={t.canvas.title}
        desc={t.canvas.subtitle}
        actions={
          <span className="flex shrink-0 items-center gap-1.5 rounded-md border border-white/10 bg-white/[0.04] px-2 py-1 text-[11px] font-medium text-zinc-300 tabular-nums">
            {engine_status.mode === "dynamic" && <span className="live-dot h-1.5 w-1.5 animate-pulse rounded-full" />}
            {engine_status.mode === "dynamic" ? t.canvas.previewLive : t.canvas.previewStatic}
          </span>
        }
      >
        <div
          ref={stageRef}
          onPointerMove={handlePointerMove}
          onPointerUp={() => setDragging(null)}
          onPointerCancel={() => setDragging(null)}
          className="relative aspect-video w-full overflow-hidden rounded-xl border border-white/10 bg-zinc-950"
        >
          <canvas ref={canvasRef} width={state.canvas_width} height={state.canvas_height} className="h-full w-full" />
          <div
            className="pointer-events-none absolute inset-0"
            style={{
              backgroundImage:
                "linear-gradient(to right, rgba(255,255,255,0.05) 1px, transparent 1px), linear-gradient(to bottom, rgba(255,255,255,0.05) 1px, transparent 1px)",
              backgroundSize: "6.25% 11.11%",
            }}
          />

          {known.map(({ pl, dev }) => {
            const isSelected = pl.device_id === selected?.pl.device_id;
            const Icon = iconForKind(dev?.kind ?? "");
            const connected = dev?.connected ?? false;
            return (
              <button
                key={pl.device_id}
                type="button"
                onPointerDown={(e) => {
                  // Keep receiving moves even if the cursor outruns the block or the stage.
                  e.currentTarget.setPointerCapture(e.pointerId);
                  setSelectedId(pl.device_id);
                  setDragging({ id: pl.device_id, startX: e.clientX, startY: e.clientY, origX: pl.x, origY: pl.y });
                }}
                style={{
                  left: `${pl.x * 100}%`,
                  top: `${pl.y * 100}%`,
                  width: `${pl.width * 100}%`,
                  height: `${pl.height * 100}%`,
                  transform: pl.rotation_deg ? `rotate(${pl.rotation_deg}deg)` : undefined,
                }}
                className={`absolute flex cursor-grab touch-none flex-col justify-between rounded-xl border p-2 text-left backdrop-blur-[1px] active:cursor-grabbing ${
                  isSelected
                    ? "border-white/90 bg-black/30 shadow-[0_0_0_3px_rgb(var(--live)/0.35)]"
                    : connected
                      ? "border-white/35 bg-black/20 hover:border-white/60"
                      : "border-dashed border-white/20 bg-black/40 opacity-60"
                }`}
              >
                <span className="flex items-center gap-1.5 truncate text-[11px] font-medium text-white drop-shadow">
                  <Icon className="h-3.5 w-3.5 shrink-0" />
                  <span className="truncate">{dev?.name ?? pl.device_id}</span>
                </span>
                <span className="text-[10px] text-white/70 tabular-nums drop-shadow">
                  {connected ? `${pl.led_count} ${t.canvas.leds}` : t.canvas.notDetected}
                </span>
              </button>
            );
          })}

          {!anyConnected && (
            <div className="absolute inset-0 flex flex-col items-center justify-center gap-2 bg-black/55 text-center">
              <Cpu className="h-6 w-6 text-zinc-400" />
              <div className="text-sm font-medium text-zinc-100">{t.canvas.noDevices}</div>
              <p className="max-w-xs text-xs text-zinc-400">{t.canvas.noDevicesDesc}</p>
              <button type="button" onClick={onOpenDevices} className="studio-btn btn-primary mt-1 rounded-lg px-3 py-1.5 text-xs font-semibold">
                {t.nav.devices}
              </button>
            </div>
          )}
        </div>
        <p className="mt-2.5 text-[11px] text-zinc-500">{t.canvas.dragHint}</p>
      </Panel>

      <div className="flex flex-col gap-4">
        {selected && (
          <Panel title={t.canvas.selectedDevice}>
            <div className="mb-3 truncate text-sm font-medium text-zinc-100">{selected.dev?.name ?? selected.pl.device_id}</div>
            <div className="flex flex-col gap-3.5">
              <Slider
                label={t.canvas.intensity}
                value={selected.pl.brightness ?? 100}
                min={0}
                max={200}
                step={5}
                marker={100}
                display={`${selected.pl.brightness ?? 100}%`}
                onChange={(v) => patchSelected({ brightness: v })}
              />
              <Slider
                label={t.canvas.width}
                value={Math.round(selected.pl.width * 100)}
                min={5}
                max={100}
                display={`${Math.round(selected.pl.width * 100)}%`}
                onChange={(v) => patchSelected({ width: v / 100 })}
              />
              <Slider
                label={t.canvas.height}
                value={Math.round(selected.pl.height * 100)}
                min={5}
                max={100}
                display={`${Math.round(selected.pl.height * 100)}%`}
                onChange={(v) => patchSelected({ height: v / 100 })}
              />
              <Slider
                label={t.canvas.rotation}
                value={selected.pl.rotation_deg}
                min={-180}
                max={180}
                step={5}
                marker={0}
                display={`${Math.round(selected.pl.rotation_deg)}°`}
                onChange={(v) => patchSelected({ rotation_deg: v })}
              />
            </div>
          </Panel>
        )}

        {profile && (
          <Panel title={t.canvas.inspectorTitle} desc={t.effects.items[profile.effect.kind].name}>
            <EffectControls columns={1} />
          </Panel>
        )}
      </div>
    </div>
  );
}
