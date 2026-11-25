use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use windows::Win32::Foundation::{E_ACCESSDENIED, HMODULE, S_FALSE, S_OK};
use windows::Win32::Graphics::Direct3D::{D3D_DRIVER_TYPE_UNKNOWN, D3D_FEATURE_LEVEL_11_0};
use windows::Win32::Graphics::Direct3D11::{
    D3D11_CPU_ACCESS_READ, D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_MAP_READ,
    D3D11_MAPPED_SUBRESOURCE, D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC, D3D11_USAGE_STAGING,
    D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Resource, ID3D11Texture2D,
};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_FORMAT, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_B8G8R8A8_UNORM_SRGB, DXGI_SAMPLE_DESC,
};
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory1, DXGI_ERROR_ACCESS_LOST, DXGI_ERROR_WAIT_TIMEOUT, DXGI_OUTDUPL_FRAME_INFO,
    IDXGIAdapter1, IDXGIFactory1, IDXGIOutput, IDXGIOutput1, IDXGIOutputDuplication, IDXGIResource,
};
use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize};
use windows::core::{IUnknown, Interface};

/// Width of the downsampled ambilight frame in pixels.
pub const AMBILIGHT_W: usize = 64;
/// Height of the downsampled ambilight frame in pixels.
pub const AMBILIGHT_H: usize = 36;

/// Downsamples a row-major BGRA8888 buffer (with `stride` bytes per row) into a
/// packed RGB888 buffer of dimensions `dst_w x dst_h`.
pub fn downsample_bgra_to_rgb(
    bgra: &[u8],
    src_w: usize,
    src_h: usize,
    stride: usize,
    dst_w: usize,
    dst_h: usize,
) -> Vec<u8> {
    let mut out = vec![0u8; dst_w.saturating_mul(dst_h).saturating_mul(3)];
    let effective_w = src_w.min(stride / 4);
    if effective_w == 0 || src_h == 0 || dst_w == 0 || dst_h == 0 || bgra.is_empty() {
        return out;
    }

    for dy in 0..dst_h {
        let y0 = dy * src_h / dst_h;
        let y1 = ((dy + 1) * src_h / dst_h).max(y0 + 1).min(src_h);
        let step_y = ((y1 - y0) / 8).max(1);

        for dx in 0..dst_w {
            let x0 = dx * effective_w / dst_w;
            let x1 = ((dx + 1) * effective_w / dst_w)
                .max(x0 + 1)
                .min(effective_w);
            let step_x = ((x1 - x0) / 8).max(1);

            let mut r_sum: u64 = 0;
            let mut g_sum: u64 = 0;
            let mut b_sum: u64 = 0;
            let mut count: u64 = 0;

            for y in (y0..y1).step_by(step_y) {
                let row_offset = y * stride;
                if row_offset >= bgra.len() {
                    break;
                }
                for x in (x0..x1).step_by(step_x) {
                    let px = row_offset + x * 4;
                    if px + 2 >= bgra.len() {
                        break;
                    }
                    b_sum += u64::from(bgra[px]);
                    g_sum += u64::from(bgra[px + 1]);
                    r_sum += u64::from(bgra[px + 2]);
                    count += 1;
                }
            }

            if count > 0 {
                let dst = (dy * dst_w + dx) * 3;
                out[dst] = (r_sum / count) as u8;
                out[dst + 1] = (g_sum / count) as u8;
                out[dst + 2] = (b_sum / count) as u8;
            }
        }
    }

    out
}

/// Captures the primary display via DXGI Desktop Duplication on a background thread
/// and exposes the latest downsampled `AMBILIGHT_W x AMBILIGHT_H` RGB888 frame.
pub struct ScreenSampler {
    latest: Arc<Mutex<Option<Vec<u8>>>>,
    running: Arc<AtomicBool>,
    stop_flag: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl Default for ScreenSampler {
    fn default() -> Self {
        Self::new()
    }
}

impl ScreenSampler {
    /// Creates an inactive screen sampler. No DXGI/D3D11 resources are allocated
    /// until [`ScreenSampler::start`] is called.
    pub fn new() -> Self {
        Self {
            latest: Arc::new(Mutex::new(None)),
            running: Arc::new(AtomicBool::new(false)),
            stop_flag: Arc::new(AtomicBool::new(false)),
            worker: None,
        }
    }

    /// Starts the DXGI Desktop Duplication capture worker thread if not already running.
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
            .name("rgb-capture-screen".to_string())
            .spawn(move || {
                run_capture_loop(latest, stop_flag);
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

    /// Signals the worker thread to stop, joins it, and releases all DXGI/D3D11 handles.
    pub fn stop(&mut self) {
        self.stop_flag.store(true, Ordering::Release);
        if let Some(handle) = self.worker.take() {
            let _ = handle.join();
        }
        self.running.store(false, Ordering::Release);
    }

    /// Returns `true` while the capture worker thread is active.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Acquire)
    }

    /// Returns the most recently captured `AMBILIGHT_W * AMBILIGHT_H * 3` RGB888 frame, if any.
    pub fn latest_frame(&self) -> Option<Vec<u8>> {
        self.latest.lock().ok().and_then(|guard| guard.clone())
    }
}

impl Drop for ScreenSampler {
    fn drop(&mut self) {
        self.stop();
    }
}

struct DuplicationSession {
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    duplication: IDXGIOutputDuplication,
    staging: Option<(ID3D11Texture2D, u32, u32, DXGI_FORMAT)>,
}

struct FrameGuard(IDXGIOutputDuplication);

impl Drop for FrameGuard {
    fn drop(&mut self) {
        let _ = unsafe { self.0.ReleaseFrame() };
    }
}

struct ComGuard {
    should_uninit: bool,
}

impl ComGuard {
    fn init_mta() -> Self {
        let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        Self {
            should_uninit: hr == S_OK || hr == S_FALSE,
        }
    }
}

impl Drop for ComGuard {
    fn drop(&mut self) {
        if self.should_uninit {
            unsafe { CoUninitialize() };
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

fn open_duplication_session() -> windows::core::Result<DuplicationSession> {
    let factory: IDXGIFactory1 = unsafe { CreateDXGIFactory1() }?;
    let adapter: IDXGIAdapter1 = unsafe { factory.EnumAdapters1(0) }?;

    let mut device: Option<ID3D11Device> = None;
    let mut context: Option<ID3D11DeviceContext> = None;
    unsafe {
        D3D11CreateDevice(
            &adapter,
            D3D_DRIVER_TYPE_UNKNOWN,
            HMODULE::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            Some(&[D3D_FEATURE_LEVEL_11_0]),
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            Some(&mut context),
        )
    }?;

    let device = device.ok_or_else(windows::core::Error::from_thread)?;
    let context = context.ok_or_else(windows::core::Error::from_thread)?;

    let output: IDXGIOutput = unsafe { adapter.EnumOutputs(0) }?;
    let output1: IDXGIOutput1 = output.cast()?;
    let device_unknown: IUnknown = device.cast()?;
    let duplication: IDXGIOutputDuplication = unsafe { output1.DuplicateOutput(&device_unknown) }?;

    Ok(DuplicationSession {
        device,
        context,
        duplication,
        staging: None,
    })
}

fn ensure_staging_texture(
    session: &mut DuplicationSession,
    width: u32,
    height: u32,
    format: DXGI_FORMAT,
) -> windows::core::Result<ID3D11Texture2D> {
    if let Some((tex, w, h, fmt)) = &session.staging
        && *w == width
        && *h == height
        && *fmt == format
    {
        return Ok(tex.clone());
    }

    let desc = D3D11_TEXTURE2D_DESC {
        Width: width,
        Height: height,
        MipLevels: 1,
        ArraySize: 1,
        Format: format,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        Usage: D3D11_USAGE_STAGING,
        BindFlags: 0,
        CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
        MiscFlags: 0,
    };

    let mut tex: Option<ID3D11Texture2D> = None;
    unsafe { session.device.CreateTexture2D(&desc, None, Some(&mut tex)) }?;
    let tex = tex.ok_or_else(windows::core::Error::from_thread)?;
    session.staging = Some((tex.clone(), width, height, format));
    Ok(tex)
}

fn capture_one_frame(
    session: &mut DuplicationSession,
    latest: &Mutex<Option<Vec<u8>>>,
) -> windows::core::Result<()> {
    let mut frame_info = DXGI_OUTDUPL_FRAME_INFO::default();
    let mut resource: Option<IDXGIResource> = None;

    unsafe {
        session
            .duplication
            .AcquireNextFrame(50, &mut frame_info, &mut resource)
    }?;
    let _frame_guard = FrameGuard(session.duplication.clone());

    let Some(resource) = resource else {
        return Ok(());
    };
    if frame_info.LastPresentTime == 0 {
        return Ok(());
    }

    let src_tex: ID3D11Texture2D = resource.cast()?;
    let mut desc = D3D11_TEXTURE2D_DESC::default();
    unsafe { src_tex.GetDesc(&mut desc) };

    if desc.Format != DXGI_FORMAT_B8G8R8A8_UNORM && desc.Format != DXGI_FORMAT_B8G8R8A8_UNORM_SRGB
    {
        return Ok(());
    }

    let staging_tex = ensure_staging_texture(session, desc.Width, desc.Height, desc.Format)?;
    let dst_res: ID3D11Resource = staging_tex.cast()?;
    let src_res: ID3D11Resource = src_tex.cast()?;

    unsafe { session.context.CopyResource(&dst_res, &src_res) };

    let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
    unsafe {
        session
            .context
            .Map(&dst_res, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
    }?;

    let width = desc.Width as usize;
    let height = desc.Height as usize;
    let stride = mapped.RowPitch as usize;

    let frame = if !mapped.pData.is_null() && stride >= width * 4 && height > 0 {
        let total_bytes = stride.saturating_mul(height);
        let bytes = unsafe { std::slice::from_raw_parts(mapped.pData as *const u8, total_bytes) };
        Some(downsample_bgra_to_rgb(
            bytes,
            width,
            height,
            stride,
            AMBILIGHT_W,
            AMBILIGHT_H,
        ))
    } else {
        None
    };

    unsafe { session.context.Unmap(&dst_res, 0) };

    if let Some(rgb) = frame
        && let Ok(mut guard) = latest.lock()
    {
        *guard = Some(rgb);
    }

    Ok(())
}

fn run_capture_loop(latest: Arc<Mutex<Option<Vec<u8>>>>, stop_flag: Arc<AtomicBool>) {
    let _com = ComGuard::init_mta();

    while !stop_flag.load(Ordering::Acquire) {
        let mut session = match open_duplication_session() {
            Ok(session) => session,
            Err(_) => {
                sleep_interruptible(&stop_flag, Duration::from_millis(250));
                continue;
            }
        };

        while !stop_flag.load(Ordering::Acquire) {
            match capture_one_frame(&mut session, &latest) {
                Ok(()) => {
                    sleep_interruptible(&stop_flag, Duration::from_millis(8));
                }
                Err(err) if err.code() == DXGI_ERROR_WAIT_TIMEOUT => {}
                Err(err)
                    if err.code() == DXGI_ERROR_ACCESS_LOST || err.code() == E_ACCESSDENIED =>
                {
                    break;
                }
                Err(_) => {
                    break;
                }
            }
        }

        drop(session);
        if !stop_flag.load(Ordering::Acquire) {
            sleep_interruptible(&stop_flag, Duration::from_millis(250));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn downsample_bgra_swaps_channels_and_respects_stride() {
        // 4x2 BGRA image with 4 extra padding bytes per row (stride = 20).
        // Left 2x2 block: all pixels (B=10, G=20, R=200, A=255) -> RGB (200, 20, 10)
        // Right 2x2 block: row 0 = (255, 0, 0, 255), row 1 = (0, 100, 0, 255) -> avg RGB (0, 50, 127)
        #[rustfmt::skip]
        let bgra: Vec<u8> = vec![
            // Row 0 (16 pixel bytes + 4 padding bytes)
            10, 20, 200, 255,   10, 20, 200, 255,   255, 0, 0, 255,   255, 0, 0, 255,   0xFF, 0xFF, 0xFF, 0xFF,
            // Row 1 (16 pixel bytes + 4 padding bytes)
            10, 20, 200, 255,   10, 20, 200, 255,   0, 100, 0, 255,   0, 100, 0, 255,   0xEE, 0xEE, 0xEE, 0xEE,
        ];

        let rgb = downsample_bgra_to_rgb(&bgra, 4, 2, 20, 2, 1);
        assert_eq!(rgb, vec![200, 20, 10, 0, 50, 127]);
    }
}
