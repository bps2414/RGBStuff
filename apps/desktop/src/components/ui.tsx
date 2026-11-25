import * as RSlider from "@radix-ui/react-slider";
import * as RSwitch from "@radix-ui/react-switch";
import type { ReactNode } from "react";
import { hexToRgb, rgbToHex, type RgbColor } from "../lib/tauri";

export function Slider({
  label,
  value,
  min = 0,
  max = 100,
  step = 1,
  display,
  onChange,
  marker,
  icon,
}: {
  label?: ReactNode;
  value: number;
  min?: number;
  max?: number;
  step?: number;
  display?: ReactNode;
  onChange: (v: number) => void;
  /** Optional tick (e.g. 100% on a 0..200 boost slider). */
  marker?: number;
  icon?: ReactNode;
}) {
  return (
    <div className="flex flex-col gap-2">
      {(label || display !== undefined) && (
        <div className="flex items-center justify-between text-xs">
          <span className="flex items-center gap-1.5 text-zinc-300">
            {icon}
            {label}
          </span>
          <span className="font-medium text-zinc-100 tabular-nums">{display ?? value}</span>
        </div>
      )}
      <RSlider.Root
        className="ui-slider relative flex h-5 w-full touch-none items-center select-none"
        value={[value]}
        min={min}
        max={max}
        step={step}
        onValueChange={([v]) => onChange(v)}
      >
        <RSlider.Track className="relative h-1.5 w-full grow overflow-hidden rounded-full bg-white/10">
          <RSlider.Range className="ui-slider-range absolute h-full rounded-full" />
        </RSlider.Track>
        {marker !== undefined && (
          <span
            className="pointer-events-none absolute top-1/2 h-2.5 w-px -translate-y-1/2 bg-white/35"
            style={{ left: `${((marker - min) / (max - min)) * 100}%` }}
          />
        )}
        <RSlider.Thumb className="ui-slider-thumb block h-4 w-4 rounded-full bg-white focus:outline-none" />
      </RSlider.Root>
    </div>
  );
}

export function Toggle({ checked, onChange, small }: { checked: boolean; onChange: (v: boolean) => void; small?: boolean }) {
  return (
    <RSwitch.Root
      checked={checked}
      onCheckedChange={onChange}
      className={`ui-toggle relative shrink-0 cursor-pointer rounded-full bg-zinc-700/80 transition-colors ${small ? "h-4 w-7" : "h-5 w-9"}`}
    >
      <RSwitch.Thumb
        className={`block rounded-full bg-white shadow transition-transform ${
          small ? "h-3 w-3 translate-x-0.5 data-[state=checked]:translate-x-3.5" : "h-4 w-4 translate-x-0.5 data-[state=checked]:translate-x-4.5"
        }`}
      />
    </RSwitch.Root>
  );
}

export function Segmented<T extends string>({
  value,
  options,
  onChange,
}: {
  value: T;
  options: Array<{ value: T; label: ReactNode }>;
  onChange: (v: T) => void;
}) {
  return (
    <div className="flex shrink-0 rounded-lg border border-white/10 bg-black/30 p-0.5">
      {options.map((o) => (
        <button
          key={o.value}
          type="button"
          onClick={() => onChange(o.value)}
          className={`rounded-md px-3 py-1 text-xs font-medium transition-colors ${
            value === o.value ? "bg-zinc-100 text-zinc-950" : "text-zinc-400 hover:text-zinc-200"
          }`}
        >
          {o.label}
        </button>
      ))}
    </div>
  );
}

export function Panel({
  title,
  desc,
  actions,
  children,
  className = "",
}: {
  title?: ReactNode;
  desc?: ReactNode;
  actions?: ReactNode;
  children: ReactNode;
  className?: string;
}) {
  return (
    <section className={`studio-panel rounded-2xl p-4 ${className}`}>
      {(title || actions) && (
        <div className="mb-3 flex items-start justify-between gap-3">
          <div className="min-w-0">
            {title && <h3 className="text-[11px] font-semibold uppercase tracking-[0.08em] text-zinc-400">{title}</h3>}
            {desc && <p className="mt-0.5 text-xs text-zinc-500">{desc}</p>}
          </div>
          {actions}
        </div>
      )}
      {children}
    </section>
  );
}

export function SettingRow({
  title,
  desc,
  children,
  first,
}: {
  title: ReactNode;
  desc?: ReactNode;
  children: ReactNode;
  first?: boolean;
}) {
  return (
    <div className={`flex flex-wrap items-center justify-between gap-4 ${first ? "" : "border-t border-white/6 pt-3"}`}>
      <div className="min-w-0 flex-1">
        <div className="text-sm font-medium text-zinc-100">{title}</div>
        {desc && <div className="mt-0.5 text-xs text-zinc-400">{desc}</div>}
      </div>
      {children}
    </div>
  );
}

export function ColorField({ label, value, onChange }: { label: string; value: RgbColor; onChange: (c: RgbColor) => void }) {
  const hex = rgbToHex(value);
  return (
    <label className="flex flex-col gap-1.5 text-xs text-zinc-300">
      <span>{label}</span>
      <span className="relative flex cursor-pointer items-center gap-2.5 rounded-lg border border-white/10 bg-black/25 px-2 py-1.5 hover:border-white/20">
        <span className="h-6 w-6 shrink-0 rounded-md border border-white/20 shadow-inner" style={{ background: hex, boxShadow: `0 0 14px ${hex}66` }} />
        <span className="font-mono text-[11px] text-zinc-200">{hex}</span>
        <input
          type="color"
          value={hex}
          onChange={(e) => onChange(hexToRgb(e.target.value))}
          className="absolute inset-0 cursor-pointer opacity-0"
        />
      </span>
    </label>
  );
}
