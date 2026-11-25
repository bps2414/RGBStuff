pub mod ambilight;
pub mod audio;

pub use ambilight::{AMBILIGHT_H, AMBILIGHT_W, ScreenSampler, downsample_bgra_to_rgb};
pub use audio::{AUDIO_BANDS, AudioSpectrumSampler, compute_spectrum_bands};
