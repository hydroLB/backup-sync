use std::time::Duration;
use tokio::time::{sleep, Instant};

/// Purpose: Executes a periodic async task on a fixed interval.
///
/// Inputs: an interval duration and a task closure.
/// Outputs: a continuously running loop.
/// Ties to: daemon scheduling loops.
/// Side effects: None.
/// Why: provide a simple, drift-aware scheduler for background work.
pub struct IntervalScheduler {
    pub interval: Duration,
}

impl IntervalScheduler {
    /// Purpose: Builds a scheduler from an interval in seconds.
    ///
    /// Inputs: the interval in seconds.
    /// Outputs: an `IntervalScheduler` instance.
    /// Ties to: config driven scheduling.
    /// Side effects: None.
    /// Why: make interval construction straightforward from config values.
    pub fn new_secs(secs: u64) -> Self {
        Self {
            interval: Duration::from_secs(secs),
        }
    }

    /// Purpose: Runs the provided async task at the configured interval.
    ///
    /// Inputs: an async task closure.
    /// Outputs: `()` while looping indefinitely.
    /// Ties to: periodic backup and verification cycles.
    /// Side effects: Runs the provided task and sleeps between intervals.
    /// Why: schedule recurring work while accounting for task duration.
    pub async fn run<F, Fut>(&self, mut task: F)
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = ()>,
    {
        loop {
            let start = Instant::now();
            task().await;
            let elapsed = start.elapsed();
            if elapsed < self.interval {
                sleep(self.interval - elapsed).await;
            }
        }
    }
}
