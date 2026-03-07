from __future__ import annotations

import threading
import time
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

from desktopctl import screenshot_command as desktop_screenshot_command
from desktopctl import _resolve_window
from grid_common import (
    DEFAULT_WORKER_CAPACITY,
    GridError,
    SESSIONS_ROOT,
    append_jsonl,
    ensure_directory,
    host_worker_capabilities,
    make_id,
    png_size,
    remove_tree,
    sha256_file,
    utc_timestamp,
)


def _namespace(**kwargs: Any) -> Any:
    """
    Summary
    Build a light namespace object compatible with existing `desktopctl` handlers.

    Inputs
    Arbitrary keyword arguments representing command parameters.

    Outputs
    An object whose attributes mirror the provided keyword arguments.

    Side effects
    None.

    Error handling
    None.

    Ties to other methods
    Used to call the existing `desktopctl` screenshot handler without rebuilding its parser.

    Why this exists
    Session capture code should reuse the proven local command handlers directly.
    """

    return type("SessionNamespace", (), kwargs)()


@dataclass
class SessionFrameState:
    """
    Summary
    Store the latest capture metadata for a worker session.

    Inputs
    Dataclass fields populated by session capture logic.

    Outputs
    Mutable frame metadata for callers and HTTP responses.

    Side effects
    None.

    Error handling
    Capture failures are recorded in `last_error` instead of throwing from the dataclass.

    Ties to other methods
    Used by `SessionCaptureLoop`, coordinate translation, and HTTP serialization.

    Why this exists
    Session-scoped automation needs deterministic frame and window metadata.
    """

    frame_path: Path
    app_frame_path: Path
    frame_hash: str | None = None
    app_frame_hash: str | None = None
    image_size: dict[str, int] | None = None
    app_image_size: dict[str, int] | None = None
    window_bounds: dict[str, int] | None = None
    last_capture_ts: float = 0.0
    last_error: str | None = None


class SessionCaptureLoop:
    """
    Summary
    Maintain per-session full-screen and app-targeted captures.

    Inputs
    Session root paths, target app, and capture interval.

    Outputs
    Mutable frame state updated by the background capture loop.

    Side effects
    Captures screenshots to disk and runs a background thread when enabled.

    Error handling
    Records contextual capture failures in the frame state instead of crashing the worker service.

    Ties to other methods
    Used by GUI sessions managed by `SessionRegistry`.

    Why this exists
    Each GUI session needs isolated frame files and metadata instead of one global host capture.
    """

    def __init__(self, session_root: Path, interval_seconds: float, target_app: str | None) -> None:
        ensure_directory(session_root)
        captures_root = ensure_directory(session_root / "captures")
        self.interval_seconds = interval_seconds
        self.target_app = target_app
        self.state = SessionFrameState(
            frame_path=captures_root / "screen.png",
            app_frame_path=captures_root / "app.png",
        )
        self._lock = threading.Lock()
        self._stop = threading.Event()
        self._thread = threading.Thread(target=self._loop, daemon=True)

    def start(self) -> None:
        """
        Summary
        Start the background capture loop and perform the initial refresh.

        Inputs
        None.

        Outputs
        None.

        Side effects
        Starts a daemon thread and writes initial frame files.

        Error handling
        Capture errors are stored in frame metadata.

        Ties to other methods
        Called when a GUI session is created.

        Why this exists
        Session frames should be immediately available to automation callers.
        """

        self.refresh()
        self._thread.start()

    def stop(self) -> None:
        """
        Summary
        Stop the background capture loop.

        Inputs
        None.

        Outputs
        None.

        Side effects
        Signals and joins the background capture thread.

        Error handling
        None.

        Ties to other methods
        Called when a session is deleted.

        Why this exists
        Session cleanup must not leak background worker threads.
        """

        self._stop.set()
        self._thread.join(timeout=2)

    def set_target_app(self, app_name: str | None) -> None:
        """
        Summary
        Update the app target for session-relative captures.

        Inputs
        `app_name` as the target app or `None` to disable app capture.

        Outputs
        None.

        Side effects
        Mutates capture state and performs an immediate refresh.

        Error handling
        Capture failures are stored in frame metadata.

        Ties to other methods
        Used by session `/target` updates.

        Why this exists
        App-relative frames are what session-relative OCR and clicks should be based on.
        """

        with self._lock:
            self.target_app = app_name
        self.refresh()

    def snapshot(self) -> dict[str, Any]:
        """
        Summary
        Return a JSON-safe copy of the latest frame metadata.

        Inputs
        None.

        Outputs
        A dictionary containing frame paths, hashes, sizes, timestamps, and bounds.

        Side effects
        None.

        Error handling
        None.

        Ties to other methods
        Used by session metadata and HTTP responses.

        Why this exists
        Callers need current frame information without mutating the capture state.
        """

        with self._lock:
            return {
                "frame_path": str(self.state.frame_path),
                "app_frame_path": str(self.state.app_frame_path),
                "frame_hash": self.state.frame_hash,
                "app_frame_hash": self.state.app_frame_hash,
                "image_size": self.state.image_size,
                "app_image_size": self.state.app_image_size,
                "window_bounds": self.state.window_bounds,
                "target_app": self.target_app,
                "last_capture_ts": self.state.last_capture_ts,
                "last_error": self.state.last_error,
                "interval_seconds": self.interval_seconds,
            }

    def refresh(self) -> None:
        """
        Summary
        Capture fresh full-screen and app-scoped screenshots for the session.

        Inputs
        None.

        Outputs
        None.

        Side effects
        Writes screenshot images to the session capture directory.

        Error handling
        Stores contextual errors in frame metadata.

        Ties to other methods
        Used by the background loop and explicit refresh requests.

        Why this exists
        Visual automation needs current frame files and current window bounds.
        """

        try:
            desktop_screenshot_command(_namespace(app=None, title=None, output=str(self.state.frame_path)))
            frame_hash = sha256_file(self.state.frame_path)
            frame_size = png_size(self.state.frame_path)
            app_hash = None
            app_size = None
            window_bounds = None
            with self._lock:
                target_app = self.target_app
            if target_app:
                desktop_screenshot_command(
                    _namespace(app=target_app, title=None, output=str(self.state.app_frame_path))
                )
                app_hash = sha256_file(self.state.app_frame_path)
                app_size = png_size(self.state.app_frame_path)
                window = _resolve_window(target_app, None)
                window_bounds = {
                    "x": int(window["x"]),
                    "y": int(window["y"]),
                    "width": int(window["width"]),
                    "height": int(window["height"]),
                }
            with self._lock:
                self.state.frame_hash = frame_hash
                self.state.app_frame_hash = app_hash
                self.state.image_size = {"width": frame_size[0], "height": frame_size[1]}
                self.state.app_image_size = (
                    {"width": app_size[0], "height": app_size[1]} if app_size else None
                )
                self.state.window_bounds = window_bounds
                self.state.last_capture_ts = time.time()
                self.state.last_error = None
        except Exception as exc:  # noqa: BLE001
            with self._lock:
                self.state.last_error = f"[grid_sessions.py::SessionCaptureLoop.refresh] {exc}"

    def _loop(self) -> None:
        """
        Summary
        Run the background capture refresh loop.

        Inputs
        None.

        Outputs
        None.

        Side effects
        Periodically writes updated session screenshots to disk.

        Error handling
        Refresh errors are recorded by `refresh`.

        Ties to other methods
        Private worker used by `start` and `stop`.

        Why this exists
        Sessions should keep fresh frames available without requiring explicit polling for every action.
        """

        while not self._stop.wait(self.interval_seconds):
            self.refresh()


@dataclass
class SessionRecord:
    """
    Summary
    Represent one session lease managed by the host worker daemon.

    Inputs
    Dataclass fields describing the session lifecycle and artifact locations.

    Outputs
    Mutable session metadata.

    Side effects
    None directly; associated capture loops and files are managed externally.

    Error handling
    Runtime errors are stored in `last_error`.

    Ties to other methods
    Used throughout `SessionRegistry`, worker HTTP responses, and orchestrator integration.

    Why this exists
    Each session needs stable metadata that survives across HTTP requests and scheduling decisions.
    """

    session_id: str
    worker_class: str
    worker_id: str
    display_id: str
    status: str
    visual_mode: str
    session_root: Path
    artifact_root: Path
    temp_root: Path
    created_at: str
    target_app: str | None = None
    frame_store: SessionCaptureLoop | None = None
    metadata: dict[str, Any] = field(default_factory=dict)
    last_error: str | None = None

    def to_dict(self) -> dict[str, Any]:
        """
        Summary
        Convert the session record into a JSON-safe dictionary.

        Inputs
        None.

        Outputs
        A JSON-serializable session metadata dictionary.

        Side effects
        Reads frame store state when present.

        Error handling
        None.

        Ties to other methods
        Used by the worker daemon session endpoints and orchestrator records.

        Why this exists
        Session metadata needs a stable wire format.
        """

        frame_snapshot = self.frame_store.snapshot() if self.frame_store else None
        return {
            "session_id": self.session_id,
            "worker_id": self.worker_id,
            "worker_class": self.worker_class,
            "display_id": self.display_id,
            "status": self.status,
            "visual_mode": self.visual_mode,
            "target_app": self.target_app,
            "frame_path": frame_snapshot["app_frame_path"] if frame_snapshot and self.target_app else (frame_snapshot["frame_path"] if frame_snapshot else None),
            "frame_hash": frame_snapshot["app_frame_hash"] if frame_snapshot and self.target_app else (frame_snapshot["frame_hash"] if frame_snapshot else None),
            "window_bounds": frame_snapshot["window_bounds"] if frame_snapshot else None,
            "session_root": str(self.session_root),
            "artifact_root": str(self.artifact_root),
            "temp_root": str(self.temp_root),
            "created_at": self.created_at,
            "last_error": self.last_error or (frame_snapshot["last_error"] if frame_snapshot else None),
            "metadata": self.metadata,
            "frame_state": frame_snapshot,
        }


class SessionRegistry:
    """
    Summary
    Manage worker sessions, capacities, artifacts, and cleanup.

    Inputs
    Capture interval, optional worker capacities, and capture enablement flag.

    Outputs
    A mutable registry that can create, inspect, and destroy session leases.

    Side effects
    Creates per-session directories and background capture loops for GUI sessions.

    Error handling
    Raises `GridError` for invalid worker classes, capacity exhaustion, and missing sessions.

    Ties to other methods
    Used by the host worker daemon and orchestrator integration tests.

    Why this exists
    Concurrent automation requires explicit session isolation and capacity enforcement.
    """

    def __init__(
        self,
        interval_seconds: float,
        worker_capacity: dict[str, int] | None = None,
        capture_enabled: bool = True,
    ) -> None:
        self.interval_seconds = interval_seconds
        self.worker_capacity = dict(DEFAULT_WORKER_CAPACITY)
        if worker_capacity:
            self.worker_capacity.update(worker_capacity)
        self.capture_enabled = capture_enabled
        self.capabilities = host_worker_capabilities()
        self._sessions: dict[str, SessionRecord] = {}
        self._lock = threading.Lock()
        ensure_directory(SESSIONS_ROOT)

    def create_session(
        self,
        worker_class: str,
        visual_mode: str,
        target_app: str | None = None,
        metadata: dict[str, Any] | None = None,
    ) -> SessionRecord:
        """
        Summary
        Create a new isolated session lease.

        Inputs
        Worker class, visual mode, optional target app, and optional metadata.

        Outputs
        The created `SessionRecord`.

        Side effects
        Creates per-session directories and starts a capture loop for supported GUI workers.

        Error handling
        Raises `GridError` for unsupported worker classes or exhausted capacity.

        Ties to other methods
        Used by the worker daemon `/sessions` endpoint and orchestrator job dispatch.

        Why this exists
        Each concurrent test run needs isolated frame state, artifacts, and capacity accounting.
        """

        supported = self.capabilities["supported"]
        if worker_class not in self.worker_capacity:
            raise GridError(
                f"[grid_sessions.py::SessionRegistry.create_session] Unknown worker class: {worker_class}"
            )
        if worker_class != "browser-worker" and not supported.get(worker_class, False):
            raise GridError(
                f"[grid_sessions.py::SessionRegistry.create_session] Worker class {worker_class} is not supported on this host."
            )

        with self._lock:
            active = sum(1 for session in self._sessions.values() if session.worker_class == worker_class and session.status in {"created", "running", "leased"})
            capacity = self.worker_capacity[worker_class]
            if active >= capacity:
                raise GridError(
                    f"[grid_sessions.py::SessionRegistry.create_session] Capacity exhausted for {worker_class}: {active}/{capacity}."
                )
            session_id = make_id("session")
            session_root = ensure_directory(SESSIONS_ROOT / session_id)
            artifact_root = ensure_directory(session_root / "artifacts")
            temp_root = ensure_directory(session_root / "temp")
            display_id = {
                "browser-worker": "headless-browser",
                "macos-gui-worker": "screen-0",
                "linux-gui-worker": "xvfb-auto",
            }[worker_class]
            frame_store = None
            if self.capture_enabled and worker_class in {"macos-gui-worker", "linux-gui-worker"}:
                frame_store = SessionCaptureLoop(session_root, self.interval_seconds, target_app)
                frame_store.start()
            record = SessionRecord(
                session_id=session_id,
                worker_class=worker_class,
                worker_id="local-host",
                display_id=display_id,
                status="created",
                visual_mode=visual_mode,
                session_root=session_root,
                artifact_root=artifact_root,
                temp_root=temp_root,
                created_at=utc_timestamp(),
                target_app=target_app,
                frame_store=frame_store,
                metadata=dict(metadata or {}),
            )
            self._sessions[session_id] = record
        self._log_event(record, "session_created", {"target_app": target_app})
        return record

    def list_sessions(self) -> list[dict[str, Any]]:
        """
        Summary
        List the current session leases.

        Inputs
        None.

        Outputs
        A list of session metadata dictionaries.

        Side effects
        None.

        Error handling
        None.

        Ties to other methods
        Used by session list endpoints and orchestrator status views.

        Why this exists
        Operators and test runners need to inspect current session occupancy.
        """

        with self._lock:
            return [record.to_dict() for record in self._sessions.values()]

    def get_session(self, session_id: str) -> SessionRecord:
        """
        Summary
        Read a session record by id.

        Inputs
        `session_id` as the session identifier.

        Outputs
        The matching `SessionRecord`.

        Side effects
        None.

        Error handling
        Raises `GridError` when the session does not exist.

        Ties to other methods
        Used by all session-scoped endpoints and helpers.

        Why this exists
        Session-scoped operations need a single authoritative lookup path.
        """

        with self._lock:
            record = self._sessions.get(session_id)
        if not record:
            raise GridError(f"[grid_sessions.py::SessionRegistry.get_session] Unknown session: {session_id}")
        return record

    def update_target(self, session_id: str, target_app: str | None) -> SessionRecord:
        """
        Summary
        Update the target application for a session.

        Inputs
        Session id and optional target app.

        Outputs
        The updated `SessionRecord`.

        Side effects
        Updates session state and refreshes the capture loop when present.

        Error handling
        Raises `GridError` when the session does not exist or does not support targeting.

        Ties to other methods
        Used by the session `/target` endpoint.

        Why this exists
        GUI sessions must be able to switch the app-relative frame and coordinate space at runtime.
        """

        record = self.get_session(session_id)
        record.target_app = target_app
        if record.frame_store:
            record.frame_store.set_target_app(target_app)
        self._log_event(record, "target_updated", {"target_app": target_app})
        return record

    def refresh_session(self, session_id: str) -> SessionRecord:
        """
        Summary
        Force an immediate frame refresh for a session.

        Inputs
        `session_id` as the session identifier.

        Outputs
        The refreshed `SessionRecord`.

        Side effects
        Captures new screenshots when the session has a frame store.

        Error handling
        Raises `GridError` when the session does not support frame capture.

        Ties to other methods
        Used by explicit refresh endpoints and polling callers.

        Why this exists
        Some automation flows need a guaranteed fresh frame before OCR or clicking.
        """

        record = self.get_session(session_id)
        if not record.frame_store:
            raise GridError(
                f"[grid_sessions.py::SessionRegistry.refresh_session] Session {session_id} does not support frame capture."
            )
        record.frame_store.refresh()
        self._log_event(record, "session_refreshed", {})
        return record

    def delete_session(self, session_id: str) -> dict[str, Any]:
        """
        Summary
        Delete a session lease and clean up its temporary state.

        Inputs
        `session_id` as the session identifier.

        Outputs
        A summary containing the deleted session id and retained artifact root.

        Side effects
        Stops any capture thread and removes the session temp directory while retaining artifacts.

        Error handling
        Raises `GridError` when the session does not exist.

        Ties to other methods
        Used by the session delete endpoint and orchestrator cleanup.

        Why this exists
        Sessions must release capacity and local resources cleanly after a test run finishes.
        """

        with self._lock:
            record = self._sessions.pop(session_id, None)
        if not record:
            raise GridError(
                f"[grid_sessions.py::SessionRegistry.delete_session] Unknown session: {session_id}"
            )
        if record.frame_store:
            record.frame_store.stop()
        remove_tree(record.temp_root)
        record.status = "deleted"
        self._log_event(record, "session_deleted", {})
        return {
            "session_id": record.session_id,
            "artifact_root": str(record.artifact_root),
            "status": record.status,
        }

    def host_status(self) -> dict[str, Any]:
        """
        Summary
        Report host worker capacity, support, and current sessions.

        Inputs
        None.

        Outputs
        A JSON-safe host status dictionary.

        Side effects
        None.

        Error handling
        None.

        Ties to other methods
        Used by the worker daemon `/health` endpoint and orchestrator worker registration.

        Why this exists
        Scheduling decisions depend on host support and current occupancy.
        """

        sessions = self.list_sessions()
        active_by_class: dict[str, int] = {key: 0 for key in self.worker_capacity}
        for session in sessions:
            worker_class = session["worker_class"]
            active_by_class[worker_class] = active_by_class.get(worker_class, 0) + 1
        return {
            **self.capabilities,
            "active_sessions": active_by_class,
            "sessions": sessions,
            "capture_interval_seconds": self.interval_seconds,
        }

    def session_frame_path(self, session_id: str, scope: str = "screen") -> Path:
        """
        Summary
        Resolve the current frame path for a session and scope.

        Inputs
        Session id and frame scope (`screen` or `app`).

        Outputs
        The frame image path.

        Side effects
        None.

        Error handling
        Raises `GridError` when the session has no frame store or the scope is invalid.

        Ties to other methods
        Used by the worker daemon frame endpoint and OCR helpers.

        Why this exists
        Session endpoints need a single way to resolve the active frame image.
        """

        record = self.get_session(session_id)
        if not record.frame_store:
            raise GridError(
                f"[grid_sessions.py::SessionRegistry.session_frame_path] Session {session_id} does not have a frame store."
            )
        snapshot = record.frame_store.snapshot()
        if scope == "app" and record.target_app:
            return Path(snapshot["app_frame_path"])
        return Path(snapshot["frame_path"])

    def translate_session_coordinates(
        self,
        session_id: str,
        x: int,
        y: int,
        coordinate_space: str = "session",
    ) -> tuple[int, int]:
        """
        Summary
        Translate session-relative coordinates into screen coordinates.

        Inputs
        Session id, raw x and y coordinates, and coordinate space mode.

        Outputs
        A tuple of absolute screen coordinates.

        Side effects
        Reads current session frame and window metadata.

        Error handling
        Raises `GridError` when session-relative coordinates are requested without app bounds.

        Ties to other methods
        Used by session click, drag, and text-targeted actions.

        Why this exists
        OCR boxes and session-local screenshots should translate directly into usable desktop coordinates.
        """

        if coordinate_space == "screen":
            return x, y
        record = self.get_session(session_id)
        if not record.frame_store or not record.target_app:
            return x, y
        snapshot = record.frame_store.snapshot()
        bounds = snapshot["window_bounds"]
        image_size = snapshot["app_image_size"]
        if not bounds or not image_size:
            raise GridError(
                f"[grid_sessions.py::SessionRegistry.translate_session_coordinates] Session {session_id} does not have app bounds."
            )
        width = max(int(image_size["width"]), 1)
        height = max(int(image_size["height"]), 1)
        translated_x = bounds["x"] + round(x * bounds["width"] / width)
        translated_y = bounds["y"] + round(y * bounds["height"] / height)
        return translated_x, translated_y

    def _log_event(self, record: SessionRecord, action: str, payload: dict[str, Any]) -> None:
        """
        Summary
        Append a session event record to the session action log.

        Inputs
        Session record, action name, and event payload.

        Outputs
        None.

        Side effects
        Writes a JSONL event record to disk.

        Error handling
        Propagates `GridError` from JSONL writing when disk operations fail.

        Ties to other methods
        Used by create, target, refresh, and delete flows.

        Why this exists
        Every session should retain a minimal audit trail for debugging automation failures.
        """

        append_jsonl(
            record.artifact_root / "session-events.jsonl",
            {
                "ts": utc_timestamp(),
                "session_id": record.session_id,
                "worker_class": record.worker_class,
                "action": action,
                "payload": payload,
            },
        )
