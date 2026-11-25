use std::collections::VecDeque;
use std::f32::consts::PI;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use rustfft::FftPlanner;
use rustfft::num_complex::Complex;
use wasapi::{
    AudioCaptureClient, AudioClient, Direction, SampleType, ShareMode, WasapiError, WaveFormat,
    deinitialize, get_default_device, initialize_mta,
};

/// Number of logarithmically spaced frequency bands exposed by [`AudioSpectrumSampler`].
pub const AUDIO_BANDS: usize = 16;

const FFT_SIZE: usize = 1024;
const SAMPLE_RATE_HZ: usize = 48_000;
const ATTACK_ALPHA: f32 = 0.55;
const RELEASE_ALPHA: f32 = 0.20;

/// Computes `num_bands` logarithmically spaced spectrum magnitudes normalized to `0.0..=1.0`
/// from up to `1024` mono `f32` samples using a Hann window and `rustfft`.
pub fn compute_spectrum_bands(samples: &[f32], num_bands: usize) -> Vec<f32> {
    if num_bands == 0 {
        return Vec::new();
    }
    if samples.is_empty() {
        return vec![0.0; num_bands];
    }

    let window_len = samples.len().min(FFT_SIZE);
    let input = &samples[samples.len() - window_len..];

    if !input.iter().any(|&s| s.is_finite() && s.abs() > 1e-9) {
        return vec![0.0; num_bands];
    }

    let mut buffer = vec![Complex::new(0.0_f32, 0.0_f32); FFT_SIZE];
    for (i, &sample) in input.iter().enumerate() {
        let clean = if sample.is_finite() { sample } else { 0.0 };
        let hann = if window_len <= 1 {
            1.0
        } else {
            0.5 * (1.0 - (2.0 * PI * i as f32 / (window_len - 1) as f32).cos())
        };
        buffer[i] = Complex::new(clean * hann, 0.0);
    }

    let mut planner = FftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(FFT_SIZE);
    fft.process(&mut buffer);

    let nyquist = FFT_SIZE / 2;
    let norm_scale = 1.0 / ((window_len as f32).max(1.0) * 0.25);
    let mags: Vec<f32> = (0..=nyquist)
        .map(|k| buffer[k].norm() * norm_scale)
        .collect();

    let min_bin = 1.0_f32;
    let max_bin = nyquist as f32;
    let mut bands = Vec::with_capacity(num_bands);

    for b in 0..num_bands {
        let t0 = b as f32 / num_bands as f32;
        let t1 = (b + 1) as f32 / num_bands as f32;
        let f0 = min_bin * (max_bin / min_bin).powf(t0);
        let f1 = min_bin * (max_bin / min_bin).powf(t1);

        let start = (f0.floor() as usize).clamp(1, nyquist);
        let end = (f1.ceil() as usize).clamp(start + 1, nyquist + 1);

        let mut sum_sq = 0.0_f32;
        let mut peak = 0.0_f32;
        for &m in &mags[start..end] {
            sum_sq += m * m;
            if m > peak {
                peak = m;
            }
        }
        let rms = (sum_sq / (end - start) as f32).sqrt();
        let level = 0.5 * peak + 0.5 * rms;

        let normalized = if level <= 1e-6 {
            0.0
        } else {
            let db = 20.0 * level.log10();
            ((db + 60.0) / 60.0).clamp(0.0, 1.0)
        };
        bands.push(normalized);
    }

    bands
}

/// Captures system loopback audio via WASAPI on a background thread and exposes
/// smoothed spectrum magnitudes across [`AUDIO_BANDS`] frequency bands.
pub struct AudioSpectrumSampler {
    latest: Arc<Mutex<Vec<f32>>>,
    running: Arc<AtomicBool>,
    stop_flag: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl Default for AudioSpectrumSampler {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioSpectrumSampler {
    /// Creates an inactive audio spectrum sampler. No WASAPI handles or threads are
    /// allocated until [`AudioSpectrumSampler::start`] is called.
    pub fn new() -> Self {
        Self {
            latest: Arc::new(Mutex::new(vec![0.0; AUDIO_BANDS])),
            running: Arc::new(AtomicBool::new(false)),
            stop_flag: Arc::new(AtomicBool::new(false)),
            worker: None,
        }
    }

    /// Starts the WASAPI loopback capture worker thread if not already running.
    pub fn start(&mut self) {
        if self.worker.is_some() {
            if self.running.load(Ordering::Acquire) {
                return;
            }
            if let Some(handle) = self.worker.take() {
                let _ = handle.join();
            }
        }

        self.stop_flag.store(false, Ordering::Release);
        self.running.store(true, Ordering::Release);

        let latest = Arc::clone(&self.latest);
        let running = Arc::clone(&self.running);
        let stop_flag = Arc::clone(&self.stop_flag);

        match thread::Builder::new()
            .name("rgb-capture-audio".to_string())
            .spawn(move || {
                run_audio_loop(latest, stop_flag);
                running.store(false, Ordering::Release);
            }) {
            Ok(handle) => {
                self.worker = Some(handle);
            }
            Err(_) => {
                self.running.store(false, Ordering::Release);
            }
        }
    }

    /// Signals the worker thread to stop, joins it, and releases all WASAPI handles.
    pub fn stop(&mut self) {
        self.stop_flag.store(true, Ordering::Release);
        if let Some(handle) = self.worker.take() {
            let _ = handle.join();
        }
        self.running.store(false, Ordering::Release);
        if let Ok(mut guard) = self.latest.lock() {
            guard.fill(0.0);
        }
    }

    /// Returns `true` while the capture worker thread is active.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Acquire)
    }

    /// Returns the latest smoothed [`AUDIO_BANDS`]-element spectrum vector in `0.0..=1.0`.
    pub fn latest_bands(&self) -> Vec<f32> {
        self.latest
            .lock()
            .map(|guard| guard.clone())
            .unwrap_or_else(|_| vec![0.0; AUDIO_BANDS])
    }
}

impl Drop for AudioSpectrumSampler {
    fn drop(&mut self) {
        self.stop();
    }
}

struct MtaComGuard {
    should_uninit: bool,
}

impl MtaComGuard {
    fn init() -> Self {
        let hr = initialize_mta();
        // S_OK (0) or S_FALSE (1) means COM was initialized by this call and must be uninitialized.
        Self {
            should_uninit: hr.0 == 0 || hr.0 == 1,
        }
    }
}

impl Drop for MtaComGuard {
    fn drop(&mut self) {
        if self.should_uninit {
            deinitialize();
        }
    }
}

fn sleep_interruptible(stop_flag: &AtomicBool, total: Duration) {
    let step = Duration::from_millis(25);
    let mut elapsed = Duration::ZERO;
    while elapsed < total && !stop_flag.load(Ordering::Acquire) {
        let chunk = step.min(total - elapsed);
        thread::sleep(chunk);
        elapsed += chunk;
    }
}

fn open_loopback_client() -> Result<(AudioClient, wasapi::Handle, AudioCaptureClient), WasapiError>
{
    let device = get_default_device(&Direction::Render)?;
    let mut audio_client = device.get_iaudioclient()?;

    let desired_format = WaveFormat::new(32, 32, &SampleType::Float, SAMPLE_RATE_HZ, 2, None);
    let (default_period, _min_period) = audio_client.get_periods()?;
    let buffer_hns = default_period.max(200_000);

    audio_client.initialize_client(
        &desired_format,
        buffer_hns,
        &Direction::Capture,
        &ShareMode::Shared,
        true,
    )?;

    let event_handle = audio_client.set_get_eventhandle()?;
    let capture_client = audio_client.get_audiocaptureclient()?;
    audio_client.start_stream()?;

    Ok((audio_client, event_handle, capture_client))
}

fn drain_stereo_f32_to_mono(
    byte_queue: &mut VecDeque<u8>,
    mono_ring: &mut VecDeque<f32>,
) -> usize {
    const BYTES_PER_FRAME: usize = 8; // 2 channels * sizeof(f32)
    let mut frames_added = 0;

    while byte_queue.len() >= BYTES_PER_FRAME {
        let mut frame = [0u8; BYTES_PER_FRAME];
        for slot in &mut frame {
            *slot = byte_queue.pop_front().unwrap_or(0);
        }
        let left = f32::from_le_bytes([frame[0], frame[1], frame[2], frame[3]]);
        let right = f32::from_le_bytes([frame[4], frame[5], frame[6], frame[7]]);
        let mono = 0.5 * (left + right);

        if mono_ring.len() >= FFT_SIZE {
            mono_ring.pop_front();
        }
        mono_ring.push_back(mono);
        frames_added += 1;
    }

    frames_added
}

fn smooth_towards(smoothed: &mut [f32], target: &[f32]) {
    for (cur, &tgt) in smoothed.iter_mut().zip(target.iter()) {
        let alpha = if tgt > *cur {
            ATTACK_ALPHA
        } else {
            RELEASE_ALPHA
        };
        *cur = (*cur + alpha * (tgt - *cur)).clamp(0.0, 1.0);
        if *cur < 1e-4 {
            *cur = 0.0;
        }
    }
}

fn run_audio_loop(latest: Arc<Mutex<Vec<f32>>>, stop_flag: Arc<AtomicBool>) {
    let _com = MtaComGuard::init();
    let mut smoothed = vec![0.0_f32; AUDIO_BANDS];
    let zeros = vec![0.0_f32; AUDIO_BANDS];

    while !stop_flag.load(Ordering::Acquire) {
        let (audio_client, event_handle, capture_client) = match open_loopback_client() {
            Ok(parts) => parts,
            Err(_) => {
                smooth_towards(&mut smoothed, &zeros);
                if let Ok(mut guard) = latest.lock() {
                    *guard = smoothed.clone();
                }
                sleep_interruptible(&stop_flag, Duration::from_millis(500));
                continue;
            }
        };

        let mut byte_queue: VecDeque<u8> = VecDeque::with_capacity(FFT_SIZE * 8 * 2);
        let mut mono_ring: VecDeque<f32> = VecDeque::with_capacity(FFT_SIZE);

        while !stop_flag.load(Ordering::Acquire) {
            let timed_out = match event_handle.wait_for_event(50) {
                Ok(()) => false,
                Err(WasapiError::EventTimeout) => true,
                Err(_) => break,
            };

            let mut read_failed = false;
            loop {
                let before = byte_queue.len();
                match capture_client.read_from_device_to_deque(&mut byte_queue) {
                    Ok(flags) => {
                        let added = byte_queue.len() - before;
                        if flags.silent && added > 0 {
                            let start = byte_queue.len() - added;
                            for b in byte_queue.range_mut(start..) {
                                *b = 0;
                            }
                        }
                        if added == 0 {
                            break;
                        }
                    }
                    Err(_) => {
                        read_failed = true;
                        break;
                    }
                }
            }
            if read_failed {
                break;
            }

            let frames_added = drain_stereo_f32_to_mono(&mut byte_queue, &mut mono_ring);
            if frames_added > 0 {
                let samples = mono_ring.make_contiguous();
                let target = compute_spectrum_bands(samples, AUDIO_BANDS);
                smooth_towards(&mut smoothed, &target);
            } else if timed_out {
                // Loopback events pause when no audio is playing; decay bands smoothly to 0.
                smooth_towards(&mut smoothed, &zeros);
            }

            if let Ok(mut guard) = latest.lock() {
                *guard = smoothed.clone();
            }
        }

        let _ = audio_client.stop_stream();
        drop(capture_client);
        drop(event_handle);
        drop(audio_client);

        if !stop_flag.load(Ordering::Acquire) {
            sleep_interruptible(&stop_flag, Duration::from_millis(250));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn silence_returns_all_zeros() {
        let silence = vec![0.0_f32; 1024];
        let bands = compute_spectrum_bands(&silence, AUDIO_BANDS);
        assert_eq!(bands.len(), AUDIO_BANDS);
        assert!(bands.iter().all(|&v| v == 0.0));
    }

    #[test]
    fn sine_wave_produces_peak_in_expected_band() {
        let sample_rate = 48_000.0_f32;
        let sine_440: Vec<f32> = (0..1024)
            .map(|i| 0.6 * (2.0 * PI * 440.0 * i as f32 / sample_rate).sin())
            .collect();
        let bands_440 = compute_spectrum_bands(&sine_440, AUDIO_BANDS);
        assert_eq!(bands_440.len(), AUDIO_BANDS);
        assert!(bands_440.iter().all(|&v| (0.0..=1.0).contains(&v)));

        let (peak_idx_440, &peak_val_440) = bands_440
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .unwrap();
        assert!(peak_val_440 > 0.5, "expected strong 440Hz peak, got {peak_val_440}");
        assert!(
            (3..=7).contains(&peak_idx_440),
            "expected 440Hz peak in lower/mid band, got band {peak_idx_440}"
        );

        let sine_1000: Vec<f32> = (0..1024)
            .map(|i| 0.6 * (2.0 * PI * 1000.0 * i as f32 / sample_rate).sin())
            .collect();
        let bands_1000 = compute_spectrum_bands(&sine_1000, AUDIO_BANDS);
        let (peak_idx_1000, &peak_val_1000) = bands_1000
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .unwrap();
        assert!(peak_val_1000 > 0.5, "expected strong 1000Hz peak, got {peak_val_1000}");
        assert!(
            peak_idx_1000 > peak_idx_440,
            "expected 1000Hz band ({peak_idx_1000}) > 440Hz band ({peak_idx_440})"
        );
    }
}
