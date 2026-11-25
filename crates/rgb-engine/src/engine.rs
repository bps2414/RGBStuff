use crate::canvas::Canvas2D;
use crate::effects::{EffectFrameInput, EffectState, render_effect};
use crate::pacer::{BusFramePacer, DirtyFrameTracker};
use rgb_core::{AppConfig, DevicePlacement, EffectKind, EffectParams, LedPoint, Rgb};
use serde::{Deserialize, Serialize};
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum EngineStatus {
    Off,
    StaticZeroCpu,
    Dynamic { fps: u8 },
}

pub struct LightingEngine {
    pub canvas: Canvas2D,
    pub pacer: BusFramePacer,
    pub dirty: DirtyFrameTracker,
    start_instant: Instant,
    static_applied: bool,
    effect_state: EffectState,
}

impl Default for LightingEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl LightingEngine {
    pub fn new() -> Self {
        let mut pacer = BusFramePacer::new();
        pacer.configure_hz("asus-aura-0b05-19af", 60);
        pacer.configure_hz("aula-f75-he", 30);
        pacer.configure_hz("asgard-ddr4-smbus", 15);
        Self {
            canvas: Canvas2D::default(),
            pacer,
            dirty: DirtyFrameTracker::new(),
            start_instant: Instant::now(),
            static_applied: false,
            effect_state: EffectState::new(),
        }
    }

    pub fn mark_needs_apply(&mut self) {
        self.static_applied = false;
        self.dirty.invalidate_all();
    }

    /// Keeps a static frame pending (e.g. a bus was still inside its pacing window).
    pub fn defer_static_apply(&mut self) {
        self.static_applied = false;
    }

    pub fn status_for(config: &AppConfig) -> EngineStatus {
        if !config.power_on {
            return EngineStatus::Off;
        }
        let kind = config
            .active_profile()
            .map(|p| p.effect.kind)
            .unwrap_or(EffectKind::Solid);
        if kind == EffectKind::Off {
            EngineStatus::Off
        } else if kind.is_static() {
            EngineStatus::StaticZeroCpu
        } else {
            EngineStatus::Dynamic { fps: 60 }
        }
    }

    pub fn effective_brightness(config: &AppConfig) -> u8 {
        if !config.power_on {
            return 0;
        }
        let profile_b = config
            .active_profile()
            .map(|p| p.brightness)
            .unwrap_or(100);
        ((config.global_brightness as u16 * profile_b as u16) / 100) as u8
    }

    pub fn should_tick(&self, config: &AppConfig) -> bool {
        !self.static_applied || !matches!(Self::status_for(config), EngineStatus::Off | EngineStatus::StaticZeroCpu)
    }

    pub fn render_step(
        &mut self,
        config: &AppConfig,
        screen_frame: Option<(usize, usize, &[u8])>,
        audio_bands: &[f32],
    ) {
        let params = if config.power_on {
            config
                .active_profile()
                .map(|p| p.effect.clone())
                .unwrap_or_default()
        } else {
            EffectParams {
                kind: EffectKind::Off,
                ..EffectParams::default()
            }
        };

        let elapsed = self.start_instant.elapsed().as_secs_f32();
        let input = EffectFrameInput {
            time_secs: elapsed,
            screen_frame,
            audio_bands,
        };
        render_effect(&mut self.canvas, &params, &input, &mut self.effect_state);

        if params.kind.is_static() || !config.power_on {
            self.static_applied = true;
        } else {
            self.static_applied = false;
        }
    }

    pub fn sample_for_device(
        &mut self,
        config: &AppConfig,
        placement: &DevicePlacement,
        local_points: &[LedPoint],
        now: Instant,
        force: bool,
    ) -> Option<Vec<Rgb>> {
        if !force && !self.pacer.should_send(&placement.device_id, now) {
            return None;
        }
        let gain = Self::effective_brightness(config) as f32 / 100.0
            * placement.brightness.min(200) as f32
            / 100.0;
        let colors = self.canvas.sample_device(placement, local_points, gain);
        if !force && !self.dirty.is_dirty(&placement.device_id, &colors) {
            return None;
        }
        self.dirty.record(&placement.device_id, &colors);
        self.pacer.mark_sent(&placement.device_id, now);
        Some(colors)
    }
}
