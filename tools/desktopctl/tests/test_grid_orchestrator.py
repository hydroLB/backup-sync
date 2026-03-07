from __future__ import annotations

import tempfile
import time
import unittest
from pathlib import Path

from grid_orchestrator import GridOrchestrator


class FakeDesktopClient:
    """
    Summary
    Provide a deterministic fake worker-daemon client for orchestrator tests.

    Inputs
    None.

    Outputs
    Fake session and health payloads.

    Side effects
    Tracks created and deleted sessions in memory.

    Error handling
    None.

    Ties to other methods
    Used by `GridOrchestrator` tests.

    Why this exists
    The orchestrator scheduler should be unit tested without a real worker daemon process.
    """

    def __init__(self) -> None:
        self.created: list[str] = []
        self.deleted: list[str] = []

    def create_session(
        self,
        worker_class: str,
        visual_mode: str,
        target_app: str | None,
        metadata: dict[str, object],
    ) -> dict[str, object]:
        """
        Summary
        Return a fake session lease.

        Inputs
        Worker class, visual mode, target app, and metadata.

        Outputs
        A fake session metadata dictionary.

        Side effects
        Appends the session id to the in-memory created list.

        Error handling
        None.

        Ties to other methods
        Used by `GridOrchestrator._run_job`.

        Why this exists
        GUI jobs need fake session ids for queueing and cleanup assertions.
        """

        session_id = f"session-{len(self.created) + 1}"
        self.created.append(session_id)
        return {
            "session_id": session_id,
            "worker_class": worker_class,
            "visual_mode": visual_mode,
            "target_app": target_app,
            "metadata": metadata,
        }

    def delete_session(self, session_id: str) -> dict[str, object]:
        """
        Summary
        Record fake session deletion.

        Inputs
        `session_id` as the fake session identifier.

        Outputs
        A fake deletion summary.

        Side effects
        Appends the session id to the deleted list.

        Error handling
        None.

        Ties to other methods
        Used by `GridOrchestrator._run_job`.

        Why this exists
        Cleanup assertions need to confirm the orchestrator releases sessions.
        """

        self.deleted.append(session_id)
        return {"session_id": session_id, "status": "deleted"}

    def health(self) -> dict[str, object]:
        """
        Summary
        Return a fake worker health payload.

        Inputs
        None.

        Outputs
        A minimal worker health dictionary.

        Side effects
        None.

        Error handling
        None.

        Ties to other methods
        Used by orchestrator health reporting.

        Why this exists
        The orchestrator status endpoint expects worker health data.
        """

        return {"supported": {"browser-worker": True, "macos-gui-worker": True, "linux-gui-worker": False}}


class GridOrchestratorTests(unittest.TestCase):
    """
    Summary
    Validate orchestrator queueing, execution, and cleanup.

    Inputs
    Standard unittest lifecycle hooks.

    Outputs
    Deterministic test assertions.

    Side effects
    Launches short-lived Python subprocesses through the real orchestrator runner.

    Error handling
    Unittest captures assertion failures and unexpected exceptions.

    Ties to other methods
    Exercises `GridOrchestrator`.

    Why this exists
    The mixed worker grid needs proof that capacity limits and cleanup behave correctly.
    """

    def test_jobs_queue_when_worker_capacity_is_full(self) -> None:
        """
        Summary
        Confirm additional jobs remain queued until worker capacity frees up.

        Inputs
        None.

        Outputs
        Assertion results.

        Side effects
        Starts the orchestrator scheduler and launches short-lived subprocesses.

        Error handling
        Unittest captures assertion failures.

        Ties to other methods
        Exercises scheduler dispatch and cleanup behavior.

        Why this exists
        Clean concurrent testing depends on queueing beyond capacity instead of overlapping sessions.
        """

        desktop_client = FakeDesktopClient()
        orchestrator = GridOrchestrator(
            desktop_client=desktop_client,
            worker_capacity={"browser-worker": 1, "macos-gui-worker": 1, "linux-gui-worker": 0},
            scheduler_poll_seconds=0.05,
        )
        orchestrator.start()
        try:
            with tempfile.TemporaryDirectory() as tmp:
                first = orchestrator.submit_job(
                    {
                        "worker_class": "browser-worker",
                        "launch_command": "python3 -c 'import time; time.sleep(0.4)'",
                        "cwd": tmp,
                        "surface_type": "website",
                        "visual_mode": "tiered_visual",
                    }
                )
                second = orchestrator.submit_job(
                    {
                        "worker_class": "browser-worker",
                        "launch_command": "python3 -c 'print(\"done\")'",
                        "cwd": tmp,
                        "surface_type": "website",
                        "visual_mode": "tiered_visual",
                    }
                )
                time.sleep(0.15)
                self.assertIn(orchestrator.get_job(first.job_id).status, {"leased", "running"})
                self.assertEqual("queued", orchestrator.get_job(second.job_id).status)
                deadline = time.time() + 5
                while time.time() < deadline:
                    if orchestrator.get_job(first.job_id).status == "succeeded" and orchestrator.get_job(second.job_id).status == "succeeded":
                        break
                    time.sleep(0.05)
                self.assertEqual("succeeded", orchestrator.get_job(first.job_id).status)
                self.assertEqual("succeeded", orchestrator.get_job(second.job_id).status)
                self.assertEqual([], desktop_client.created)
        finally:
            orchestrator.stop()

    def test_gui_job_creates_and_deletes_session(self) -> None:
        """
        Summary
        Confirm GUI jobs lease and release worker-daemon sessions.

        Inputs
        None.

        Outputs
        Assertion results.

        Side effects
        Starts the orchestrator scheduler and launches a short-lived subprocess.

        Error handling
        Unittest captures assertion failures.

        Ties to other methods
        Exercises GUI session lifecycle integration.

        Why this exists
        Native GUI capacity depends on explicit session cleanup after each run.
        """

        desktop_client = FakeDesktopClient()
        orchestrator = GridOrchestrator(
            desktop_client=desktop_client,
            worker_capacity={"browser-worker": 1, "macos-gui-worker": 1, "linux-gui-worker": 0},
            scheduler_poll_seconds=0.05,
        )
        orchestrator.start()
        try:
            with tempfile.TemporaryDirectory() as tmp:
                job = orchestrator.submit_job(
                    {
                        "worker_class": "macos-gui-worker",
                        "launch_command": "python3 -c 'print(\"gui\")'",
                        "cwd": tmp,
                        "surface_type": "application",
                        "visual_mode": "tiered_visual",
                        "app_name": "Backup Sync",
                    }
                )
                deadline = time.time() + 5
                while time.time() < deadline:
                    if orchestrator.get_job(job.job_id).status in {"succeeded", "failed"}:
                        break
                    time.sleep(0.05)
                self.assertEqual("succeeded", orchestrator.get_job(job.job_id).status)
                self.assertEqual(["session-1"], desktop_client.created)
                self.assertEqual(["session-1"], desktop_client.deleted)
        finally:
            orchestrator.stop()


if __name__ == "__main__":
    unittest.main()
