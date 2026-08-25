use std::time::Duration;
use tokio::sync::watch;
use tokio::time::{sleep, Instant};

/// Provide a simple, drift-aware scheduler for background work.
pub struct IntervalScheduler {
    pub interval: Duration,
}

impl IntervalScheduler {
    /// Make interval construction straightforward from config values.
    pub fn new_secs(secs: u64) -> Self {
        Self {
            interval: Duration::from_secs(secs),
        }
    }

    /// Schedule recurring work while accounting for task duration.
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

    /// Periodic jobs should support explicit cancellation for graceful shutdown.
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
