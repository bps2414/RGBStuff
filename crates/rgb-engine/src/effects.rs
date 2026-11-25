use crate::canvas::Canvas2D;
use rgb_core::{EffectKind, EffectParams, Rgb, hsv_to_rgb};
use std::f32::consts::{PI, TAU};

pub struct EffectFrameInput<'a> {
    pub time_secs: f32,
    pub screen_frame: Option<(usize, usize, &'a [u8])>,
    pub audio_bands: &'a [f32],
}

/// Memory carried between frames: temporal smoothing, peak hold, crossfades.
#[derive(Debug, Default)]
pub struct EffectState {
    last_time: Option<f32>,
    last_kind: Option<EffectKind>,
    /// Smoothed ambilight canvas (RGB, 0..=255 as f32).
    ambient: Vec<f32>,
    /// Scratch buffers for the blurred, letterbox-cropped screen.
    blur: Vec<f32>,
    blur_tmp: Vec<f32>,
    peaks: Vec<f32>,
    bass: f32,
}

impl EffectState {
    pub fn new() -> Self {
        Self::default()
    }
}

pub fn render_effect(
    canvas: &mut Canvas2D,
    params: &EffectParams,
    input: &EffectFrameInput<'_>,
    state: &mut EffectState,
) {
    let dt = state
        .last_time
        .map(|t| (input.time_secs - t).clamp(0.0, 0.25))
        .unwrap_or(1.0 / 60.0);
    state.last_time = Some(input.time_secs);
    let switched = state.last_kind != Some(params.kind);
    state.last_kind = Some(params.kind);

    match params.kind {
        EffectKind::Off => canvas.fill(Rgb::BLACK),
        EffectKind::Solid => canvas.fill(params.primary_color),
        EffectKind::Gradient => render_gradient(canvas, params),
        EffectKind::RainbowWave => render_rainbow_wave(canvas, params, input.time_secs),
        EffectKind::Breathing => render_breathing(canvas, params, input.time_secs),
        EffectKind::DualPulse => render_dual_pulse(canvas, params, input.time_secs),
        EffectKind::Starlight => render_starlight(canvas, params, input.time_secs),
        EffectKind::Aurora => render_aurora(canvas, params, input.time_secs),
        EffectKind::Plasma => render_plasma(canvas, params, input.time_secs),
        EffectKind::Ripple => render_ripple(canvas, params, input.time_secs),
        EffectKind::Ambilight => render_ambilight(canvas, params, input, state, dt, switched),
        EffectKind::AudioSpectrum => render_audio_spectrum(canvas, params, input, state, dt),
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

#[inline]
fn rgb_f(c: Rgb) -> [f32; 3] {
    [c.r as f32 / 255.0, c.g as f32 / 255.0, c.b as f32 / 255.0]
}

#[inline]
fn to_rgb(c: [f32; 3]) -> Rgb {
    let q = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    Rgb::new(q(c[0]), q(c[1]), q(c[2]))
}

#[inline]
fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

#[inline]
fn scale3(c: [f32; 3], k: f32) -> [f32; 3] {
    [c[0] * k, c[1] * k, c[2] * k]
}

#[inline]
fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[inline]
fn hash3(x: i32, y: i32, z: i32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x8DA6_B343)
        ^ (y as u32).wrapping_mul(0xD816_3841)
        ^ (z as u32).wrapping_mul(0xCB1A_B31F);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5BD1_E995);
    h ^= h >> 15;
    (h & 0x00FF_FFFF) as f32 / 16_777_215.0
}

/// Smooth 3D value noise in `0..=1`.
fn noise3(x: f32, y: f32, z: f32) -> f32 {
    let (xi, yi, zi) = (x.floor(), y.floor(), z.floor());
    let (xf, yf, zf) = (x - xi, y - yi, z - zi);
    let (xi, yi, zi) = (xi as i32, yi as i32, zi as i32);
    let fade = |t: f32| t * t * t * (t * (t * 6.0 - 15.0) + 10.0);
    let (u, v, w) = (fade(xf), fade(yf), fade(zf));
    let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;

    let c = |dx: i32, dy: i32, dz: i32| hash3(xi + dx, yi + dy, zi + dz);
    let x00 = lerp(c(0, 0, 0), c(1, 0, 0), u);
    let x10 = lerp(c(0, 1, 0), c(1, 1, 0), u);
    let x01 = lerp(c(0, 0, 1), c(1, 0, 1), u);
    let x11 = lerp(c(0, 1, 1), c(1, 1, 1), u);
    lerp(lerp(x00, x10, v), lerp(x01, x11, v), w)
}

/// Three-octave fractal noise in `0..=1`.
fn fbm(x: f32, y: f32, z: f32) -> f32 {
    let mut sum = 0.0;
    let mut amp = 0.5;
    let mut freq = 1.0;
    for _ in 0..3 {
        sum += amp * noise3(x * freq, y * freq, z * freq);
        freq *= 2.03;
        amp *= 0.5;
    }
    sum / 0.875
}

/// Maps the speed slider (`1..=100`) to a multiplier.
#[inline]
fn speed_k(params: &EffectParams, min: f32, max: f32) -> f32 {
    min + (params.speed.clamp(1, 100) as f32 / 100.0) * (max - min)
}

#[inline]
fn scale_k(params: &EffectParams) -> f32 {
    (params.scale.max(10) as f32) / 100.0
}

/// Iterates canvas pixels with normalized coords (x aspect-corrected to 16:9).
fn for_each_pixel(canvas: &mut Canvas2D, mut f: impl FnMut(f32, f32) -> [f32; 3]) {
    let w = (canvas.width.saturating_sub(1)).max(1) as f32;
    let h = (canvas.height.saturating_sub(1)).max(1) as f32;
    let aspect = canvas.width as f32 / canvas.height.max(1) as f32;
    for y in 0..canvas.height {
        let ny = y as f32 / h;
        for x in 0..canvas.width {
            let nx = x as f32 / w * aspect;
            let c = f(nx, ny);
            canvas.set_pixel(x, y, to_rgb(c));
        }
    }
}

// ---------------------------------------------------------------------------
// Classic effects
// ---------------------------------------------------------------------------

fn render_gradient(canvas: &mut Canvas2D, params: &EffectParams) {
    let rad = params.angle_deg.to_radians();
    let (sin_a, cos_a) = rad.sin_cos();
    let w = (canvas.width.saturating_sub(1)).max(1) as f32;
    let h = (canvas.height.saturating_sub(1)).max(1) as f32;

    for y in 0..canvas.height {
        let ny = y as f32 / h - 0.5;
        for x in 0..canvas.width {
            let nx = x as f32 / w - 0.5;
            let proj = (nx * cos_a + ny * sin_a) + 0.5;
            let c = params.primary_color.lerp(params.secondary_color, proj);
            canvas.set_pixel(x, y, c);
        }
    }
}

fn render_rainbow_wave(canvas: &mut Canvas2D, params: &EffectParams, time_secs: f32) {
    let rad = params.angle_deg.to_radians();
    let (sin_a, cos_a) = rad.sin_cos();
    let speed = speed_k(params, 0.15, 1.55);
    let scale = scale_k(params);
    let w = (canvas.width.saturating_sub(1)).max(1) as f32;
    let h = (canvas.height.saturating_sub(1)).max(1) as f32;

    for y in 0..canvas.height {
        let ny = y as f32 / h;
        for x in 0..canvas.width {
            let nx = x as f32 / w;
            // A slow noise warp keeps the bands organic instead of ruler-straight.
            let warp = (noise3(nx * 2.0, ny * 2.0, time_secs * 0.15) - 0.5) * 0.25;
            let proj = nx * cos_a + ny * sin_a + warp;
            let hue = (proj * 360.0 * scale - time_secs * speed * 180.0).rem_euclid(360.0);
            canvas.set_pixel(x, y, hsv_to_rgb(hue, 0.95, 1.0));
        }
    }
}

/// Breathes alternately in the primary and secondary colors, with the
/// exponential-sine curve that reads as natural breathing (never fully off).
fn render_breathing(canvas: &mut Canvas2D, params: &EffectParams, time_secs: f32) {
    let freq = speed_k(params, 0.12, 0.9);
    let phase = time_secs * freq;
    let cycle = phase.floor() as i64;
    let local = phase - phase.floor();
    let e = std::f32::consts::E;
    let curve = (((local * TAU - PI / 2.0).sin()).exp() - 1.0 / e) / (e - 1.0 / e);
    let color = if cycle % 2 == 0 {
        params.primary_color
    } else {
        params.secondary_color
    };
    let level = 0.06 + 0.94 * curve.clamp(0.0, 1.0);
    canvas.fill(to_rgb(scale3(rgb_f(color), level)));
}

fn render_dual_pulse(canvas: &mut Canvas2D, params: &EffectParams, time_secs: f32) {
    let rad = params.angle_deg.to_radians();
    let (sin_a, cos_a) = rad.sin_cos();
    let speed = speed_k(params, 0.25, 1.85);
    let scale = scale_k(params);
    let w = (canvas.width.saturating_sub(1)).max(1) as f32;
    let h = (canvas.height.saturating_sub(1)).max(1) as f32;

    for y in 0..canvas.height {
        let ny = y as f32 / h;
        for x in 0..canvas.width {
            let nx = x as f32 / w;
            let proj = nx * cos_a + ny * sin_a;
            let t = (((proj * scale * TAU) - time_secs * speed * TAU).sin() + 1.0) * 0.5;
            let color = params.primary_color.lerp(params.secondary_color, t);
            canvas.set_pixel(x, y, color);
        }
    }
}

/// Soft stars on a slowly drifting nebula. Stars are blobs a few pixels wide so
/// that sparse LEDs actually catch them; each one fades in and out once per cycle
/// and re-spawns somewhere else.
fn render_starlight(canvas: &mut Canvas2D, params: &EffectParams, time_secs: f32) {
    let speed = speed_k(params, 0.25, 1.6);
    let cell = (7.0 / scale_k(params)).clamp(3.0, 18.0);
    let primary = rgb_f(params.primary_color);
    let secondary = rgb_f(params.secondary_color);
    let w = canvas.width;
    let h = canvas.height;

    for y in 0..h {
        for x in 0..w {
            let (fx, fy) = (x as f32, y as f32);
            let nebula = fbm(fx * 0.035, fy * 0.05, time_secs * 0.06);
            let mut c = scale3(secondary, 0.08 + 0.17 * nebula);

            let cx = (fx / cell).floor() as i32;
            let cy = (fy / cell).floor() as i32;
            // Check neighbouring cells so blobs can overlap cell borders.
            for oy in -1..=1 {
                for ox in -1..=1 {
                    let (gx, gy) = (cx + ox, cy + oy);
                    let offset = hash3(gx, gy, 7);
                    let t = time_secs * speed * (0.6 + 0.8 * hash3(gx, gy, 3)) + offset * 10.0;
                    let cycle = t.floor() as i32;
                    if hash3(gx, gy, cycle) > 0.5 {
                        continue;
                    }
                    let life = (t - t.floor()) * PI;
                    let intensity = life.sin().powi(3);
                    let sx = (gx as f32 + 0.2 + 0.6 * hash3(gx, gy, cycle + 11)) * cell;
                    let sy = (gy as f32 + 0.2 + 0.6 * hash3(gx, gy, cycle + 23)) * cell;
                    let d2 = ((fx - sx).powi(2) + (fy - sy).powi(2)) / (cell * cell * 0.22);
                    let glow = (-d2).exp() * intensity;
                    if glow < 0.01 {
                        continue;
                    }
                    let tint = mix(primary, secondary, hash3(gx, gy, cycle + 5) * 0.6);
                    // Brightest moment flashes slightly toward white.
                    let star = mix(tint, [1.0, 1.0, 1.0], 0.35 * intensity.powi(4));
                    c = mix(c, star, glow.min(1.0));
                }
            }
            canvas.set_pixel(x, y, to_rgb(c));
        }
    }
}

// ---------------------------------------------------------------------------
// New generative effects
// ---------------------------------------------------------------------------

/// Northern-lights curtains: noise-warped vertical bands that drift and fold,
/// colored between primary and secondary, over a deep tinted sky.
fn render_aurora(canvas: &mut Canvas2D, params: &EffectParams, time_secs: f32) {
    let s = speed_k(params, 0.05, 0.6);
    let k = scale_k(params);
    let t = time_secs * s;
    let primary = rgb_f(params.primary_color);
    let secondary = rgb_f(params.secondary_color);
    let sky = scale3(mix(primary, secondary, 0.5), 0.16);

    for_each_pixel(canvas, |nx, ny| {
        let warp = fbm(nx * 1.1 * k + t * 0.4, ny * 0.8 * k, t * 0.25);
        let curtain = ((nx * 2.4 * k + warp * 3.2 - t * 0.9) * TAU * 0.5).sin() * 0.5 + 0.5;
        let ribbon = curtain.powf(1.6);
        let shimmer = 0.75 + 0.25 * noise3(nx * 6.0, ny * 2.0, t * 2.5);
        let fall = 0.55 + 0.45 * (1.0 - ny);
        let intensity = (ribbon * shimmer * fall).clamp(0.0, 1.0);
        let hue_mix = fbm(nx * 0.7 * k - t * 0.3, ny * 0.9, t * 0.15 + 4.0);
        let tone = mix(primary, secondary, smoothstep(0.3, 0.7, hue_mix));
        mix(sky, tone, 0.3 + 0.7 * intensity)
    });
}

/// Lava-lamp plasma: overlapping sine fields + noise, mapped onto a looping
/// primary → secondary → primary palette with soft highlights.
fn render_plasma(canvas: &mut Canvas2D, params: &EffectParams, time_secs: f32) {
    let s = speed_k(params, 0.08, 0.9);
    let k = scale_k(params) * 2.2;
    let t = time_secs * s;
    let primary = rgb_f(params.primary_color);
    let secondary = rgb_f(params.secondary_color);

    for_each_pixel(canvas, |nx, ny| {
        let (x, y) = (nx * k, ny * k);
        let v = (x * 1.3 + t).sin()
            + (y * 1.7 - t * 1.2).sin()
            + ((x + y) * 1.1 + t * 0.7).sin()
            + (((x - 0.9).powi(2) + (y - 0.5).powi(2)).sqrt() * 2.4 - t * 1.5).sin()
            + (fbm(x * 0.6, y * 0.6, t * 0.4) - 0.5) * 3.0;
        let p = (v / 5.0 + 0.5).rem_euclid(1.0);
        // Triangle wave → smooth looping palette.
        let tri = 1.0 - (p * 2.0 - 1.0).abs();
        let base = mix(primary, secondary, smoothstep(0.0, 1.0, tri));
        let highlight = ((v * 1.7).sin() * 0.5 + 0.5).powi(6) * 0.3;
        mix(scale3(base, 0.6 + 0.4 * tri), [1.0, 1.0, 1.0], highlight)
    });
}

/// Rings of light spreading from random points across the setup, like drops on
/// water. Primary rings over a softly glowing secondary base.
fn render_ripple(canvas: &mut Canvas2D, params: &EffectParams, time_secs: f32) {
    const EMITTERS: i32 = 4;
    let s = speed_k(params, 0.15, 0.9);
    let width = 0.12 * scale_k(params);
    let primary = rgb_f(params.primary_color);
    let secondary = rgb_f(params.secondary_color);
    let aspect = canvas.width as f32 / canvas.height.max(1) as f32;
    let max_r = aspect.max(1.0) * 0.9;

    // Precompute this frame's rings.
    let rings: Vec<(f32, f32, f32, f32)> = (0..EMITTERS)
        .map(|i| {
            let t = time_secs * s + i as f32 / EMITTERS as f32;
            let cycle = t.floor() as i32;
            let phase = t - t.floor();
            let cx = (0.1 + 0.8 * hash3(i, cycle, 1)) * aspect;
            let cy = 0.1 + 0.8 * hash3(i, cycle, 2);
            (cx, cy, phase * max_r, (1.0 - phase).powf(1.5))
        })
        .collect();

    for_each_pixel(canvas, |nx, ny| {
        let mut c = scale3(secondary, 0.24);
        for &(cx, cy, r, fade) in &rings {
            let d = ((nx - cx).powi(2) + (ny - cy).powi(2)).sqrt();
            let band = (-((d - r) / width).powi(2)).exp() * fade;
            // Faint trailing wake inside the ring.
            let wake = if d < r { 0.18 * fade * (d / r.max(1e-3)) } else { 0.0 };
            c = mix(c, primary, (band + wake).min(1.0));
        }
        c
    });
}

// ---------------------------------------------------------------------------
// Ambilight
// ---------------------------------------------------------------------------

fn render_ambilight(
    canvas: &mut Canvas2D,
    params: &EffectParams,
    input: &EffectFrameInput<'_>,
    state: &mut EffectState,
    dt: f32,
    switched: bool,
) {
    let n = canvas.pixels.len();
    if switched || state.ambient.len() != n {
        // Start from whatever was on the LEDs so switching in is a crossfade.
        state.ambient = canvas.pixels.iter().map(|&v| v as f32).collect();
    }

    match input.screen_frame {
        Some((sw, sh, data)) if sw > 1 && sh > 1 && data.len() >= sw * sh * 3 => {
            build_ambient_target(canvas, params, sw, sh, data, state);
        }
        // Capture not ready yet (or protected content): a calm aurora instead of black.
        _ => render_aurora(canvas, params, input.time_secs * 0.5),
    }

    // Temporal smoothing. Speed slider = responsiveness (~500 ms → ~40 ms).
    // Brightening reacts a bit faster than fading so flashes still land.
    let tau = 0.04 + 0.46 * (1.0 - params.speed.clamp(1, 100) as f32 / 100.0);
    let a_up = 1.0 - (-dt / (tau * 0.6)).exp();
    let a_down = 1.0 - (-dt / tau).exp();
    for (s, px) in state.ambient.iter_mut().zip(canvas.pixels.iter_mut()) {
        let target = *px as f32;
        let a = if target > *s { a_up } else { a_down };
        *s += (target - *s) * a;
        *px = s.round().clamp(0.0, 255.0) as u8;
    }
}

/// Letterbox-cropped, blurred screen mapped onto the canvas, blended with the
/// screen's dominant vivid color and enhanced for LEDs.
fn build_ambient_target(
    canvas: &mut Canvas2D,
    params: &EffectParams,
    sw: usize,
    sh: usize,
    data: &[u8],
    state: &mut EffectState,
) {
    const DARK: u8 = 18;
    let px_max = |x: usize, y: usize| {
        let i = (y * sw + x) * 3;
        data[i].max(data[i + 1]).max(data[i + 2])
    };
    let row_dark = |y: usize| (0..sw).all(|x| px_max(x, y) <= DARK);
    let col_dark = |x: usize, y0: usize, y1: usize| (y0..y1).all(|y| px_max(x, y) <= DARK);

    // 1. Black-bar detection (movies, ultrawide content, pillarboxing).
    let mut top = 0;
    while top < sh / 3 && row_dark(top) {
        top += 1;
    }
    let mut bottom = sh;
    while bottom > sh - sh / 3 && bottom > top + 1 && row_dark(bottom - 1) {
        bottom -= 1;
    }
    let mut left = 0;
    while left < sw / 4 && col_dark(left, top, bottom) {
        left += 1;
    }
    let mut right = sw;
    while right > sw - sw / 4 && right > left + 1 && col_dark(right - 1, top, bottom) {
        right -= 1;
    }
    let cw = right - left;
    let ch = bottom - top;

    // 2. Separable box blur (two passes ≈ gaussian) so each LED sees an area,
    //    not a single noisy pixel (text, cursors, UI edges).
    let len = cw * ch * 3;
    state.blur.resize(len, 0.0);
    state.blur_tmp.resize(len, 0.0);
    for y in 0..ch {
        for x in 0..cw {
            let src = ((y + top) * sw + (x + left)) * 3;
            let dst = (y * cw + x) * 3;
            for c in 0..3 {
                state.blur[dst + c] = data[src + c] as f32 / 255.0;
            }
        }
    }
    let radius = 2usize;
    for _ in 0..2 {
        box_blur_axis(&state.blur, &mut state.blur_tmp, cw, ch, radius, true);
        box_blur_axis(&state.blur_tmp, &mut state.blur, cw, ch, radius, false);
    }

    // 3. Dominant color: vivid and bright regions outweigh grey UI and text.
    let mut acc = [0.0f32; 3];
    let mut wsum = 0.0f32;
    for y in top..bottom {
        for x in left..right {
            let i = (y * sw + x) * 3;
            let c = [
                data[i] as f32 / 255.0,
                data[i + 1] as f32 / 255.0,
                data[i + 2] as f32 / 255.0,
            ];
            let max = c[0].max(c[1]).max(c[2]);
            let min = c[0].min(c[1]).min(c[2]);
            let sat = if max > 0.0 { (max - min) / max } else { 0.0 };
            let w = 0.02 * max + sat * sat * max * 4.0;
            acc = [acc[0] + c[0] * w, acc[1] + c[1] * w, acc[2] + c[2] * w];
            wsum += w;
        }
    }
    let dominant = if wsum > 1e-4 {
        [acc[0] / wsum, acc[1] / wsum, acc[2] / wsum]
    } else {
        [0.0; 3]
    };

    // 4. Map onto the canvas. Scale slider: low = spatial detail per device,
    //    high = whole setup unified on the dominant color.
    let unify = (params.scale as f32 / 200.0 * 0.7).clamp(0.05, 0.7);
    let cwf = (canvas.width.max(1)) as f32;
    let chf = (canvas.height.max(1)) as f32;
    for y in 0..canvas.height {
        let sy = ((y as f32 + 0.5) / chf * ch as f32 - 0.5).clamp(0.0, (ch - 1) as f32);
        for x in 0..canvas.width {
            let sx = ((x as f32 + 0.5) / cwf * cw as f32 - 0.5).clamp(0.0, (cw - 1) as f32);
            let local = sample_bilinear_f(&state.blur, cw, ch, sx, sy);
            let c = mix(local, dominant, unify);
            canvas.set_pixel(x, y, enhance_ambient(c));
        }
    }
}

fn box_blur_axis(src: &[f32], dst: &mut [f32], w: usize, h: usize, r: usize, horizontal: bool) {
    let (outer, inner) = if horizontal { (h, w) } else { (w, h) };
    let idx = |o: usize, i: usize| if horizontal { (o * w + i) * 3 } else { (i * w + o) * 3 };
    for o in 0..outer {
        for i in 0..inner {
            let lo = i.saturating_sub(r);
            let hi = (i + r).min(inner - 1);
            let mut s = [0.0f32; 3];
            for j in lo..=hi {
                let k = idx(o, j);
                s[0] += src[k];
                s[1] += src[k + 1];
                s[2] += src[k + 2];
            }
            let n = (hi - lo + 1) as f32;
            let d = idx(o, i);
            dst[d] = s[0] / n;
            dst[d + 1] = s[1] / n;
            dst[d + 2] = s[2] / n;
        }
    }
}

fn sample_bilinear_f(buf: &[f32], w: usize, h: usize, x: f32, y: f32) -> [f32; 3] {
    let x0 = x.floor() as usize;
    let y0 = y.floor() as usize;
    let x1 = (x0 + 1).min(w - 1);
    let y1 = (y0 + 1).min(h - 1);
    let (tx, ty) = (x - x0 as f32, y - y0 as f32);
    let at = |xx: usize, yy: usize| {
        let i = (yy * w + xx) * 3;
        [buf[i], buf[i + 1], buf[i + 2]]
    };
    mix(mix(at(x0, y0), at(x1, y0), tx), mix(at(x0, y1), at(x1, y1), tx), ty)
}

/// Screen colors copied 1:1 to LEDs look dull: desktop UIs are mostly dark and
/// desaturated. Lift the value curve (dark tones gain the most, white stays white)
/// and boost saturation while keeping the hue. Near-black stays off so letterbox
/// bars and dark scenes do not glow grey.
fn enhance_ambient(c: [f32; 3]) -> Rgb {
    const BLACK_FLOOR: f32 = 6.0 / 255.0;
    const VALUE_GAMMA: f32 = 0.55;
    const SATURATION_GAIN: f32 = 1.45;

    let max = c[0].max(c[1]).max(c[2]);
    if max <= BLACK_FLOOR {
        return Rgb::BLACK;
    }
    let min = c[0].min(c[1]).min(c[2]);
    let sat = (max - min) / max;
    let value = max.powf(VALUE_GAMMA);
    let new_sat = (sat * SATURATION_GAIN).min(1.0);

    // (max - ch) / (max - min) is 0 for the dominant channel and 1 for the weakest,
    // so rebuilding from it preserves hue.
    let channel = |ch: f32| {
        let rel = if sat > 0.0 { (max - ch) / (max - min) } else { 0.0 };
        value * (1.0 - new_sat * rel)
    };
    to_rgb([channel(c[0]), channel(c[1]), channel(c[2])])
}

// ---------------------------------------------------------------------------
// Audio
// ---------------------------------------------------------------------------

/// Spectrum bars interpolated across the setup with falling peak markers and
/// a bass-driven ambient glow behind them.
fn render_audio_spectrum(
    canvas: &mut Canvas2D,
    params: &EffectParams,
    input: &EffectFrameInput<'_>,
    state: &mut EffectState,
    dt: f32,
) {
    let idle;
    let bands: &[f32] = if input.audio_bands.is_empty() {
        idle = (0..16)
            .map(|i| 0.25 + 0.3 * ((i as f32 * 0.6 + input.time_secs * 2.0).sin() * 0.5 + 0.5))
            .collect::<Vec<_>>();
        &idle
    } else {
        input.audio_bands
    };
    let nb = bands.len();

    if state.peaks.len() != nb {
        state.peaks = vec![0.0; nb];
    }
    for (p, &b) in state.peaks.iter_mut().zip(bands) {
        *p = (*p - dt * 0.55).max(b.clamp(0.0, 1.0));
    }
    let bass_now = bands.iter().take(3).sum::<f32>() / 3.0_f32.min(nb as f32).max(1.0);
    let a = if bass_now > state.bass { 0.6 } else { 1.0 - (-dt / 0.25).exp() };
    state.bass += (bass_now - state.bass) * a;

    let primary = rgb_f(params.primary_color);
    let secondary = rgb_f(params.secondary_color);
    let background = scale3(primary, 0.05 + 0.4 * state.bass.powf(1.5));
    let w = (canvas.width.saturating_sub(1)).max(1) as f32;
    let h = (canvas.height.saturating_sub(1)).max(1) as f32;

    for x in 0..canvas.width {
        let pos = x as f32 / w * (nb - 1) as f32;
        let i0 = pos.floor() as usize;
        let i1 = (i0 + 1).min(nb - 1);
        let f = smoothstep(0.0, 1.0, pos - i0 as f32);
        let energy = bands[i0] + (bands[i1] - bands[i0]) * f;
        let peak = state.peaks[i0] + (state.peaks[i1] - state.peaks[i0]) * f;

        for y in 0..canvas.height {
            let level = 1.0 - y as f32 / h;
            let c = if level <= energy {
                let t = (level / energy.max(0.01)).clamp(0.0, 1.0);
                let col = mix(primary, secondary, t);
                // Bright cap at the top of each bar.
                mix(col, [1.0, 1.0, 1.0], smoothstep(0.85, 1.0, t) * 0.25)
            } else if (level - peak).abs() < 0.035 {
                mix(secondary, [1.0, 1.0, 1.0], 0.4)
            } else {
                background
            };
            canvas.set_pixel(x, y, to_rgb(c));
        }
    }
}
