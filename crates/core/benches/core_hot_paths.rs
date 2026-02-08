use backup_core::backup::versioned;
use backup_core::config::model::{Destination, WatchedKind, WatchedPath};
use backup_core::config::registry::config_defaults;
use backup_core::hashing::sha256_file_hex;
use criterion::{criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion, Throughput};
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use tempfile::TempDir;

/// Purpose: Create a deterministic file payload for hashing benchmarks.
///
/// Inputs: `dir` as the temp directory, `name` as the file name, `bytes` as size.
/// Outputs: The full `PathBuf` to the created file.
/// Ties to: Hashing benchmarks in this module.
/// Side effects: Writes a file to the filesystem.
/// Why: Keep benchmark input consistent across runs.
fn create_payload(dir: &TempDir, name: &str, bytes: usize) -> PathBuf {
    let path = dir.path().join(name);
    let mut file = File::create(&path).expect("bench::create_payload failed to create file");
    let buf = vec![0xAB; bytes];
    file.write_all(&buf)
        .expect("bench::create_payload failed to write file");
    path
}

/// Purpose: Build a deterministic watched tree for versioned simulation benchmarks.
///
/// Inputs: `dir` as the temp directory and `count` as file count.
/// Outputs: Path to the watched directory.
/// Ties to: `versioned::simulate_backup_cycle_with_sample_limit` benchmark input.
/// Side effects: Writes files to the filesystem.
/// Why: Keep benchmark inputs stable while exercising scanning and hashing.
fn create_watched_tree(dir: &TempDir, count: usize) -> PathBuf {
    let watched = dir.path().join("watched");
    fs::create_dir_all(&watched).expect("bench::create_watched_tree failed to create watched dir");
    let buf = vec![0xCD; 4 * 1024];
    for i in 0..count {
        let path = watched.join(format!("f_{i:05}.bin"));
        fs::write(&path, &buf).expect("bench::create_watched_tree failed to write file");
    }
    watched
}

/// Purpose: Benchmark the file hashing hot path.
///
/// Inputs: Criterion harness instance.
/// Outputs: Criterion measurements recorded by the runner.
/// Ties to: `fs::hashing::hash_file` in core.
/// Side effects: Reads from the filesystem repeatedly.
/// Why: Guard the primary content hashing throughput.
fn bench_hashing(c: &mut Criterion) {
    let mut group = c.benchmark_group("hashing");
    let temp = TempDir::new().expect("bench::bench_hashing failed to create temp dir");
    let path = create_payload(&temp, "payload.bin", 16 * 1024 * 1024);
    group.throughput(Throughput::Bytes(16 * 1024 * 1024));
    group.bench_function(BenchmarkId::from_parameter("sha256_file_hex"), |b| {
        b.iter(|| {
            let _ = sha256_file_hex(&path).expect("bench::bench_hashing sha256_file_hex failed");
        });
    });
    group.finish();
}

/// Purpose: Benchmark the versioned simulation hot path.
///
/// Inputs: Criterion harness instance.
/// Outputs: Criterion measurements recorded by the runner.
/// Ties to: `backup::versioned` scanning and hashing logic.
/// Side effects: Reads destination store metadata; hashes watched files.
/// Why: Track the end-to-end scan+diff cost that dominates steady-state cycles.
fn bench_versioned_simulate(c: &mut Criterion) {
    let mut group = c.benchmark_group("versioned");
    group.bench_function("simulate_200_files", |b| {
        b.iter_batched(
            || {
                let dir = TempDir::new().expect("bench::bench_versioned_simulate temp dir");
                let watched = create_watched_tree(&dir, 200);
                let dest = dir.path().join("dest");
                fs::create_dir_all(&dest).expect("bench::bench_versioned_simulate dest dir");
                let mut cfg = config_defaults().expect("bench::bench_versioned_simulate defaults");
                cfg.backup_root = dest.clone();
                cfg.runtime.source_snapshots_enabled = false;
                cfg.destinations = vec![Destination {
                    id: "primary".to_string(),
                    path: dest.clone(),
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
                    .expect("bench::bench_versioned_simulate seed run");
                (dir, cfg)
            },
            |(_dir, cfg)| {
                let _ = versioned::simulate_backup_cycle_with_sample_limit(&cfg, 0)
                    .expect("bench::bench_versioned_simulate simulate failed");
            },
            BatchSize::SmallInput,
        );
    });
    group.finish();
}

/// Purpose: Register core hot-path benchmarks.
///
/// Inputs: Criterion harness instance.
/// Outputs: Criterion measurements recorded by the runner.
/// Ties to: `bench_hashing` and `bench_retention`.
/// Side effects: Delegates to benchmark functions.
/// Why: Keep a single entry point for criterion registration.
fn core_hot_paths(c: &mut Criterion) {
    bench_hashing(c);
    bench_versioned_simulate(c);
}

criterion_group!(benches, core_hot_paths);
criterion_main!(benches);
