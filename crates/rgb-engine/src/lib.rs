pub mod canvas;
pub mod effects;
pub mod engine;
pub mod pacer;

pub use canvas::Canvas2D;
pub use effects::{EffectFrameInput, EffectState, render_effect};
pub use engine::{EngineStatus, LightingEngine};
pub use pacer::{BusFramePacer, DirtyFrameTracker};

#[cfg(test)]
mod tests {
    use super::*;
    use rgb_core::{AppConfig, DevicePlacement, EffectKind, EffectParams, LedPoint, Rgb};
    use std::time::{Duration, Instant};

    #[test]
    fn bilinear_sampling_interpolates_corners() {
        let mut canvas = Canvas2D::new(2, 2);
        canvas.set_pixel(0, 0, Rgb::new(0, 0, 0));
        canvas.set_pixel(1, 0, Rgb::new(200, 0, 0));
        canvas.set_pixel(0, 1, Rgb::new(0, 200, 0));
        canvas.set_pixel(1, 1, Rgb::new(200, 200, 0));

        let mid = canvas.sample_bilinear(0.5, 0.5);
        assert_eq!(mid, Rgb::new(100, 100, 0));
    }

    #[test]
    fn static_effects_enable_zero_cpu_sleep() {
        let mut cfg = AppConfig::default();
        cfg.active_profile_id = "work".into();
        assert_eq!(
            LightingEngine::status_for(&cfg),
            EngineStatus::StaticZeroCpu
        );

        cfg.active_profile_id = "gaming".into();
        assert_eq!(
            LightingEngine::status_for(&cfg),
            EngineStatus::StaticZeroCpu
        );

        cfg.active_profile_id = "ambilight".into();
        assert_eq!(
            LightingEngine::status_for(&cfg),
            EngineStatus::Dynamic { fps: 60 }
        );

        cfg.power_on = false;
        assert_eq!(LightingEngine::status_for(&cfg), EngineStatus::Off);
    }

    #[test]
    fn dirty_tracker_skips_identical_frames_and_pacer_throttles() {
        let mut engine = LightingEngine::new();
        let mut cfg = AppConfig::default();
        cfg.active_profile_id = "work".into();
        cfg.global_brightness = 100;
        if let Some(p) = cfg.active_profile_mut() {
            p.brightness = 100;
            p.effect = EffectParams {
                kind: EffectKind::Solid,
                primary_color: Rgb::new(10, 20, 30),
                ..EffectParams::default()
            };
        }

        let placement = DevicePlacement {
            device_id: "asgard-ddr4-smbus".into(),
            x: 0.0,
            y: 0.0,
            width: 0.5,
            height: 0.5,
            rotation_deg: 0.0,
            led_count: 4,
            brightness: 100,
        };
        let pts = vec![LedPoint { x: 0.5, y: 0.5 }; 4];

        engine.render_step(&cfg, None, &[]);
        assert!(!engine.should_tick(&cfg));

        let t0 = Instant::now();
        let first = engine.sample_for_device(&cfg, &placement, &pts, t0, false);
        assert_eq!(first, Some(vec![Rgb::new(10, 20, 30); 4]));

        // Too soon for 15 Hz bus
        let too_soon = engine.sample_for_device(&cfg, &placement, &pts, t0 + Duration::from_millis(10), false);
        assert_eq!(too_soon, None);

        // Enough time elapsed, but identical color -> skipped by dirty tracker
        let later_same = engine.sample_for_device(&cfg, &placement, &pts, t0 + Duration::from_millis(100), false);
        assert_eq!(later_same, None);
    }
}
