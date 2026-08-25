use anyhow::{Context, Result};
use backup_core::backup::versioned;
use backup_core::config::model::{Destination, WatchedKind, WatchedPath};
use backup_core::config::registry::config_defaults;
use backup_core::sha256_file_hex;
use serde::{Deserialize, Serialize};
use std::env;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::time::Instant;
use tempfile::TempDir;

/// Standardize how performance metrics are recorded and compared.
#[derive(Debug, Serialize, Deserialize)]
struct PerfMetrics {
    sha256_small_ms: u128,
    sha256_ms: u128,
    versioned_simulate_ms: u128,
    versioned_simulate_large_ms: u128,
    versioned_incremental_change_ms: u128,
}

/// Keep the perf guard configuration explicit and testable.
#[derive(Debug)]
struct PerfConfig {
    baseline_path: PathBuf,
    max_ratio: f64,
    record: bool,
}

/// Avoid scattered argument parsing logic.
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

/// Keep hashing input consistent across runs.
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

/// Keep the simulation workload stable so regressions are meaningful.
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

/// Avoid duplicate setup logic for versioned workload measurements.
fn build_versioned_fixture(
    temp: &TempDir,
    files: usize,
    bytes_each: usize,
) -> Result<(backup_core::Config, PathBuf)> {
    let watched = create_watched_tree(temp, files, bytes_each)?;
    let destination = temp.path().join("dest");
    fs::create_dir_all(&destination).with_context(|| {
        format!(
            "perf_guard::build_versioned_fixture failed to create destination dir {:?}",
            destination
        )
    })?;
    let mut cfg = config_defaults()
        .context("perf_guard::build_versioned_fixture failed to build defaults")?;
    cfg.backup_root = destination.clone();
    cfg.runtime.source_snapshots_enabled = false;
    cfg.destinations = vec![Destination {
        id: "primary".to_string(),
        path: destination,
        label: Some("Primary".to_string()),
        max_backups_per_file: None,
        replicate_to: vec![],
    }];
    cfg.watched = vec![WatchedPath {
        path: watched.clone(),
        kind: WatchedKind::Directory,
        enabled: true,
        destination_id: "primary".to_string(),
        max_backups_per_file: Some(5),
    }];
    Ok((cfg, watched))
}

/// Track real hot paths for regression detection.
fn measure_metrics() -> Result<PerfMetrics> {
    let temp = TempDir::new().context("perf_guard::measure_metrics failed to create temp dir")?;
    let payload_small = create_payload(&temp, 1024 * 1024)?;
    let sha_small_start = Instant::now();
    for _ in 0..20 {
        sha256_file_hex(&payload_small).with_context(|| {
            format!(
                "perf_guard::measure_metrics sha256_file_hex failed for {:?}",
                payload_small
            )
        })?;
    }
    let sha256_small_ms = sha_small_start.elapsed().as_millis();

    let payload = create_payload(&temp, 8 * 1024 * 1024)?;
    let sha_start = Instant::now();
    for _ in 0..5 {
        sha256_file_hex(&payload).with_context(|| {
            format!(
                "perf_guard::measure_metrics sha256_file_hex failed for {:?}",
                payload
            )
        })?;
    }
    let sha256_ms = sha_start.elapsed().as_millis();

    let (cfg, watched) = build_versioned_fixture(&temp, 200, 4 * 1024)?;
    versioned::run_backup_cycle(&cfg)
        .context("perf_guard::measure_metrics failed initial versioned backup")?;

    let simulate_start = Instant::now();
    for _ in 0..3 {
        versioned::simulate_backup_cycle_with_sample_limit(&cfg, 0)
            .context("perf_guard::measure_metrics simulate failed")?;
    }
    let versioned_simulate_ms = simulate_start.elapsed().as_millis();

    let changed_file = watched.join("f_00042.bin");
    fs::write(&changed_file, vec![0xEF; 4 * 1024]).with_context(|| {
        format!(
            "perf_guard::measure_metrics failed to mutate watched file {:?}",
            changed_file
        )
    })?;
    let incremental_start = Instant::now();
    versioned::run_backup_cycle(&cfg)
        .context("perf_guard::measure_metrics incremental backup cycle failed")?;
    let versioned_incremental_change_ms = incremental_start.elapsed().as_millis();

    let temp_large =
        TempDir::new().context("perf_guard::measure_metrics failed to create large temp dir")?;
    let (cfg_large, _) = build_versioned_fixture(&temp_large, 1000, 4 * 1024)?;
    versioned::run_backup_cycle(&cfg_large)
        .context("perf_guard::measure_metrics failed large initial backup cycle")?;
    let simulate_large_start = Instant::now();
    versioned::simulate_backup_cycle_with_sample_limit(&cfg_large, 0)
        .context("perf_guard::measure_metrics large simulate failed")?;
    let versioned_simulate_large_ms = simulate_large_start.elapsed().as_millis();

    Ok(PerfMetrics {
        sha256_small_ms,
        sha256_ms,
        versioned_simulate_ms,
        versioned_simulate_large_ms,
        versioned_incremental_change_ms,
    })
}

/// Compare current performance to a committed baseline.
fn load_baseline(path: &PathBuf) -> Result<PerfMetrics> {
    let mut file = File::open(path)
        .with_context(|| format!("perf_guard::load_baseline failed to open {:?}", path))?;
    let mut buf = String::new();
    file.read_to_string(&mut buf)
        .with_context(|| format!("perf_guard::load_baseline failed to read {:?}", path))?;
    serde_json::from_str(&buf)
        .with_context(|| format!("perf_guard::load_baseline failed to parse {:?}", path))
}

/// Persist a reproducible baseline for future comparisons.
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

/// Fail fast on performance regressions.
fn compare_metrics(baseline: &PerfMetrics, current: &PerfMetrics, max_ratio: f64) -> Result<()> {
    let ratio = |current_ms: u128, baseline_ms: u128| -> f64 {
        let current = current_ms.max(1) as f64;
        let baseline = baseline_ms.max(1) as f64;
        current / baseline
    };

    let sha_small_ratio = ratio(current.sha256_small_ms, baseline.sha256_small_ms);
    let sha_ratio = ratio(current.sha256_ms, baseline.sha256_ms);
    let simulate_ratio = ratio(
        current.versioned_simulate_ms,
        baseline.versioned_simulate_ms,
    );
    let simulate_large_ratio = ratio(
        current.versioned_simulate_large_ms,
        baseline.versioned_simulate_large_ms,
    );
    let incremental_ratio = ratio(
        current.versioned_incremental_change_ms,
        baseline.versioned_incremental_change_ms,
    );

    if sha_small_ratio > max_ratio {
        anyhow::bail!(
            "perf_guard::compare_metrics sha256_small regression {:.2}x > {:.2}x",
            sha_small_ratio,
            max_ratio
        );
    }
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
    if simulate_large_ratio > max_ratio {
        anyhow::bail!(
            "perf_guard::compare_metrics versioned_simulate_large regression {:.2}x > {:.2}x",
            simulate_large_ratio,
            max_ratio
        );
    }
    if incremental_ratio > max_ratio {
        anyhow::bail!(
            "perf_guard::compare_metrics versioned_incremental_change regression {:.2}x > {:.2}x",
            incremental_ratio,
            max_ratio
        );
    }
    Ok(())
}

/// Provide a reproducible performance gate for hot paths.
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
        "perf_guard ok: sha256_small {} ms, sha256_large {} ms, versioned_simulate_small {} ms, versioned_simulate_large {} ms, versioned_incremental_change {} ms",
        metrics.sha256_small_ms,
        metrics.sha256_ms,
        metrics.versioned_simulate_ms,
        metrics.versioned_simulate_large_ms,
        metrics.versioned_incremental_change_ms
    );
    Ok(())
}
