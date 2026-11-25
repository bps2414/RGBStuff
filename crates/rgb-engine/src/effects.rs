use crate::canvas::Canvas2D;
use rgb_core::{EffectKind, EffectParams, Rgb, hsv_to_rgb};
use std::f32::consts::TAU;

pub struct EffectFrameInput<'a> {
    pub time_secs: f32,
    pub screen_frame: Option<(usize, usize, &'a [u8])>,
    pub audio_bands: &'a [f32],
}

pub fn render_effect(canvas: &mut Canvas2D, params: &EffectParams, input: &EffectFrameInput<'_>) {
    match params.kind {
        EffectKind::Off => canvas.fill(Rgb::BLACK),
        EffectKind::Solid => canvas.fill(params.primary_color),
        EffectKind::Gradient => render_gradient(canvas, params),
        EffectKind::RainbowWave => render_rainbow_wave(canvas, params, input.time_secs),
        EffectKind::Breathing => render_breathing(canvas, params, input.time_secs),
        EffectKind::DualPulse => render_dual_pulse(canvas, params, input.time_secs),
        EffectKind::Starlight => render_starlight(canvas, params, input.time_secs),
        EffectKind::Ambilight => render_ambilight(canvas, params, input),
        EffectKind::AudioSpectrum => render_audio_spectrum(canvas, params, input),
    }
}

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
    let speed = 0.15 + (params.speed as f32 / 100.0) * 1.4;
    let scale = (params.scale.max(10) as f32) / 100.0;
    let w = (canvas.width.saturating_sub(1)).max(1) as f32;
    let h = (canvas.height.saturating_sub(1)).max(1) as f32;

    for y in 0..canvas.height {
        let ny = y as f32 / h;
        for x in 0..canvas.width {
            let nx = x as f32 / w;
            let proj = nx * cos_a + ny * sin_a;
            let hue = (proj * 360.0 * scale - time_secs * speed * 180.0).rem_euclid(360.0);
            canvas.set_pixel(x, y, hsv_to_rgb(hue, 0.92, 1.0));
        }
    }
}

fn render_breathing(canvas: &mut Canvas2D, params: &EffectParams, time_secs: f32) {
    let freq = 0.2 + (params.speed as f32 / 100.0) * 1.2;
    let wave = ((time_secs * freq * TAU).sin() + 1.0) * 0.5;
    let smooth = wave * wave * (3.0 - 2.0 * wave);
    let color = params.secondary_color.lerp(params.primary_color, smooth);
    canvas.fill(color);
}

fn render_dual_pulse(canvas: &mut Canvas2D, params: &EffectParams, time_secs: f32) {
    let rad = params.angle_deg.to_radians();
    let (sin_a, cos_a) = rad.sin_cos();
    let speed = 0.25 + (params.speed as f32 / 100.0) * 1.6;
    let scale = (params.scale.max(10) as f32) / 100.0;
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

fn render_starlight(canvas: &mut Canvas2D, params: &EffectParams, time_secs: f32) {
    let speed = 0.4 + (params.speed as f32 / 100.0) * 2.4;
    let base = params.secondary_color.scale_brightness(28);
    for y in 0..canvas.height {
        for x in 0..canvas.width {
            let hash = ((x as u32).wrapping_mul(374761393) ^ (y as u32).wrapping_mul(668265263))
                .wrapping_mul(1274126177);
            let phase = (hash & 0xFFFF) as f32 / 65535.0 * TAU;
            let sparkle = ((time_secs * speed + phase).sin()).max(0.0).powi(6);
            canvas.set_pixel(x, y, base.lerp(params.primary_color, sparkle));
        }
    }
}

fn render_ambilight(canvas: &mut Canvas2D, params: &EffectParams, input: &EffectFrameInput<'_>) {
    if let Some((sw, sh, data)) = input.screen_frame
        && sw > 0
        && sh > 0
        && data.len() >= sw * sh * 3
    {
        for y in 0..canvas.height {
            let sy = (y * sh) / canvas.height;
            for x in 0..canvas.width {
                let sx = (x * sw) / canvas.width;
                let idx = (sy * sw + sx) * 3;
                canvas.set_pixel(x, y, Rgb::new(data[idx], data[idx + 1], data[idx + 2]));
            }
        }
        return;
    }

    // Smooth ambient fallback when DXGI frame is not yet connected.
    render_dual_pulse(canvas, params, input.time_secs * 0.45);
}

fn render_audio_spectrum(canvas: &mut Canvas2D, params: &EffectParams, input: &EffectFrameInput<'_>) {
    let bands = input.audio_bands;
    let w = (canvas.width.saturating_sub(1)).max(1) as f32;
    let h = (canvas.height.saturating_sub(1)).max(1) as f32;

    for y in 0..canvas.height {
        let inv_y = 1.0 - (y as f32 / h);
        for x in 0..canvas.width {
            let nx = x as f32 / w;
            let energy = if !bands.is_empty() {
                let idx = ((nx * bands.len() as f32) as usize).min(bands.len() - 1);
                bands[idx].clamp(0.0, 1.0)
            } else {
                let wave = ((nx * 4.0 * TAU + input.time_secs * 3.0).sin() + 1.0) * 0.5;
                0.25 + 0.55 * wave
            };

            let color = if inv_y <= energy {
                let t = (inv_y / energy.max(0.01)).clamp(0.0, 1.0);
                params.primary_color.lerp(params.secondary_color, t)
            } else {
                params.primary_color.scale_brightness(8)
            };
            canvas.set_pixel(x, y, color);
        }
    }
}
