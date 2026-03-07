from __future__ import annotations

import json
import shlex
import subprocess
import threading
import time
import urllib.error
import urllib.request
from dataclasses import dataclass, field
from http import HTTPStatus
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Any

from grid_common import (
    DEFAULT_WORKER_CAPACITY,
    GridError,
    JOBS_ROOT,
    append_jsonl,
    ensure_directory,
    host_worker_capabilities,
    make_id,
    remove_tree,
    shell_env_with,
    utc_timestamp,
    write_json,
)


class DesktopWorkerClient:
    """
    Summary
    Provide a minimal HTTP client for talking to the local worker daemon.

    Inputs
    `base_url` as the worker daemon root URL.

    Outputs
    A client that can create and delete worker sessions over HTTP.

    Side effects
    Performs HTTP requests against the local worker daemon.

    Error handling
    Raises `GridError` when the worker daemon is unreachable or returns invalid responses.

    Ties to other methods
    Used by the orchestrator dispatcher for GUI session leasing.

    Why this exists
    The orchestrator and worker daemon should stay decoupled processes with a stable HTTP boundary.
    """

    def __init__(self, base_url: str) -> None:
        self.base_url = base_url.rstrip("/")

    def create_session(
        self,
        worker_class: str,
        visual_mode: str,
        target_app: str | None,
        metadata: dict[str, Any],
    ) -> dict[str, Any]:
        """
        Summary
        Create a worker session on the local worker daemon.

        Inputs
        Worker class, visual mode, optional target app, and metadata.

        Outputs
        The created session metadata dictionary.

        Side effects
        Sends an HTTP POST request to the worker daemon.

        Error handling
        Raises `GridError` when the worker daemon rejects the request.

        Ties to other methods
        Used by job dispatch before launching the actual app command.

        Why this exists
        GUI jobs need a session lease before their processes start.
        """

        return self._request_json(
            "/sessions",
            {
                "worker_class": worker_class,
                "visual_mode": visual_mode,
                "target_app": target_app,
                "metadata": metadata,
            },
        )

    def delete_session(self, session_id: str) -> dict[str, Any]:
        """
        Summary
        Delete a worker session on the local worker daemon.

        Inputs
        `session_id` as the leased session identifier.

        Outputs
        The worker daemon delete summary.

        Side effects
        Sends an HTTP DELETE request to the worker daemon.

        Error handling
        Raises `GridError` when the worker daemon rejects the request.

        Ties to other methods
        Used by orchestrator cleanup.

        Why this exists
        GUI capacity must be released after job completion or failure.
        """

        request = urllib.request.Request(
            f"{self.base_url}/sessions/{session_id}",
            method="DELETE",
        )
        return self._open_json(request)

    def health(self) -> dict[str, Any]:
        """
        Summary
        Read worker daemon health and capacity metadata.

        Inputs
        None.

        Outputs
        The worker daemon health payload.

        Side effects
        Sends an HTTP GET request to the worker daemon.

        Error handling
        Raises `GridError` when the worker daemon is unreachable or returns invalid JSON.

        Ties to other methods
        Used to build orchestrator worker registration state.

        Why this exists
        The orchestrator should expose the current host worker support to callers.
        """

        request = urllib.request.Request(f"{self.base_url}/health", method="GET")
        return self._open_json(request)

    def _request_json(self, path: str, payload: dict[str, Any]) -> dict[str, Any]:
        """
        Summary
        Send a JSON POST request and decode the JSON response.

        Inputs
        URL path and request payload.

        Outputs
        Parsed JSON response dictionary.

        Side effects
        Performs an HTTP request.

        Error handling
        Raises `GridError` when the request fails or the response is invalid.

        Ties to other methods
        Shared by orchestrator session management calls.

        Why this exists
        JSON worker endpoints should be wrapped consistently.
        """

        request = urllib.request.Request(
            f"{self.base_url}{path}",
            data=json.dumps(payload).encode("utf-8"),
            headers={"Content-Type": "application/json"},
            method="POST",
        )
        return self._open_json(request)

    def _open_json(self, request: urllib.request.Request) -> dict[str, Any]:
        """
        Summary
        Open an HTTP request and decode a JSON object response.

        Inputs
        A prepared `urllib` request object.

        Outputs
        Parsed JSON response dictionary.

        Side effects
        Performs network I/O against the local worker daemon.

        Error handling
        Raises `GridError` when the request fails or the response is not a JSON object.

        Ties to other methods
        Used by all public worker daemon client methods.

        Why this exists
        The orchestrator should surface clear worker-daemon failures instead of low-level urllib exceptions.
        """

        try:
            with urllib.request.urlopen(request, timeout=10) as response:
                payload = json.loads(response.read().decode("utf-8"))
        except (urllib.error.URLError, urllib.error.HTTPError, json.JSONDecodeError) as exc:
            raise GridError(f"[grid_orchestrator.py::DesktopWorkerClient._open_json] Worker daemon request failed: {exc}") from exc
        if not isinstance(payload, dict):
            raise GridError("[grid_orchestrator.py::DesktopWorkerClient._open_json] Worker daemon response was not a JSON object.")
        return payload


@dataclass
class JobRecord:
    """
    Summary
    Represent one queued or running orchestrator job.

    Inputs
    Dataclass fields describing the requested work and execution lifecycle.

    Outputs
    Mutable job metadata.

    Side effects
    None directly; execution state is managed by `GridOrchestrator`.

    Error handling
    Failures are stored in `last_error` and `summary_path`.

    Ties to other methods
    Used by the scheduler, job endpoints, and test assertions.

    Why this exists
    Concurrent orchestration needs stable per-job state and artifacts.
    """

    job_id: str
    worker_class: str
    launch_command: str | list[str]
    cwd: str
    app_name: str | None
    surface_type: str
    visual_mode: str
    artifact_policy: dict[str, Any]
    timeout_seconds: int
    aesthetic_review: bool
    job_root: Path
    artifact_root: Path
    temp_root: Path
    status: str = "queued"
    created_at: str = field(default_factory=utc_timestamp)
    started_at: str | None = None
    finished_at: str | None = None
    session_id: str | None = None
    return_code: int | None = None
    stdout_path: Path | None = None
    stderr_path: Path | None = None
    summary_path: Path | None = None
    last_error: str | None = None

    def to_dict(self) -> dict[str, Any]:
        """
        Summary
        Convert the job record to a JSON-safe dictionary.

        Inputs
        None.

        Outputs
        A JSON-serializable job dictionary.

        Side effects
        None.

        Error handling
        None.

        Ties to other methods
        Used by job API responses and persisted summaries.

        Why this exists
        Job state should be easy to inspect both programmatically and on disk.
        """

        return {
            "job_id": self.job_id,
            "worker_class": self.worker_class,
            "launch_command": self.launch_command,
            "cwd": self.cwd,
            "app_name": self.app_name,
            "surface_type": self.surface_type,
            "visual_mode": self.visual_mode,
            "artifact_policy": self.artifact_policy,
            "timeout_seconds": self.timeout_seconds,
            "aesthetic_review": self.aesthetic_review,
            "status": self.status,
            "created_at": self.created_at,
            "started_at": self.started_at,
            "finished_at": self.finished_at,
            "session_id": self.session_id,
            "return_code": self.return_code,
            "stdout_path": str(self.stdout_path) if self.stdout_path else None,
            "stderr_path": str(self.stderr_path) if self.stderr_path else None,
            "summary_path": str(self.summary_path) if self.summary_path else None,
            "artifact_root": str(self.artifact_root),
            "temp_root": str(self.temp_root),
            "last_error": self.last_error,
        }


class GridOrchestrator:
    """
    Summary
    Queue, schedule, execute, and summarize concurrent mixed-surface test jobs.

    Inputs
    Worker daemon client, optional worker capacities, and scheduler poll interval.

    Outputs
    A mutable orchestrator that manages worker registration and job execution.

    Side effects
    Creates per-job directories, spawns subprocesses, and communicates with the worker daemon.

    Error handling
    Records contextual errors per job and raises `GridError` for invalid API usage.

    Ties to other methods
    Used by the `gridd` HTTP server and orchestration tests.

    Why this exists
    Scaling to 20 programs cleanly requires a queueing and capacity-aware control plane above the worker daemon.
    """

    def __init__(
        self,
        desktop_client: DesktopWorkerClient,
        worker_capacity: dict[str, int] | None = None,
        scheduler_poll_seconds: float = 0.5,
    ) -> None:
        self.desktop_client = desktop_client
        self.worker_capacity = dict(DEFAULT_WORKER_CAPACITY)
        if worker_capacity:
            self.worker_capacity.update(worker_capacity)
        self.scheduler_poll_seconds = scheduler_poll_seconds
        self.capabilities = host_worker_capabilities()
        self._jobs: dict[str, JobRecord] = {}
        self._active_by_worker: dict[str, int] = {key: 0 for key in self.worker_capacity}
        self._lock = threading.Lock()
        self._stop = threading.Event()
        self._thread = threading.Thread(target=self._scheduler_loop, daemon=True)
        ensure_directory(JOBS_ROOT)

    def start(self) -> None:
        """
        Summary
        Start the orchestrator scheduler loop.

        Inputs
        None.

        Outputs
        None.

        Side effects
        Starts the scheduler background thread.

        Error handling
        None.

        Ties to other methods
        Called by the `gridd` process entrypoint.

        Why this exists
        Queued jobs should dispatch automatically without external polling logic.
        """

        self._thread.start()

    def stop(self) -> None:
        """
        Summary
        Stop the orchestrator scheduler loop.

        Inputs
        None.

        Outputs
        None.

        Side effects
        Signals and joins the scheduler thread.

        Error handling
        None.

        Ties to other methods
        Called during `gridd` shutdown.

        Why this exists
        The orchestrator should exit cleanly without leaving scheduling threads behind.
        """

        self._stop.set()
        self._thread.join(timeout=2)

    def submit_job(self, payload: dict[str, Any]) -> JobRecord:
        """
        Summary
        Validate and enqueue a new orchestration job.

        Inputs
        JSON-like job payload matching the public grid job schema.

        Outputs
        The created `JobRecord`.

        Side effects
        Creates per-job directories and writes an initial job summary.

        Error handling
        Raises `GridError` when required fields are missing or invalid.

        Ties to other methods
        Used by the job creation HTTP endpoint and tests.

        Why this exists
        Callers need a single stable entrypoint for mixed-surface test work.
        """

        worker_class = self._required_str(payload, "worker_class")
        if worker_class not in self.worker_capacity:
            raise GridError(f"[grid_orchestrator.py::GridOrchestrator.submit_job] Unknown worker class: {worker_class}")
        launch_command = payload.get("launch_command")
        if not isinstance(launch_command, (str, list)) or (
            isinstance(launch_command, list) and not all(isinstance(part, str) for part in launch_command)
        ):
            raise GridError("[grid_orchestrator.py::GridOrchestrator.submit_job] launch_command must be a string or a string list.")
        visual_mode = self._optional_str(payload, "visual_mode", "tiered_visual")
        cwd = self._optional_str(payload, "cwd", ".")
        app_name = payload.get("app_name")
        if app_name is not None and not isinstance(app_name, str):
            raise GridError("[grid_orchestrator.py::GridOrchestrator.submit_job] app_name must be a string when provided.")
        timeouts = payload.get("timeouts") or {}
        if not isinstance(timeouts, dict):
            raise GridError("[grid_orchestrator.py::GridOrchestrator.submit_job] timeouts must be an object when provided.")
        artifact_policy = payload.get("artifact_policy") or {}
        if not isinstance(artifact_policy, dict):
            raise GridError("[grid_orchestrator.py::GridOrchestrator.submit_job] artifact_policy must be an object when provided.")
        job_id = make_id("job")
        job_root = ensure_directory(JOBS_ROOT / job_id)
        artifact_root = ensure_directory(job_root / "artifacts")
        temp_root = ensure_directory(job_root / "temp")
        record = JobRecord(
            job_id=job_id,
            worker_class=worker_class,
            launch_command=launch_command,
            cwd=cwd,
            app_name=app_name,
            surface_type=self._optional_str(payload, "surface_type", "application"),
            visual_mode=visual_mode,
            artifact_policy=dict(artifact_policy),
            timeout_seconds=int(timeouts.get("total_seconds", payload.get("timeout_seconds", 900))),
            aesthetic_review=bool(payload.get("aesthetic_review", False)),
            job_root=job_root,
            artifact_root=artifact_root,
            temp_root=temp_root,
        )
        record.stdout_path = artifact_root / "stdout.log"
        record.stderr_path = artifact_root / "stderr.log"
        record.summary_path = artifact_root / "summary.json"
        with self._lock:
            self._jobs[job_id] = record
        self._write_summary(record)
        self._log_event(record, "job_submitted", {"visual_mode": visual_mode})
        return record

    def list_jobs(self) -> list[dict[str, Any]]:
        """
        Summary
        List all known jobs.

        Inputs
        None.

        Outputs
        A list of job metadata dictionaries.

        Side effects
        None.

        Error handling
        None.

        Ties to other methods
        Used by the job list endpoint and tests.

        Why this exists
        Operators need to inspect queue and execution state.
        """

        with self._lock:
            return [job.to_dict() for job in self._jobs.values()]

    def get_job(self, job_id: str) -> JobRecord:
        """
        Summary
        Retrieve a job by id.

        Inputs
        `job_id` as the job identifier.

        Outputs
        The matching `JobRecord`.

        Side effects
        None.

        Error handling
        Raises `GridError` when the job does not exist.

        Ties to other methods
        Used by job detail endpoints and scheduler helpers.

        Why this exists
        Job-scoped operations need a single authoritative lookup path.
        """

        with self._lock:
            job = self._jobs.get(job_id)
        if not job:
            raise GridError(f"[grid_orchestrator.py::GridOrchestrator.get_job] Unknown job: {job_id}")
        return job

    def worker_status(self) -> dict[str, Any]:
        """
        Summary
        Return the orchestrator worker registry and occupancy.

        Inputs
        None.

        Outputs
        A JSON-safe worker status dictionary.

        Side effects
        Reads worker daemon health through the desktop client.

        Error handling
        Returns worker daemon errors in a degraded status payload instead of raising.

        Ties to other methods
        Used by the orchestrator health and worker endpoints.

        Why this exists
        Scheduling and operations need both local queue state and worker-host support information.
        """

        daemon_error = None
        daemon_health: dict[str, Any] | None = None
        try:
            daemon_health = self.desktop_client.health()
        except GridError as exc:
            daemon_error = str(exc)
        return {
            "capacity": self.worker_capacity,
            "active_jobs": self._active_by_worker,
            "supported": self.capabilities["supported"],
            "worker_daemon": daemon_health,
            "worker_daemon_error": daemon_error,
        }

    def _scheduler_loop(self) -> None:
        """
        Summary
        Dispatch queued jobs whenever worker capacity is available.

        Inputs
        None.

        Outputs
        None.

        Side effects
        Starts job execution threads and updates job state.

        Error handling
        Job-specific dispatch failures are recorded on the affected job.

        Ties to other methods
        Private worker started by `start`.

        Why this exists
        The orchestrator should queue beyond capacity and dispatch automatically once workers free up.
        """

        while not self._stop.wait(self.scheduler_poll_seconds):
            queued: list[JobRecord] = []
            with self._lock:
                queued = [
                    job
                    for job in self._jobs.values()
                    if job.status == "queued"
                ]
            for job in queued:
                if self._active_by_worker.get(job.worker_class, 0) >= self.worker_capacity[job.worker_class]:
                    continue
                self._start_job(job)

    def _start_job(self, job: JobRecord) -> None:
        """
        Summary
        Transition a queued job into a running execution thread.

        Inputs
        `job` as the queued job record.

        Outputs
        None.

        Side effects
        Updates active worker counts and starts a background execution thread.

        Error handling
        Guards against duplicate dispatch with a lock.

        Ties to other methods
        Used by the scheduler loop.

        Why this exists
        Queue dispatch should stay atomic when multiple scheduler ticks observe the same job.
        """

        with self._lock:
            if job.status != "queued":
                return
            job.status = "leased"
            job.started_at = utc_timestamp()
            self._active_by_worker[job.worker_class] = self._active_by_worker.get(job.worker_class, 0) + 1
        self._write_summary(job)
        self._log_event(job, "job_leased", {})
        threading.Thread(target=self._run_job, args=(job,), daemon=True).start()

    def _run_job(self, job: JobRecord) -> None:
        """
        Summary
        Execute one job, manage its worker session, and persist final artifacts.

        Inputs
        `job` as the leased job record.

        Outputs
        None.

        Side effects
        May create and delete worker sessions, run a subprocess, and write output artifacts.

        Error handling
        Records failures on the job record and always releases worker capacity.

        Ties to other methods
        Used by `_start_job`.

        Why this exists
        Orchestrated runs need deterministic setup, execution, teardown, and result summaries.
        """

        session_id = None
        try:
            if job.worker_class in {"macos-gui-worker", "linux-gui-worker"}:
                if (
                    job.worker_class == "linux-gui-worker"
                    and not self.capabilities["supported"]["linux-gui-worker"]
                ):
                    raise GridError(
                        "[grid_orchestrator.py::GridOrchestrator._run_job] linux-gui-worker is not supported on this host."
                    )
                session = self.desktop_client.create_session(
                    worker_class=job.worker_class,
                    visual_mode=job.visual_mode,
                    target_app=job.app_name if job.worker_class != "browser-worker" else None,
                    metadata={
                        "job_id": job.job_id,
                        "surface_type": job.surface_type,
                        "aesthetic_review": job.aesthetic_review,
                    },
                )
                session_id = session["session_id"]
                job.session_id = session_id

            job.status = "running"
            self._write_summary(job)
            self._log_event(job, "job_running", {"session_id": session_id})

            env = shell_env_with(
                None,
                {
                    "CODEX_GRID_JOB_ID": job.job_id,
                    "CODEX_GRID_SESSION_ID": session_id or "",
                    "CODEX_GRID_WORKER_CLASS": job.worker_class,
                    "CODEX_GRID_VISUAL_MODE": job.visual_mode,
                    "CODEX_GRID_ARTIFACT_DIR": str(job.artifact_root),
                    "CODEX_GRID_AESTHETIC_REVIEW": "1" if job.aesthetic_review else "0",
                    "PLAYWRIGHT_OUTPUT_DIR": str(job.artifact_root / "playwright"),
                },
            )
            ensure_directory(Path(env["PLAYWRIGHT_OUTPUT_DIR"]))
            command = (
                job.launch_command
                if isinstance(job.launch_command, list)
                else ["/bin/zsh", "-lc", job.launch_command]
            )
            with job.stdout_path.open("w", encoding="utf-8") as stdout_handle, job.stderr_path.open(
                "w", encoding="utf-8"
            ) as stderr_handle:
                process = subprocess.Popen(
                    command,
                    cwd=job.cwd,
                    stdout=stdout_handle,
                    stderr=stderr_handle,
                    env=env,
                )
                try:
                    return_code = process.wait(timeout=job.timeout_seconds)
                except subprocess.TimeoutExpired:
                    process.kill()
                    return_code = process.wait(timeout=5)
                    job.last_error = (
                        f"[grid_orchestrator.py::GridOrchestrator._run_job] Job timed out after {job.timeout_seconds}s."
                    )
            job.return_code = return_code
            if return_code == 0 and not job.last_error:
                job.status = "succeeded"
            else:
                job.status = "failed"
                if not job.last_error:
                    job.last_error = f"Job exited with status {return_code}."
                if not job.aesthetic_review:
                    job.artifact_policy["auto_promoted_visual_capture"] = True
        except Exception as exc:  # noqa: BLE001
            job.status = "failed"
            job.last_error = f"[grid_orchestrator.py::GridOrchestrator._run_job] {exc}"
        finally:
            if session_id:
                try:
                    self.desktop_client.delete_session(session_id)
                except GridError as exc:
                    if not job.last_error:
                        job.last_error = str(exc)
            remove_tree(job.temp_root)
            job.finished_at = utc_timestamp()
            self._write_summary(job)
            self._log_event(
                job,
                "job_finished",
                {"status": job.status, "return_code": job.return_code, "session_id": session_id},
            )
            with self._lock:
                self._active_by_worker[job.worker_class] = max(
                    0, self._active_by_worker.get(job.worker_class, 1) - 1
                )

    def _write_summary(self, job: JobRecord) -> None:
        """
        Summary
        Persist the current job summary to disk.

        Inputs
        `job` as the job to serialize.

        Outputs
        None.

        Side effects
        Writes a JSON summary file under the job artifact directory.

        Error handling
        Propagates `GridError` when writing fails.

        Ties to other methods
        Used after every major job state transition.

        Why this exists
        Job results should stay inspectable even if the orchestrator process crashes later.
        """

        if not job.summary_path:
            raise GridError("[grid_orchestrator.py::GridOrchestrator._write_summary] Missing job summary path.")
        write_json(job.summary_path, job.to_dict())

    def _log_event(self, job: JobRecord, action: str, payload: dict[str, Any]) -> None:
        """
        Summary
        Append an event to the job event log.

        Inputs
        Job record, action name, and event payload.

        Outputs
        None.

        Side effects
        Writes a JSONL event record to the job artifact directory.

        Error handling
        Propagates `GridError` when the log write fails.

        Ties to other methods
        Used throughout job submission, dispatch, execution, and cleanup.

        Why this exists
        Per-job event logs are critical when many concurrent runs are active.
        """

        append_jsonl(
            job.artifact_root / "job-events.jsonl",
            {
                "ts": utc_timestamp(),
                "job_id": job.job_id,
                "worker_class": job.worker_class,
                "action": action,
                "payload": payload,
            },
        )

    def _required_str(self, payload: dict[str, Any], key: str) -> str:
        """
        Summary
        Read a required non-empty string field from a payload.

        Inputs
        JSON-like payload and field name.

        Outputs
        The validated string value.

        Side effects
        None.

        Error handling
        Raises `GridError` when the field is missing or invalid.

        Ties to other methods
        Used by `submit_job`.

        Why this exists
        Public job submission should validate core fields explicitly.
        """

        value = payload.get(key)
        if not isinstance(value, str) or not value.strip():
            raise GridError(f"[grid_orchestrator.py::GridOrchestrator._required_str] Field {key!r} must be a non-empty string.")
        return value

    def _optional_str(self, payload: dict[str, Any], key: str, default: str) -> str:
        """
        Summary
        Read an optional string field with a default.

        Inputs
        JSON-like payload, field name, and default value.

        Outputs
        The provided string value or the default.

        Side effects
        None.

        Error handling
        Raises `GridError` when a provided value is not a string.

        Ties to other methods
        Used by `submit_job`.

        Why this exists
        Optional job fields still need strict typing.
        """

        value = payload.get(key, default)
        if not isinstance(value, str):
            raise GridError(f"[grid_orchestrator.py::GridOrchestrator._optional_str] Field {key!r} must be a string.")
        return value


class GridRequestHandler(BaseHTTPRequestHandler):
    """
    Summary
    Serve orchestrator HTTP endpoints for jobs and worker status.

    Inputs
    Standard HTTP requests handled by `BaseHTTPRequestHandler`.

    Outputs
    JSON HTTP responses.

    Side effects
    Reads and mutates orchestrator job state.

    Error handling
    Returns contextual JSON error payloads on failures.

    Ties to other methods
    Uses `GridOrchestrator` for all stateful behavior.

    Why this exists
    The orchestrator should be scriptable from agents and external tooling through a simple HTTP API.
    """

    orchestrator: GridOrchestrator

    def log_message(self, format: str, *args: Any) -> None:
        return

    def do_GET(self) -> None:  # noqa: N802
        try:
            if self.path == "/health":
                self._json_response(HTTPStatus.OK, self.orchestrator.worker_status())
                return
            if self.path == "/workers":
                self._json_response(HTTPStatus.OK, self.orchestrator.worker_status())
                return
            if self.path == "/jobs":
                self._json_response(HTTPStatus.OK, {"jobs": self.orchestrator.list_jobs()})
                return
            if self.path.startswith("/jobs/"):
                job_id = self.path.split("/", 2)[2]
                self._json_response(HTTPStatus.OK, self.orchestrator.get_job(job_id).to_dict())
                return
            self._json_response(HTTPStatus.NOT_FOUND, {"error": "not_found"})
        except Exception as exc:  # noqa: BLE001
            self._json_response(HTTPStatus.BAD_REQUEST, {"error": str(exc)})

    def do_POST(self) -> None:  # noqa: N802
        try:
            if self.path != "/jobs":
                self._json_response(HTTPStatus.NOT_FOUND, {"error": "not_found"})
                return
            payload = self._read_json_body()
            job = self.orchestrator.submit_job(payload)
            self._json_response(HTTPStatus.CREATED, job.to_dict())
        except Exception as exc:  # noqa: BLE001
            self._json_response(HTTPStatus.BAD_REQUEST, {"error": str(exc)})

    def _read_json_body(self) -> dict[str, Any]:
        """
        Summary
        Read and decode a JSON request body.

        Inputs
        None.

        Outputs
        A JSON object dictionary.

        Side effects
        Reads bytes from the client socket.

        Error handling
        Raises `GridError` when the body is invalid JSON or not an object.

        Ties to other methods
        Used by POST endpoints.

        Why this exists
        The orchestrator uses a JSON-only API for deterministic automation.
        """

        length = int(self.headers.get("Content-Length", "0"))
        raw = self.rfile.read(length) if length > 0 else b"{}"
        try:
            payload = json.loads(raw.decode("utf-8"))
        except json.JSONDecodeError as exc:
            raise GridError(f"[grid_orchestrator.py::GridRequestHandler._read_json_body] Invalid JSON body: {exc}") from exc
        if not isinstance(payload, dict):
            raise GridError("[grid_orchestrator.py::GridRequestHandler._read_json_body] JSON body must be an object.")
        return payload

    def _json_response(self, status: HTTPStatus, payload: Any) -> None:
        """
        Summary
        Write a JSON response to the client.

        Inputs
        HTTP status and JSON-serializable payload.

        Outputs
        None.

        Side effects
        Writes response bytes to the client socket.

        Error handling
        Assumes the payload is JSON-serializable.

        Ties to other methods
        Used by all orchestrator endpoints.

        Why this exists
        Consistent JSON responses make automation easier to debug and consume.
        """

        encoded = json.dumps(payload).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(encoded)))
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        self.wfile.write(encoded)


def build_http_server(
    host: str,
    port: int,
    desktop_base_url: str,
    worker_capacity: dict[str, int] | None = None,
) -> tuple[ThreadingHTTPServer, GridOrchestrator]:
    """
    Summary
    Build the orchestrator HTTP server and scheduler instance.

    Inputs
    Host, port, desktop worker daemon base URL, and optional worker capacities.

    Outputs
    A configured HTTP server and the orchestrator instance it serves.

    Side effects
    Instantiates the orchestrator and binds the HTTP server socket.

    Error handling
    Propagates socket binding or initialization errors to the caller.

    Ties to other methods
    Used by `gridd.py`.

    Why this exists
    The process entrypoint should stay thin and defer actual orchestration logic to reusable classes.
    """

    orchestrator = GridOrchestrator(
        desktop_client=DesktopWorkerClient(desktop_base_url),
        worker_capacity=worker_capacity,
    )
    handler_cls = type("GridHttpHandler", (GridRequestHandler,), {"orchestrator": orchestrator})
    server = ThreadingHTTPServer((host, port), handler_cls)
    return server, orchestrator
