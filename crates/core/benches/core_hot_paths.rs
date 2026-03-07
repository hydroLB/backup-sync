use backup_core::backup::versioned;
use backup_core::config::model::{Destination, WatchedKind, WatchedPath};
use backup_core::config::registry::config_defaults;
use backup_core::sha256_file_hex;
use criterion::{criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion, Throughput};
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use tempfile::TempDir;

/// Summary: Create a deterministic file payload for hashing benchmarks.
///
/// Inputs: `dir` as the temp directory, `name` as the file name, `bytes` as size.
///
/// Outputs: The full `PathBuf` to the created file.
///
/// Side effects: Writes a file to the filesystem.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: Hashing benchmarks in this module.
///
/// Why this exists: Keep benchmark input consistent across runs.
fn create_payload(dir: &TempDir, name: &str, bytes: usize) -> PathBuf {
    let path = dir.path().join(name);
    let mut file = File::create(&path).expect("bench::create_payload failed to create file");
    let buf = vec![0xAB; bytes];
    file.write_all(&buf)
        .expect("bench::create_payload failed to write file");
    path
}

/// Summary: Build a deterministic watched tree for versioned simulation benchmarks.
///
/// Inputs: `dir` as the temp directory and `count` as file count.
///
/// Outputs: Path to the watched directory.
///
/// Side effects: Writes files to the filesystem.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: `versioned::simulate_backup_cycle_with_sample_limit` benchmark input.
///
/// Why this exists: Keep benchmark inputs stable while exercising scanning and hashing.
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

/// Summary: Builds deterministic versioned benchmark fixture state.
///
/// Inputs: file count for the watched tree fixture.
///
/// Outputs: temp directory handle and configured backup config seeded with one backup cycle.
///
/// Side effects: creates temporary directories and writes fixture files to disk.
///
/// Error handling: Panics with contextual messages when fixture setup fails.
///
/// Ties to other methods: `bench_versioned_simulate` benchmark variants.
///
/// Why this exists: avoid duplicated setup logic across versioned benchmark cases.
fn build_versioned_fixture(file_count: usize) -> (TempDir, backup_core::Config) {
    let dir = TempDir::new().expect("bench::build_versioned_fixture temp dir");
    let watched = create_watched_tree(&dir, file_count);
    let dest = dir.path().join("dest");
    fs::create_dir_all(&dest).expect("bench::build_versioned_fixture dest dir");
    let mut cfg = config_defaults().expect("bench::build_versioned_fixture defaults");
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
    versioned::run_backup_cycle(&cfg).expect("bench::build_versioned_fixture seed run");
    (dir, cfg)
}

/// Summary: Benchmark the file hashing hot path.
///
/// Inputs: Criterion harness instance.
///
/// Outputs: Criterion measurements recorded by the runner.
///
/// Side effects: Reads from the filesystem repeatedly.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: `fs::hashing::hash_file` in core.
///
/// Why this exists: Guard the primary content hashing throughput.
fn bench_hashing(c: &mut Criterion) {
    let mut group = c.benchmark_group("hashing");
    let temp = TempDir::new().expect("bench::bench_hashing failed to create temp dir");
    let cases = [
        ("1_mib", 1024 * 1024usize),
        ("16_mib", 16 * 1024 * 1024usize),
        ("64_mib", 64 * 1024 * 1024usize),
    ];
    for (label, bytes) in cases {
        let path = create_payload(&temp, &format!("payload_{label}.bin"), bytes);
        group.throughput(Throughput::Bytes(bytes as u64));
        group.bench_function(BenchmarkId::new("sha256_file_hex", label), |b| {
            b.iter(|| {
                sha256_file_hex(&path).expect("bench::bench_hashing sha256_file_hex failed");
            });
        });
    }
    group.finish();
}

/// Summary: Benchmark the versioned simulation hot path.
///
/// Inputs: Criterion harness instance.
///
/// Outputs: Criterion measurements recorded by the runner.
///
/// Side effects: Reads destination store metadata; hashes watched files.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: `backup::versioned` scanning and hashing logic.
///
/// Why this exists: Track the end-to-end scan+diff cost that dominates steady-state cycles.
fn bench_versioned_simulate(c: &mut Criterion) {
    let mut group = c.benchmark_group("versioned");
    let cases = [
        ("simulate_200_files", 200usize),
        ("simulate_1000_files", 1000usize),
    ];
    for (label, file_count) in cases {
        group.bench_function(label, |b| {
            b.iter_batched(
                || build_versioned_fixture(file_count),
                |(_dir, cfg)| {
                    versioned::simulate_backup_cycle_with_sample_limit(&cfg, 0)
                        .expect("bench::bench_versioned_simulate simulate failed");
                },
                BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

/// Summary: Register core hot-path benchmarks.
///
/// Inputs: Criterion harness instance.
///
/// Outputs: Criterion measurements recorded by the runner.
///
/// Side effects: Delegates to benchmark functions.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: `bench_hashing` and `bench_versioned_simulate`.
///
/// Why this exists: Keep a single entry point for criterion registration.
fn core_hot_paths(c: &mut Criterion) {
    bench_hashing(c);
    bench_versioned_simulate(c);
}

criterion_group!(benches, core_hot_paths);
criterion_main!(benches);
