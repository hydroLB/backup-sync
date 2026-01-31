use backup_core::backup::retention;
use backup_core::fs::hashing::hash_file;
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

/// Purpose: Build a temp directory with retention-style filenames.
///
/// Inputs: `count` as total files, `dir` as the temp directory.
/// Outputs: A vector of file paths in the directory.
/// Ties to: `retention::enforce` benchmark inputs.
/// Side effects: Writes files to the filesystem.
/// Why: Model realistic retention workloads with sortable timestamps.
fn create_retention_files(dir: &TempDir, count: usize) -> Vec<PathBuf> {
    let mut paths = Vec::with_capacity(count);
    for i in 0..count {
        let name = format!("20240101-{:06}__file.txt", i);
        let path = dir.path().join(name);
        File::create(&path).expect("bench::create_retention_files failed to create file");
        paths.push(path);
    }
    paths
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
    group.bench_function(BenchmarkId::from_parameter("hash_file"), |b| {
        b.iter(|| {
            let _ = hash_file(&path).expect("bench::bench_hashing hash_file failed");
        });
    });
    group.finish();
}

/// Purpose: Benchmark retention pruning behavior.
///
/// Inputs: Criterion harness instance.
/// Outputs: Criterion measurements recorded by the runner.
/// Ties to: `backup::retention::enforce` pruning logic.
/// Side effects: Creates and deletes files in a temp directory.
/// Why: Track retention performance with realistic file counts.
fn bench_retention(c: &mut Criterion) {
    let mut group = c.benchmark_group("retention");
    group.bench_function("enforce_200_keep_100", |b| {
        b.iter_batched(
            || {
                let dir = TempDir::new().expect("bench::bench_retention failed to create temp dir");
                let files = create_retention_files(&dir, 200);
                (dir, files)
            },
            |(dir, files)| {
                let _ = retention::enforce(100, files)
                    .expect("bench::bench_retention retention::enforce failed");
                fs::read_dir(dir.path())
                    .expect("bench::bench_retention failed to list retention dir");
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
    bench_retention(c);
}

criterion_group!(benches, core_hot_paths);
criterion_main!(benches);
