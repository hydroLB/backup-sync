use anyhow::{bail, Result};
use std::time::{Duration, Instant};

/// Prevent IO saturation while keeping throughput predictable.
pub struct Throttle {
    max_bytes_per_second: u64,
    start: Instant,
    written: u64,
}

impl Throttle {
    /// Provide reusable throttling without duplicating math.
    pub fn new(max_bytes_per_second: u64) -> Result<Self> {
        if max_bytes_per_second == 0 {
            bail!("scheduling::throttling::Throttle::new max_bytes_per_second must be > 0");
        }
        Ok(Self {
            max_bytes_per_second,
            start: Instant::now(),
            written: 0,
        })
    }

    /// Smooth throughput without complex token buckets.
    pub fn record(&mut self, bytes: u64) {
        self.written = self.written.saturating_add(bytes);
        let elapsed = self.start.elapsed();
        if elapsed.is_zero() {
            return;
        }
        let allowed = (self.max_bytes_per_second as f64) * elapsed.as_secs_f64();
        if (self.written as f64) > allowed {
            let over = self.written as f64 - allowed;
            let sleep_secs = over / self.max_bytes_per_second as f64;
            if sleep_secs > 0.0 {
                std::thread::sleep(Duration::from_secs_f64(sleep_secs));
            }
        }
    }
}
