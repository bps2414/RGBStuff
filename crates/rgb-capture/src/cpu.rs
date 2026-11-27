use std::time::{Duration, Instant};
use windows::Win32::Foundation::FILETIME;
use windows::Win32::System::Threading::GetSystemTimes;

const REFRESH: Duration = Duration::from_millis(250);

/// Whole-system CPU usage from `GetSystemTimes`. No thread, no handles: it is
/// polled from the render loop only while an effect asks for it.
#[derive(Default)]
pub struct CpuLoadSampler {
    last_times: Option<(u64, u64)>,
    last_at: Option<Instant>,
    load: f32,
}

fn ft(t: FILETIME) -> u64 {
    ((t.dwHighDateTime as u64) << 32) | t.dwLowDateTime as u64
}

impl CpuLoadSampler {
    pub fn new() -> Self {
        Self::default()
    }

    /// Latest usage in `0.0..=1.0`, refreshed at most every 250 ms.
    pub fn load(&mut self) -> f32 {
        if self.last_at.is_some_and(|t| t.elapsed() < REFRESH) {
            return self.load;
        }
        self.last_at = Some(Instant::now());
        let (mut idle, mut kernel, mut user) = (FILETIME::default(), FILETIME::default(), FILETIME::default());
        // SAFETY: the three pointers are valid, writable FILETIMEs on this stack frame.
        if unsafe { GetSystemTimes(Some(&mut idle), Some(&mut kernel), Some(&mut user)) }.is_err() {
            return self.load;
        }
        // Kernel time already includes idle time.
        let (idle, total) = (ft(idle), ft(kernel) + ft(user));
        if let Some((pi, pt)) = self.last_times {
            let dt = total.saturating_sub(pt);
            if dt > 0 {
                let busy = dt.saturating_sub(idle.saturating_sub(pi));
                self.load = (busy as f32 / dt as f32).clamp(0.0, 1.0);
            }
        }
        self.last_times = Some((idle, total));
        self.load
    }
}
