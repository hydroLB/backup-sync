from __future__ import annotations

import json
import os
import platform
import shutil
import uuid
from datetime import UTC, datetime
from hashlib import sha256
from pathlib import Path
from typing import Any

GRID_ROOT = Path.home() / ".codex-desktop-grid"
SESSIONS_ROOT = GRID_ROOT / "sessions"
JOBS_ROOT = GRID_ROOT / "jobs"
LOGS_ROOT = GRID_ROOT / "logs"
LEGACY_INSTALL_ROOT = Path.home() / ".codex-desktop-bridge"

DEFAULT_WORKER_CAPACITY = {
    "browser-worker": 12,
    "macos-gui-worker": 2,
    "linux-gui-worker": 6,
}


class GridError(RuntimeError):
    """
    Summary
    Provide a grid-specific exception type for orchestrator and worker failures.

    Inputs
    Standard exception message text.

    Outputs
    An exception instance carrying the contextual failure message.

    Side effects
    Captures a traceback when raised.

    Error handling
    Used as the canonical typed failure for grid services.

    Ties to other methods
    Shared by session, OCR, and orchestration modules.

    Why this exists
    Grid services need one stable error type for clear HTTP and CLI error reporting.
    """


def ensure_directory(path: Path) -> Path:
    """
    Summary
    Create a directory tree when it does not already exist.

    Inputs
    `path` as the directory to create.

    Outputs
    The same `Path` after ensuring it exists.

    Side effects
    Creates directories on disk.

    Error handling
    Raises `GridError` when the directory cannot be created.

    Ties to other methods
    Used by session roots, job roots, log directories, and artifact writers.

    Why this exists
    The grid stores per-session and per-job artifacts in predictable directories that must exist before use.
    """

    try:
        path.mkdir(parents=True, exist_ok=True)
        return path
    except OSError as exc:
        raise GridError(f"[grid_common.py::ensure_directory] Failed to create {path}: {exc}") from exc


def utc_timestamp() -> str:
    """
    Summary
    Build a UTC timestamp string for artifacts and metadata.

    Inputs
    None.

    Outputs
    An ISO-8601 UTC timestamp string.

    Side effects
    Reads the system clock.

    Error handling
    None.

    Ties to other methods
    Used by session metadata, job metadata, and artifact summaries.

    Why this exists
    Artifact timelines need a consistent timezone-neutral timestamp format.
    """

    return datetime.now(UTC).isoformat()


def make_id(prefix: str) -> str:
    """
    Summary
    Generate a short stable identifier with the requested prefix.

    Inputs
    `prefix` as the identifier namespace.

    Outputs
    A unique identifier string.

    Side effects
    Reads randomness from the OS UUID implementation.

    Error handling
    None.

    Ties to other methods
    Used for session ids, job ids, and frame ids.

    Why this exists
    Grid resources need human-readable ids that are still unique across concurrent runs.
    """

    return f"{prefix}-{uuid.uuid4().hex[:12]}"


def sha256_file(path: Path) -> str:
    """
    Summary
    Compute the SHA-256 digest of a file.

    Inputs
    `path` as the file to hash.

    Outputs
    A hexadecimal SHA-256 digest.

    Side effects
    Reads file bytes from disk.

    Error handling
    Raises `GridError` when the file cannot be read.

    Ties to other methods
    Used for frame ids and OCR cache keys.

    Why this exists
    OCR and visual artifacts should be cacheable by exact frame content instead of timestamps alone.
    """

    try:
        digest = sha256()
        with path.open("rb") as handle:
            for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                digest.update(chunk)
        return digest.hexdigest()
    except OSError as exc:
        raise GridError(f"[grid_common.py::sha256_file] Failed to hash {path}: {exc}") from exc


def write_json(path: Path, payload: Any) -> None:
    """
    Summary
    Serialize a JSON payload to disk.

    Inputs
    `path` as the target file and `payload` as the JSON-serializable value.

    Outputs
    None.

    Side effects
    Writes a UTF-8 JSON file to disk.

    Error handling
    Raises `GridError` when serialization or writing fails.

    Ties to other methods
    Used by OCR cache entries, session summaries, and job summaries.

    Why this exists
    Grid artifacts should be easy to inspect and reuse from deterministic JSON files.
    """

    try:
        ensure_directory(path.parent)
        path.write_text(json.dumps(payload, indent=2, sort_keys=True), encoding="utf-8")
    except (OSError, TypeError, ValueError) as exc:
        raise GridError(f"[grid_common.py::write_json] Failed to write {path}: {exc}") from exc


def append_jsonl(path: Path, payload: Any) -> None:
    """
    Summary
    Append a JSON line record to a log file.

    Inputs
    `path` as the JSONL file and `payload` as the JSON-serializable record.

    Outputs
    None.

    Side effects
    Creates the parent directory and appends a line to disk.

    Error handling
    Raises `GridError` when serialization or writing fails.

    Ties to other methods
    Used by session action logs and orchestrator job event logs.

    Why this exists
    Action histories should stay append-only and stream-friendly for later debugging.
    """

    try:
        ensure_directory(path.parent)
        with path.open("a", encoding="utf-8") as handle:
            handle.write(json.dumps(payload, sort_keys=True))
            handle.write("\n")
    except (OSError, TypeError, ValueError) as exc:
        raise GridError(f"[grid_common.py::append_jsonl] Failed to append {path}: {exc}") from exc


def remove_tree(path: Path) -> None:
    """
    Summary
    Remove a directory tree when it exists.

    Inputs
    `path` as the directory tree to remove.

    Outputs
    None.

    Side effects
    Deletes files and directories from disk.

    Error handling
    Raises `GridError` when removal fails.

    Ties to other methods
    Used during session and job cleanup.

    Why this exists
    Temporary execution state must be cleaned without touching retained artifacts.
    """

    try:
        if path.exists():
            shutil.rmtree(path)
    except OSError as exc:
        raise GridError(f"[grid_common.py::remove_tree] Failed to remove {path}: {exc}") from exc


def host_worker_capabilities() -> dict[str, Any]:
    """
    Summary
    Describe the worker classes supported by the current host.

    Inputs
    None.

    Outputs
    A JSON-serializable capability dictionary with capacities and availability flags.

    Side effects
    Reads platform state and executable availability from the environment.

    Error handling
    None.

    Ties to other methods
    Used by the worker daemon health endpoint and orchestrator worker registration.

    Why this exists
    Scheduling must know which worker classes are actually runnable on the current machine.
    """

    system_name = platform.system().lower()
    xvfb_available = shutil.which("Xvfb") is not None
    return {
        "host_platform": system_name,
        "worker_capacity": DEFAULT_WORKER_CAPACITY,
        "supported": {
            "browser-worker": True,
            "macos-gui-worker": system_name == "darwin",
            "linux-gui-worker": xvfb_available and system_name == "linux",
        },
        "paths": {
            "grid_root": str(GRID_ROOT),
            "sessions_root": str(SESSIONS_ROOT),
            "jobs_root": str(JOBS_ROOT),
            "logs_root": str(LOGS_ROOT),
            "legacy_install_root": str(LEGACY_INSTALL_ROOT),
        },
    }


def png_size(path: Path) -> tuple[int, int]:
    """
    Summary
    Read the pixel dimensions from a PNG file without external image libraries.

    Inputs
    `path` as the PNG image to inspect.

    Outputs
    A tuple of `(width, height)` in pixels.

    Side effects
    Reads file bytes from disk.

    Error handling
    Raises `GridError` when the file is missing or not a valid PNG.

    Ties to other methods
    Used for translating OCR pixel coordinates into window-relative click coordinates.

    Why this exists
    The grid should not depend on Pillow just to read screenshot dimensions.
    """

    try:
        with path.open("rb") as handle:
            header = handle.read(24)
    except OSError as exc:
        raise GridError(f"[grid_common.py::png_size] Failed to read {path}: {exc}") from exc
    if len(header) < 24 or header[:8] != b"\x89PNG\r\n\x1a\n":
        raise GridError(f"[grid_common.py::png_size] File is not a valid PNG: {path}")
    width = int.from_bytes(header[16:20], "big")
    height = int.from_bytes(header[20:24], "big")
    return width, height


def shell_env_with(base: dict[str, str] | None, extra: dict[str, str]) -> dict[str, str]:
    """
    Summary
    Merge environment variables for subprocess execution.

    Inputs
    `base` as the starting environment and `extra` as required overrides.

    Outputs
    A merged environment mapping.

    Side effects
    Reads the current process environment when `base` is omitted.

    Error handling
    None.

    Ties to other methods
    Used by orchestrator job execution to inject artifact and worker metadata.

    Why this exists
    Job runners need deterministic environment injection without mutating the global process environment.
    """

    env = dict(base or os.environ)
    env.update(extra)
    return env
