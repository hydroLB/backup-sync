use filetime::{set_file_mtime, FileTime};
use std::fs;
use std::path::Path;

/// Summary: Writes deterministic fixture content and forces a monotonic mtime revision.
///
/// Inputs: target file path, monotonic revision number, and file contents.
///
/// Outputs: none.
///
/// Side effects: Writes fixture bytes and updates file mtime metadata on disk.
///
/// Error handling: Panics with contextual method/file messaging when fixture writes fail.
///
/// Ties to other methods: integration tests that rely on change detection across file revisions.
///
/// Why this exists: remove timing sleeps from tests while keeping deterministic file-change detection.
pub fn write_fixture_revision(path: &Path, revision: i64, contents: &str) {
    fs::write(path, contents).unwrap_or_else(|error| {
        panic!("core::tests::support::write_fixture_revision write failed: {error}")
    });
    let mtime = FileTime::from_unix_time(1_700_000_000 + revision, 0);
    set_file_mtime(path, mtime).unwrap_or_else(|error| {
        panic!("core::tests::support::write_fixture_revision set_file_mtime failed: {error}")
    });
}
