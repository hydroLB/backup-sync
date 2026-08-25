use filetime::{set_file_mtime, FileTime};
use std::fs;
use std::path::Path;

/// Remove timing sleeps from tests while keeping deterministic file-change detection.
pub fn write_fixture_revision(path: &Path, revision: i64, contents: &str) {
    fs::write(path, contents).unwrap_or_else(|error| {
        panic!("core::tests::support::write_fixture_revision write failed: {error}")
    });
    let mtime = FileTime::from_unix_time(1_700_000_000 + revision, 0);
    set_file_mtime(path, mtime).unwrap_or_else(|error| {
        panic!("core::tests::support::write_fixture_revision set_file_mtime failed: {error}")
    });
}
