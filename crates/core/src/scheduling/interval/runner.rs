use std::time::Duration;
use tokio::sync::watch;
use tokio::time::{sleep, Instant};

/// Summary: Executes a periodic async task on a fixed interval.
///
/// Inputs: an interval duration and a task closure.
///
/// Outputs: a continuously running loop.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: daemon scheduling loops.
///
/// Why this exists: provide a simple, drift-aware scheduler for background work.
pub struct IntervalScheduler {
    pub interval: Duration,
}

impl IntervalScheduler {
    /// Summary: Builds a scheduler from an interval in seconds.
    ///
    /// Inputs: the interval in seconds.
    ///
    /// Outputs: an `IntervalScheduler` instance.
    ///
    /// Side effects: None.
    ///
    /// Error handling: Propagates contextual errors to the caller when operations fail.
    ///
    /// Ties to other methods: config driven scheduling.
    ///
    /// Why this exists: make interval construction straightforward from config values.
    pub fn new_secs(secs: u64) -> Self {
        Self {
            interval: Duration::from_secs(secs),
        }
    }

    /// Summary: Runs the provided async task at the configured interval.
    ///
    /// Inputs: an async task closure.
    ///
    /// Outputs: `()` while looping indefinitely.
    ///
    /// Side effects: Runs the provided task and sleeps between intervals.
    ///
    /// Error handling: Propagates contextual errors to the caller when operations fail.
    ///
    /// Ties to other methods: periodic backup and verification cycles.
    ///
    /// Why this exists: schedule recurring work while accounting for task duration.
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

    /// Summary: Runs the provided async task at the configured interval until shutdown is requested.
    ///
    /// Inputs: a shutdown receiver and an async task closure.
    ///
    /// Outputs: `()` after shutdown is requested.
    ///
    /// Side effects: Runs the provided task and sleeps between intervals.
    ///
    /// Error handling: Propagates contextual errors to the caller when operations fail.
    ///
    /// Ties to other methods: daemon runtime scheduling cancellation behavior.
    ///
    /// Why this exists: periodic jobs should support explicit cancellation for graceful shutdown.
    pub async fn run_until_shutdown<F, Fut>(
        &self,
        shutdown: &mut watch::Receiver<bool>,
        mut task: F,
    ) where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = ()>,
    {
        loop {
            if *shutdown.borrow() {
                break;
            }
            let start = Instant::now();
            task().await;
            let elapsed = start.elapsed();
            if elapsed < self.interval {
                tokio::select! {
                    changed = shutdown.changed() => {
                        match changed {
                            Ok(()) | Err(_) => break,
                        }
                    }
                    _ = sleep(self.interval - elapsed) => {}
                }
            }
        }
    }
}
