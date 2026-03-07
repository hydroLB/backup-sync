from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from grid_common import ensure_directory
from grid_sessions import SessionRegistry


class FakeFrameStore:
    """
    Summary
    Provide a deterministic fake frame store for coordinate translation tests.

    Inputs
    None.

    Outputs
    A session-like frame snapshot.

    Side effects
    None.

    Error handling
    None.

    Ties to other methods
    Used to test `SessionRegistry.translate_session_coordinates` without taking real screenshots.

    Why this exists
    Coordinate translation should be unit tested without depending on GUI capture.
    """

    def snapshot(self) -> dict[str, object]:
        """
        Summary
        Return a fake app-frame snapshot.

        Inputs
        None.

        Outputs
        A dictionary with app image size and window bounds.

        Side effects
        None.

        Error handling
        None.

        Ties to other methods
        Used by the fake session registry record.

        Why this exists
        Session-relative clicks depend on current frame and bounds metadata.
        """

        return {
            "app_image_size": {"width": 400, "height": 200},
            "window_bounds": {"x": 100, "y": 200, "width": 200, "height": 100},
            "frame_path": "",
            "app_frame_path": "",
            "frame_hash": None,
            "app_frame_hash": None,
            "image_size": None,
            "last_capture_ts": 0.0,
            "last_error": None,
            "target_app": "Backup Sync",
        }

    def stop(self) -> None:
        """
        Summary
        Provide a no-op stop method for session cleanup compatibility.

        Inputs
        None.

        Outputs
        None.

        Side effects
        None.

        Error handling
        None.

        Ties to other methods
        Used by `SessionRegistry.delete_session`.

        Why this exists
        The fake frame store replaces the real capture loop in tests and still needs the same cleanup surface.
        """

        return None


class GridSessionTests(unittest.TestCase):
    """
    Summary
    Validate session registry capacity, lifecycle, and coordinate translation.

    Inputs
    Standard unittest lifecycle hooks.

    Outputs
    Deterministic test assertions.

    Side effects
    Creates isolated temporary session roots through the real session registry.

    Error handling
    Unittest captures assertion failures and unexpected exceptions.

    Ties to other methods
    Exercises `SessionRegistry`.

    Why this exists
    Session isolation and coordinate translation are core guarantees of the new worker daemon.
    """

    def test_browser_session_capacity_is_released_after_delete(self) -> None:
        """
        Summary
        Confirm session deletion releases worker-class capacity.

        Inputs
        None.

        Outputs
        Assertion results.

        Side effects
        Creates and deletes browser-worker sessions on disk.

        Error handling
        Unittest captures assertion failures.

        Ties to other methods
        Exercises `create_session` and `delete_session`.

        Why this exists
        The worker daemon must queue beyond capacity cleanly and release capacity on teardown.
        """

        registry = SessionRegistry(
            interval_seconds=1.0,
            worker_capacity={"browser-worker": 1, "macos-gui-worker": 2, "linux-gui-worker": 0},
            capture_enabled=False,
        )
        first = registry.create_session("browser-worker", "tiered_visual")
        with self.assertRaisesRegex(Exception, "Capacity exhausted"):
            registry.create_session("browser-worker", "tiered_visual")
        registry.delete_session(first.session_id)
        second = registry.create_session("browser-worker", "tiered_visual")
        self.assertNotEqual(first.session_id, second.session_id)
        registry.delete_session(second.session_id)

    def test_session_relative_coordinates_translate_from_app_frame(self) -> None:
        """
        Summary
        Confirm session-relative pixel coordinates map into screen coordinates using current app bounds.

        Inputs
        None.

        Outputs
        Assertion results.

        Side effects
        Creates a browser session and swaps in fake frame metadata.

        Error handling
        Unittest captures assertion failures.

        Ties to other methods
        Exercises `translate_session_coordinates`.

        Why this exists
        OCR boxes are in frame pixels, while click injection must use screen coordinates.
        """

        registry = SessionRegistry(interval_seconds=1.0, capture_enabled=False)
        record = registry.create_session("browser-worker", "tiered_visual")
        record.worker_class = "macos-gui-worker"
        record.target_app = "Backup Sync"
        record.frame_store = FakeFrameStore()
        translated = registry.translate_session_coordinates(record.session_id, 200, 100)
        self.assertEqual((200, 250), translated)
        registry.delete_session(record.session_id)


if __name__ == "__main__":
    unittest.main()
