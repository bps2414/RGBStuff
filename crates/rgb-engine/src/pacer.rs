use rgb_core::Rgb;
use std::collections::HashMap;
use std::time::{Duration, Instant};

#[derive(Debug)]
pub struct BusFramePacer {
    min_intervals: HashMap<String, Duration>,
    last_sent: HashMap<String, Instant>,
}

impl Default for BusFramePacer {
    fn default() -> Self {
        Self::new()
    }
}

impl BusFramePacer {
    pub fn new() -> Self {
        Self {
            min_intervals: HashMap::new(),
            last_sent: HashMap::new(),
        }
    }

    pub fn configure_hz(&mut self, device_id: impl Into<String>, max_hz: u32) {
        let hz = max_hz.max(1);
        let interval = Duration::from_secs_f64(1.0 / hz as f64);
        self.min_intervals.insert(device_id.into(), interval);
    }

    pub fn should_send(&self, device_id: &str, now: Instant) -> bool {
        let Some(interval) = self.min_intervals.get(device_id) else {
            return true;
        };
        match self.last_sent.get(device_id) {
            Some(prev) => now.duration_since(*prev) >= *interval,
            None => true,
        }
    }

    pub fn mark_sent(&mut self, device_id: &str, now: Instant) {
        self.last_sent.insert(device_id.to_string(), now);
    }

    pub fn clear(&mut self, device_id: &str) {
        self.last_sent.remove(device_id);
    }
}

#[derive(Debug, Default)]
pub struct DirtyFrameTracker {
    last_frames: HashMap<String, Vec<Rgb>>,
}

impl DirtyFrameTracker {
    pub fn new() -> Self {
        Self {
            last_frames: HashMap::new(),
        }
    }

    /// Returns `true` if `frame` differs from the last accepted frame for `device_id`.
    pub fn is_dirty(&self, device_id: &str, frame: &[Rgb]) -> bool {
        match self.last_frames.get(device_id) {
            Some(prev) => prev.as_slice() != frame,
            None => true,
        }
    }

    pub fn record(&mut self, device_id: &str, frame: &[Rgb]) {
        self.last_frames
            .insert(device_id.to_string(), frame.to_vec());
    }

    pub fn invalidate(&mut self, device_id: &str) {
        self.last_frames.remove(device_id);
    }

    pub fn invalidate_all(&mut self) {
        self.last_frames.clear();
    }
}
