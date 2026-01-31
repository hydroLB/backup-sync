use crate::scheduling::throttling::Throttle;
use anyhow::{Context, Result};
use std::{
    fs,
    io::{BufReader, BufWriter, Read, Write},
    path::Path,
    time::{Duration, Instant},
};

/// Purpose: Copies data from source to destination with an optional throughput cap.
///
/// Inputs: the source path, destination path, throttle limit, and buffer size.
/// Outputs: `Ok(())` when the destination flushes successfully.
/// Ties to: `BackupExecutor::write_temp_copy` during backup staging.
/// Side effects: Reads and writes filesystem data and may sleep for throttling.
/// Why: provide predictable IO behavior while keeping memory usage bounded.
pub(super) fn copy_with_throttle(
    src: &Path,
    dst: &Path,
    max_bps: Option<u64>,
    buffer_bytes: usize,
    timeout: Option<Duration>,
) -> Result<()> {
    if buffer_bytes == 0 {
        anyhow::bail!("backup::execution::copy_with_throttle buffer_bytes must be > 0");
    }
    let start = Instant::now();
    let mut reader = BufReader::with_capacity(
        buffer_bytes,
        fs::File::open(src).with_context(|| format!("copy_with_throttle open source {:?}", src))?,
    );
    let mut writer = BufWriter::with_capacity(
        buffer_bytes,
        fs::File::create(dst)
            .with_context(|| format!("copy_with_throttle create destination {:?}", dst))?,
    );
    let mut buf = vec![0u8; buffer_bytes];
    let mut throttle = match max_bps {
        Some(limit) => Some(Throttle::new(limit)?),
        None => None,
    };
    loop {
        if let Some(timeout) = timeout {
            if start.elapsed() > timeout {
                anyhow::bail!(
                    "backup::execution::copy_with_throttle timed out after {:?} for {:?}",
                    timeout,
                    src
                );
            }
        }
        let n = reader
            .read(&mut buf)
            .with_context(|| format!("copy_with_throttle read from {:?}", src))?;
        if n == 0 {
            break;
        }
        writer
            .write_all(&buf[..n])
            .with_context(|| format!("copy_with_throttle write to temp {:?}", dst))?;
        if let Some(throttle) = throttle.as_mut() {
            throttle.record(n as u64);
        }
    }
    writer
        .flush()
        .with_context(|| format!("copy_with_throttle flush temp {:?}", dst))?;
    Ok(())
}
