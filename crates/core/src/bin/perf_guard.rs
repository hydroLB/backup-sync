use anyhow::{Context, Result};
use backup_core::backup::versioned;
use backup_core::config::model::{Destination, WatchedKind, WatchedPath};
use backup_core::config::registry::config_defaults;
use backup_core::hashing::sha256_file_hex;
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
    sha256_ms: u128,
    versioned_simulate_ms: u128,
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

/// Purpose: Create a deterministic watched tree for versioned simulation tests.
///
/// Inputs: `dir` as the temp directory root, `files` count, and `bytes_each` size.
/// Outputs: The watched directory path created under `dir`.
/// Ties to: Versioned scan and hashing measurement in `measure_metrics`.
/// Side effects: Writes a set of files to disk.
/// Why: Keep the simulation workload stable so regressions are meaningful.
fn create_watched_tree(dir: &TempDir, files: usize, bytes_each: usize) -> Result<PathBuf> {
    let watched = dir.path().join("watched");
    fs::create_dir_all(&watched).with_context(|| {
        format!(
            "perf_guard::create_watched_tree failed to create watched dir {:?}",
            watched
        )
    })?;
    let payload = vec![0xAB; bytes_each];
    for i in 0..files {
        let path = watched.join(format!("f_{i:05}.bin"));
        fs::write(&path, &payload).with_context(|| {
            format!(
                "perf_guard::create_watched_tree failed to write file {:?}",
                path
            )
        })?;
    }
    Ok(watched)
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
    let sha_start = Instant::now();
    for _ in 0..5 {
        let _ = sha256_file_hex(&payload).with_context(|| {
            format!(
                "perf_guard::measure_metrics sha256_file_hex failed for {:?}",
                payload
            )
        })?;
    }
    let sha256_ms = sha_start.elapsed().as_millis();

    let watched = create_watched_tree(&temp, 200, 4 * 1024)?;
    let destination = temp.path().join("dest");
    fs::create_dir_all(&destination).with_context(|| {
        format!(
            "perf_guard::measure_metrics failed to create destination dir {:?}",
            destination
        )
    })?;
    let mut cfg =
        config_defaults().context("perf_guard::measure_metrics failed to build defaults")?;
    cfg.backup_root = destination.clone();
    cfg.runtime.source_snapshots_enabled = false;
    cfg.destinations = vec![Destination {
        id: "primary".to_string(),
        path: destination.clone(),
        label: Some("Primary".to_string()),
        max_backups_per_file: None,
        replicate_to: vec![],
    }];
    cfg.watched = vec![WatchedPath {
        path: watched,
        kind: WatchedKind::Directory,
        enabled: true,
        destination_id: "primary".to_string(),
        max_backups_per_file: Some(5),
    }];
    versioned::run_backup_cycle(&cfg)
        .context("perf_guard::measure_metrics failed initial versioned backup")?;

    let simulate_start = Instant::now();
    for _ in 0..3 {
        let _ = versioned::simulate_backup_cycle_with_sample_limit(&cfg, 0)
            .context("perf_guard::measure_metrics simulate failed")?;
    }
    let versioned_simulate_ms = simulate_start.elapsed().as_millis();

    Ok(PerfMetrics {
        sha256_ms,
        versioned_simulate_ms,
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
    let sha_ratio = current.sha256_ms as f64 / baseline.sha256_ms as f64;
    let simulate_ratio =
        current.versioned_simulate_ms as f64 / baseline.versioned_simulate_ms as f64;

    if sha_ratio > max_ratio {
        anyhow::bail!(
            "perf_guard::compare_metrics sha256 regression {:.2}x > {:.2}x",
            sha_ratio,
            max_ratio
        );
    }
    if simulate_ratio > max_ratio {
        anyhow::bail!(
            "perf_guard::compare_metrics versioned_simulate regression {:.2}x > {:.2}x",
            simulate_ratio,
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
        "perf_guard ok: sha256 {} ms, versioned_simulate {} ms",
        metrics.sha256_ms, metrics.versioned_simulate_ms
    );
    Ok(())
}
