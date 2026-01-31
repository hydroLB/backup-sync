use anyhow::{bail, Result};
use std::time::{Duration, Instant};

/// Purpose: Throttles byte throughput to a fixed bytes per second rate.
///
/// Inputs: the maximum bytes per second and a stream of written bytes.
/// Outputs: a sleep delay when the caller exceeds the allowance.
/// Ties to: copy and IO heavy operations that need rate control.
/// Side effects: None.
/// Why: prevent IO saturation while keeping throughput predictable.
pub struct Throttle {
    max_bytes_per_second: u64,
    start: Instant,
    written: u64,
}

impl Throttle {
    /// Purpose: Builds a throughput throttle for a fixed bytes per second limit.
    ///
    /// Inputs: the maximum bytes per second.
    /// Outputs: a configured `Throttle` instance.
    /// Ties to: copy operations that need bandwidth caps.
    /// Side effects: Reads the system clock to initialize timing.
    /// Why: provide reusable throttling without duplicating math.
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

    /// Purpose: Records bytes written and sleeps when the rate exceeds the limit.
    ///
    /// Inputs: the number of bytes just written.
    /// Outputs: `()` after any required sleep.
    /// Ties to: copy loops to enforce throughput caps.
    /// Side effects: Sleeps to enforce throughput limits.
    /// Why: smooth throughput without complex token buckets.
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
