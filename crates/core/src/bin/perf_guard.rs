use anyhow::{Context, Result};
use backup_core::backup::retention;
use backup_core::fs::hashing::hash_file;
use serde::{Deserialize, Serialize};
use std::env;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::time::Instant;
use tempfile::TempDir;

/// Purpose: Capture timing metrics for a single run.
///
/// Inputs: Timing measurements for hashing and retention.
/// Outputs: A serialized JSON payload with timing metrics.
/// Ties to: Baseline comparisons in the perf guard.
/// Side effects: None.
/// Why: Standardize how performance metrics are recorded and compared.
#[derive(Debug, Serialize, Deserialize)]
struct PerfMetrics {
    hash_ms: u128,
    retention_ms: u128,
}

/// Purpose: Provide the performance guard configuration.
///
/// Inputs: CLI args and environment.
/// Outputs: Parsed configuration with baseline path and tolerance.
/// Ties to: CLI handling in `main`.
/// Side effects: None.
/// Why: Keep the perf guard configuration explicit and testable.
#[derive(Debug)]
struct PerfConfig {
    baseline_path: PathBuf,
    max_ratio: f64,
    record: bool,
}

/// Purpose: Parse CLI args into a structured config.
///
/// Inputs: CLI args from `std::env::args`.
/// Outputs: A `PerfConfig` instance.
/// Ties to: `main` and baseline checks.
/// Side effects: Reads CLI arguments from the process environment.
/// Why: Avoid scattered argument parsing logic.
fn parse_args() -> Result<PerfConfig> {
    let mut baseline_path = PathBuf::from("docs/perf-baseline.json");
    let mut max_ratio = 1.5f64;
    let mut record = false;
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--baseline" => {
                let path = args
                    .next()
                    .context("perf_guard::parse_args missing value for --baseline")?;
                baseline_path = PathBuf::from(path);
            }
            "--max-ratio" => {
                let ratio = args
                    .next()
                    .context("perf_guard::parse_args missing value for --max-ratio")?;
                max_ratio = ratio.parse().with_context(|| {
                    format!("perf_guard::parse_args invalid --max-ratio {}", ratio)
                })?;
            }
            "--record" => {
                record = true;
            }
            _ => {}
        }
    }
    Ok(PerfConfig {
        baseline_path,
        max_ratio,
        record,
    })
}

/// Purpose: Create a deterministic file payload for hashing.
///
/// Inputs: `dir` as the temp directory, `bytes` as size.
/// Outputs: Path to the created file.
/// Ties to: Hashing measurements.
/// Side effects: Writes a file to disk.
/// Why: Keep hashing input consistent across runs.
fn create_payload(dir: &TempDir, bytes: usize) -> Result<PathBuf> {
    let path = dir.path().join("payload.bin");
    let mut file = File::create(&path).with_context(|| {
        format!(
            "perf_guard::create_payload failed to create file {:?}",
            path
        )
    })?;
    let buf = vec![0xCD; bytes];
    file.write_all(&buf)
        .with_context(|| format!("perf_guard::create_payload failed to write file {:?}", path))?;
    Ok(path)
}

/// Purpose: Create retention-style files for pruning tests.
///
/// Inputs: `dir` as the temp directory, `count` as file count.
/// Outputs: Vector of file paths.
/// Ties to: Retention enforcement measurements.
/// Side effects: Writes files to disk.
/// Why: Model retention workloads with sortable timestamps.
fn create_retention_files(dir: &TempDir, count: usize) -> Result<Vec<PathBuf>> {
    let mut paths = Vec::with_capacity(count);
    for i in 0..count {
        let name = format!("20240101-{:06}__file.txt", i);
        let path = dir.path().join(name);
        File::create(&path).with_context(|| {
            format!(
                "perf_guard::create_retention_files failed to create file {:?}",
                path
            )
        })?;
        paths.push(path);
    }
    Ok(paths)
}

/// Purpose: Measure hashing and retention timings for the perf guard.
///
/// Inputs: None.
/// Outputs: A `PerfMetrics` struct with timing data.
/// Ties to: Baseline recording and comparison.
/// Side effects: Reads and writes temporary files.
/// Why: Track real hot paths for regression detection.
fn measure_metrics() -> Result<PerfMetrics> {
    let temp = TempDir::new().context("perf_guard::measure_metrics failed to create temp dir")?;
    let payload = create_payload(&temp, 8 * 1024 * 1024)?;
    let hash_start = Instant::now();
    for _ in 0..5 {
        let _ = hash_file(&payload).with_context(|| {
            format!(
                "perf_guard::measure_metrics hash_file failed for {:?}",
                payload
            )
        })?;
    }
    let hash_ms = hash_start.elapsed().as_millis();

    let retention_start = Instant::now();
    let files = create_retention_files(&temp, 200)?;
    let _ = retention::enforce(100, files)
        .context("perf_guard::measure_metrics retention::enforce failed during retention check")?;
    let retention_ms = retention_start.elapsed().as_millis();

    Ok(PerfMetrics {
        hash_ms,
        retention_ms,
    })
}

/// Purpose: Load a performance baseline from disk.
///
/// Inputs: Baseline path.
/// Outputs: Parsed `PerfMetrics` data.
/// Ties to: Baseline checks in `main`.
/// Side effects: Reads a JSON file.
/// Why: Compare current performance to a committed baseline.
fn load_baseline(path: &PathBuf) -> Result<PerfMetrics> {
    let mut file = File::open(path)
        .with_context(|| format!("perf_guard::load_baseline failed to open {:?}", path))?;
    let mut buf = String::new();
    file.read_to_string(&mut buf)
        .with_context(|| format!("perf_guard::load_baseline failed to read {:?}", path))?;
    serde_json::from_str(&buf)
        .with_context(|| format!("perf_guard::load_baseline failed to parse {:?}", path))
}

/// Purpose: Write a performance baseline to disk.
///
/// Inputs: Baseline path and metrics to serialize.
/// Outputs: None.
/// Ties to: `--record` flow in `main`.
/// Side effects: Writes a JSON file to disk.
/// Why: Persist a reproducible baseline for future comparisons.
fn write_baseline(path: &PathBuf, metrics: &PerfMetrics) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "perf_guard::write_baseline failed to create directory {:?}",
                parent
            )
        })?;
    }
    let body = serde_json::to_string_pretty(metrics)
        .context("perf_guard::write_baseline failed to serialize metrics")?;
    fs::write(path, body)
        .with_context(|| format!("perf_guard::write_baseline failed to write {:?}", path))?;
    Ok(())
}

/// Purpose: Compare metrics against the baseline and enforce a slowdown ratio.
///
/// Inputs: Baseline metrics, current metrics, and max ratio.
/// Outputs: `Ok(())` when within tolerance, `Err` otherwise.
/// Ties to: CI performance gating.
/// Side effects: None.
/// Why: Fail fast on performance regressions.
fn compare_metrics(baseline: &PerfMetrics, current: &PerfMetrics, max_ratio: f64) -> Result<()> {
    let hash_ratio = current.hash_ms as f64 / baseline.hash_ms as f64;
    let retention_ratio = current.retention_ms as f64 / baseline.retention_ms as f64;

    if hash_ratio > max_ratio {
        anyhow::bail!(
            "perf_guard::compare_metrics hash regression {:.2}x > {:.2}x",
            hash_ratio,
            max_ratio
        );
    }
    if retention_ratio > max_ratio {
        anyhow::bail!(
            "perf_guard::compare_metrics retention regression {:.2}x > {:.2}x",
            retention_ratio,
            max_ratio
        );
    }
    Ok(())
}

/// Purpose: Execute the perf guard entrypoint.
///
/// Inputs: CLI args for baseline path, ratio, and record mode.
/// Outputs: `Ok(())` on success or a descriptive error on failure.
/// Ties to: `scripts/perf/check_baseline.sh` and CI.
/// Side effects: Reads and writes files, runs performance measurements.
/// Why: Provide a reproducible performance gate for hot paths.
fn main() -> Result<()> {
    let config = parse_args()?;
    let metrics = measure_metrics()?;

    if config.record {
        write_baseline(&config.baseline_path, &metrics)?;
        println!("perf_guard baseline recorded at {:?}", config.baseline_path);
        return Ok(());
    }

    let baseline = load_baseline(&config.baseline_path)?;
    compare_metrics(&baseline, &metrics, config.max_ratio)?;

    println!(
        "perf_guard ok: hash {} ms, retention {} ms",
        metrics.hash_ms, metrics.retention_ms
    );
    Ok(())
}
