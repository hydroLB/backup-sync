#!/usr/bin/env python3

from __future__ import annotations

import argparse
import contextlib
import io
import json
import time
from http import HTTPStatus
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Any
from urllib.parse import parse_qs, urlparse

from desktopctl import (
    activate_command as desktop_activate_command,
    ax_buttons_command as desktop_ax_buttons_command,
    click_command as desktop_click_command,
    color_command as desktop_color_command,
    cursor_command as desktop_cursor_command,
    double_click_command as desktop_double_click_command,
    drag_command as desktop_drag_command,
    key_command as desktop_key_command,
    move_command as desktop_move_command,
    ocr_command as desktop_ocr_command,
    right_click_command as desktop_right_click_command,
    type_command as desktop_type_command,
    windows_command as desktop_windows_command,
)
from grid_common import GridError
from grid_ocr import find_text_matches
from grid_sessions import SessionRegistry


class DesktopDaemonError(RuntimeError):
    """
    Summary
    Provide a typed error for worker daemon request failures.

    Inputs
    Standard exception message text.

    Outputs
    An exception instance carrying contextual worker-daemon error text.

    Side effects
    Captures a traceback when raised.

    Error handling
    Used as the canonical exception type for request validation and command failures.

    Ties to other methods
    Shared across request parsing and endpoint action helpers in this module.

    Why this exists
    HTTP handlers need one stable failure type that can be converted into clear JSON error responses.
    """


def _namespace(**kwargs: Any) -> argparse.Namespace:
    """
    Summary
    Build an argparse namespace for reusing existing `desktopctl` command handlers.

    Inputs
    Arbitrary keyword arguments representing handler parameters.

    Outputs
    An `argparse.Namespace` instance.

    Side effects
    None.

    Error handling
    None.

    Ties to other methods
    Used by action helpers that call existing `desktopctl` subcommand implementations.

    Why this exists
    The daemon should reuse the local CLI command implementations instead of duplicating low-level desktop logic.
    """

    return argparse.Namespace(**kwargs)


def _capture_text_output(func: Any, args: argparse.Namespace) -> str:
    """
    Summary
    Run a command handler and capture its stdout as text.

    Inputs
    A `desktopctl` command function and argument namespace.

    Outputs
    The captured stdout text.

    Side effects
    Temporarily redirects stdout.

    Error handling
    Propagates command failures to the caller.

    Ties to other methods
    Used by JSON endpoints that wrap textual `desktopctl` commands.

    Why this exists
    The worker daemon needs structured JSON responses even when the reused CLI handlers print to stdout.
    """

    buffer = io.StringIO()
    with contextlib.redirect_stdout(buffer):
        func(args)
    return buffer.getvalue()


def _capture_json_output(func: Any, args: argparse.Namespace) -> Any:
    """
    Summary
    Run a command handler that prints JSON and decode the result.

    Inputs
    A `desktopctl` command function and argument namespace.

    Outputs
    The decoded JSON payload.

    Side effects
    Temporarily redirects stdout.

    Error handling
    Raises `DesktopDaemonError` when the command output is not valid JSON.

    Ties to other methods
    Used by host and session endpoints that proxy JSON-producing CLI handlers.

    Why this exists
    The worker daemon must preserve typed payloads when reusing JSON-emitting local commands.
    """

    raw = _capture_text_output(func, args)
    try:
        return json.loads(raw)
    except json.JSONDecodeError as exc:
        raise DesktopDaemonError(
            f"[desktopd.py::_capture_json_output] Failed to decode JSON output: {exc}"
        ) from exc


def _run_no_output(func: Any, args: argparse.Namespace) -> None:
    """
    Summary
    Run a command handler and discard any stdout it emits.

    Inputs
    A `desktopctl` command function and argument namespace.

    Outputs
    None.

    Side effects
    May mutate desktop state through the underlying command.

    Error handling
    Propagates command failures to the caller.

    Ties to other methods
    Used by mutation endpoints such as click, drag, and typing.

    Why this exists
    Most input actions only need success or failure semantics, not textual output.
    """

    _capture_text_output(func, args)


class DesktopRequestHandler(BaseHTTPRequestHandler):
    """
    Summary
    Serve host and session-scoped worker daemon endpoints over HTTP.

    Inputs
    Standard HTTP requests handled by `BaseHTTPRequestHandler`.

    Outputs
    JSON responses and frame image responses.

    Side effects
    Reads and mutates desktop session state through the shared session registry.

    Error handling
    Returns contextual JSON error payloads with stable request-local context.

    Ties to other methods
    Uses `SessionRegistry` and the reused `desktopctl` action handlers.

    Why this exists
    A simple HTTP API lets agents and orchestrators drive isolated sessions without shelling into the host directly.
    """

    session_registry: SessionRegistry

    def log_message(self, format: str, *args: Any) -> None:
        return

    def do_GET(self) -> None:  # noqa: N802
        """
        Summary
        Route HTTP GET requests to host or session-scoped endpoints.

        Inputs
        The current request path and query string.

        Outputs
        JSON or PNG responses written to the client socket.

        Side effects
        May trigger an on-demand session refresh when requested through the frame endpoint.

        Error handling
        Converts request and backend failures into JSON error responses.

        Ties to other methods
        Delegates to `_handle_host_get` and `_handle_session_get`.

        Why this exists
        The daemon needs separate host-level and session-level read paths.
        """

        try:
            parsed = urlparse(self.path)
            session_id, tail = self._session_path(parsed.path)
            if session_id:
                self._handle_session_get(session_id, tail, parsed.query)
                return
            self._handle_host_get(parsed.path)
        except Exception as exc:  # noqa: BLE001
            self._json_response(HTTPStatus.BAD_REQUEST, {"error": str(exc)})

    def do_POST(self) -> None:  # noqa: N802
        """
        Summary
        Route HTTP POST requests to host or session-scoped mutation endpoints.

        Inputs
        The current request path and JSON body.

        Outputs
        JSON responses written to the client socket.

        Side effects
        Creates sessions, runs desktop input actions, and triggers OCR.

        Error handling
        Converts request and backend failures into JSON error responses.

        Ties to other methods
        Delegates to `_handle_host_post` and `_handle_session_post`.

        Why this exists
        Session lifecycle and input control are both mutable worker-daemon operations.
        """

        try:
            parsed = urlparse(self.path)
            payload = self._read_json_body()
            session_id, tail = self._session_path(parsed.path)
            if session_id:
                self._handle_session_post(session_id, tail, payload)
                return
            self._handle_host_post(parsed.path, payload)
        except Exception as exc:  # noqa: BLE001
            self._json_response(HTTPStatus.BAD_REQUEST, {"error": str(exc)})

    def do_DELETE(self) -> None:  # noqa: N802
        """
        Summary
        Route HTTP DELETE requests to session cleanup endpoints.

        Inputs
        The current request path.

        Outputs
        JSON responses written to the client socket.

        Side effects
        Deletes worker sessions and releases capacity.

        Error handling
        Converts request and backend failures into JSON error responses.

        Ties to other methods
        Delegates to `SessionRegistry.delete_session`.

        Why this exists
        Session lifecycle management requires explicit deletion for cleanup and capacity release.
        """

        try:
            parsed = urlparse(self.path)
            session_id, tail = self._session_path(parsed.path)
            if not session_id or tail:
                self._json_response(HTTPStatus.NOT_FOUND, {"error": "not_found"})
                return
            self._json_response(
                HTTPStatus.OK, self.session_registry.delete_session(session_id)
            )
        except Exception as exc:  # noqa: BLE001
            self._json_response(HTTPStatus.BAD_REQUEST, {"error": str(exc)})

    def _handle_host_get(self, path: str) -> None:
        """
        Summary
        Serve host-level GET endpoints.

        Inputs
        Host-level path.

        Outputs
        JSON responses written to the client socket.

        Side effects
        None.

        Error handling
        Raises `DesktopDaemonError` when the path is unknown.

        Ties to other methods
        Used by `do_GET`.

        Why this exists
        Host-level worker status should be separated from session-local state.
        """

        if path == "/health":
            self._json_response(HTTPStatus.OK, self.session_registry.host_status())
            return
        if path == "/windows":
            payload = _capture_json_output(desktop_windows_command, _namespace(app=None))
            self._json_response(HTTPStatus.OK, {"windows": payload})
            return
        if path == "/sessions":
            self._json_response(
                HTTPStatus.OK, {"sessions": self.session_registry.list_sessions()}
            )
            return
        raise DesktopDaemonError("[desktopd.py::_handle_host_get] Unknown GET endpoint.")

    def _handle_host_post(self, path: str, payload: dict[str, Any]) -> None:
        """
        Summary
        Serve host-level POST endpoints such as session creation.

        Inputs
        Host-level path and JSON body.

        Outputs
        JSON responses written to the client socket.

        Side effects
        Creates session leases.

        Error handling
        Raises `DesktopDaemonError` when the path is unknown or invalid.

        Ties to other methods
        Used by `do_POST`.

        Why this exists
        Session creation belongs at the host level because it allocates shared worker capacity.
        """

        if path != "/sessions":
            raise DesktopDaemonError("[desktopd.py::_handle_host_post] Unknown POST endpoint.")
        worker_class = self._required_str(payload, "worker_class")
        visual_mode = self._optional_str(payload, "visual_mode", "tiered_visual")
        target_app = payload.get("target_app")
        if target_app is not None and not isinstance(target_app, str):
            raise DesktopDaemonError("[desktopd.py::_handle_host_post] target_app must be a string or null.")
        metadata = payload.get("metadata") or {}
        if not isinstance(metadata, dict):
            raise DesktopDaemonError("[desktopd.py::_handle_host_post] metadata must be an object.")
        record = self.session_registry.create_session(
            worker_class=worker_class,
            visual_mode=visual_mode,
            target_app=target_app,
            metadata=metadata,
        )
        self._json_response(HTTPStatus.CREATED, record.to_dict())

    def _handle_session_get(self, session_id: str, tail: str, query_string: str) -> None:
        """
        Summary
        Serve session-scoped GET endpoints.

        Inputs
        Session id, trailing path, and query string.

        Outputs
        JSON or PNG responses written to the client socket.

        Side effects
        May trigger a fresh capture when `refresh=1` is present on the frame endpoint.

        Error handling
        Raises `DesktopDaemonError` when the path is invalid or the session does not support the requested action.

        Ties to other methods
        Used by `do_GET`.

        Why this exists
        Session-local frame, OCR, and metadata access should stay scoped to one isolated session.
        """

        if tail == "":
            self._json_response(HTTPStatus.OK, self.session_registry.get_session(session_id).to_dict())
            return
        if tail == "windows":
            payload = _capture_json_output(
                desktop_windows_command,
                _namespace(app=self.session_registry.get_session(session_id).target_app),
            )
            self._json_response(HTTPStatus.OK, {"windows": payload})
            return
        if tail == "frame":
            params = parse_qs(query_string)
            scope = params.get("scope", ["screen"])[0]
            if params.get("refresh", ["0"])[0] == "1":
                self.session_registry.refresh_session(session_id)
            frame_path = self.session_registry.session_frame_path(session_id, scope=scope)
            self._file_response(frame_path, "image/png")
            return
        raise DesktopDaemonError("[desktopd.py::_handle_session_get] Unknown session GET endpoint.")

    def _handle_session_post(
        self,
        session_id: str,
        tail: str,
        payload: dict[str, Any],
    ) -> None:
        """
        Summary
        Serve session-scoped POST endpoints for targeting, input, OCR, and text actions.

        Inputs
        Session id, trailing path, and JSON body.

        Outputs
        JSON responses written to the client socket.

        Side effects
        Mutates session targets, performs input actions, and executes OCR queries.

        Error handling
        Raises `DesktopDaemonError` when the request is invalid or unsupported.

        Ties to other methods
        Used by `do_POST`.

        Why this exists
        Session-local mutation endpoints are the core of isolated GUI automation.
        """

        if tail == "target":
            app_name = payload.get("target_app", payload.get("app"))
            if app_name is not None and not isinstance(app_name, str):
                raise DesktopDaemonError("[desktopd.py::_handle_session_post] target_app must be a string or null.")
            record = self.session_registry.update_target(session_id, app_name)
            self._json_response(HTTPStatus.OK, record.to_dict())
            return
        if tail == "refresh":
            record = self.session_registry.refresh_session(session_id)
            self._json_response(HTTPStatus.OK, record.to_dict())
            return
        if tail == "activate":
            app_name = self._optional_str(
                payload,
                "app",
                self.session_registry.get_session(session_id).target_app or "",
            )
            if not app_name:
                raise DesktopDaemonError("[desktopd.py::_handle_session_post] Session has no target app to activate.")
            _run_no_output(desktop_activate_command, _namespace(app=app_name))
            self._json_response(HTTPStatus.OK, {"ok": True, "session_id": session_id, "app": app_name})
            return
        if tail in {"click", "double-click", "right-click", "move"}:
            x, y = self._resolved_coordinates(session_id, payload)
            action_map = {
                "click": desktop_click_command,
                "double-click": desktop_double_click_command,
                "right-click": desktop_right_click_command,
                "move": desktop_move_command,
            }
            _run_no_output(action_map[tail], _namespace(x=x, y=y))
            self._json_response(
                HTTPStatus.OK,
                {
                    "ok": True,
                    "session_id": session_id,
                    "coordinate_space": payload.get("coordinate_space", "session"),
                    "screen": {"x": x, "y": y},
                },
            )
            return
        if tail == "drag":
            x1, y1 = self._resolved_coordinates(
                session_id,
                {"x": self._required_int(payload, "x1"), "y": self._required_int(payload, "y1"), "coordinate_space": payload.get("coordinate_space", "session")},
            )
            x2, y2 = self._resolved_coordinates(
                session_id,
                {"x": self._required_int(payload, "x2"), "y": self._required_int(payload, "y2"), "coordinate_space": payload.get("coordinate_space", "session")},
            )
            _run_no_output(desktop_drag_command, _namespace(x1=x1, y1=y1, x2=x2, y2=y2))
            self._json_response(
                HTTPStatus.OK,
                {"ok": True, "session_id": session_id, "screen": {"x1": x1, "y1": y1, "x2": x2, "y2": y2}},
            )
            return
        if tail == "type":
            _run_no_output(desktop_type_command, _namespace(text=self._required_str(payload, "text")))
            self._json_response(HTTPStatus.OK, {"ok": True, "session_id": session_id})
            return
        if tail == "key":
            _run_no_output(desktop_key_command, _namespace(key=self._required_str(payload, "key")))
            self._json_response(HTTPStatus.OK, {"ok": True, "session_id": session_id})
            return
        if tail == "cursor":
            cursor_value = _capture_text_output(desktop_cursor_command, _namespace()).strip()
            self._json_response(HTTPStatus.OK, {"cursor": cursor_value, "session_id": session_id})
            return
        if tail == "color":
            x, y = self._resolved_coordinates(session_id, payload)
            rgb = _capture_text_output(desktop_color_command, _namespace(x=x, y=y)).strip()
            self._json_response(HTTPStatus.OK, {"rgb": rgb, "screen": {"x": x, "y": y}, "session_id": session_id})
            return
        if tail == "ax-buttons":
            record = self.session_registry.get_session(session_id)
            app_name = self._optional_str(payload, "app", record.target_app or "")
            if not app_name:
                raise DesktopDaemonError("[desktopd.py::_handle_session_post] Session has no target app for accessibility inspection.")
            buttons = _capture_json_output(
                desktop_ax_buttons_command,
                _namespace(app=app_name, window=int(payload.get("window", 1))),
            )
            self._json_response(HTTPStatus.OK, {"buttons": buttons, "session_id": session_id})
            return
        if tail == "ocr":
            payload = self._structured_ocr(session_id, payload)
            self._json_response(HTTPStatus.OK, payload)
            return
        if tail == "find-text":
            matches = self._find_text(session_id, payload)
            self._json_response(HTTPStatus.OK, matches)
            return
        if tail == "click-text":
            response = self._click_text(session_id, payload)
            self._json_response(HTTPStatus.OK, response)
            return
        if tail == "wait-for-text":
            response = self._wait_for_text(session_id, payload)
            self._json_response(HTTPStatus.OK, response)
            return
        raise DesktopDaemonError("[desktopd.py::_handle_session_post] Unknown session POST endpoint.")

    def _structured_ocr(self, session_id: str, payload: dict[str, Any]) -> dict[str, Any]:
        """
        Summary
        Run structured OCR for a session frame.

        Inputs
        Session id and OCR request payload.

        Outputs
        A structured OCR payload.

        Side effects
        Reads the current session frame and spawns Tesseract.

        Error handling
        Raises `DesktopDaemonError` when OCR arguments are invalid or OCR execution fails.

        Ties to other methods
        Used by the session OCR and text-targeted endpoints.

        Why this exists
        Text-based automation needs structured OCR output with boxes and confidence.
        """

        image = payload.get("image")
        if image is not None and not isinstance(image, str):
            raise DesktopDaemonError("[desktopd.py::_structured_ocr] image must be a string when provided.")
        mode = self._optional_str(payload, "mode", "full_frame")
        scope = self._optional_str(payload, "scope", "app")
        image_path = Path(image) if image else self.session_registry.session_frame_path(session_id, scope=scope)
        args = _namespace(
            image=str(image_path),
            json=True,
            mode=mode,
            x=payload.get("x"),
            y=payload.get("y"),
            width=payload.get("width"),
            height=payload.get("height"),
            around_x=payload.get("around_x"),
            around_y=payload.get("around_y"),
            radius=int(payload.get("radius", 120)),
        )
        return _capture_json_output(desktop_ocr_command, args)

    def _find_text(self, session_id: str, payload: dict[str, Any]) -> dict[str, Any]:
        """
        Summary
        Find OCR text matches within the current session frame.

        Inputs
        Session id and text matching payload.

        Outputs
        A JSON object containing the OCR payload and matching boxes.

        Side effects
        Runs OCR against the session frame.

        Error handling
        Raises `DesktopDaemonError` when the query is missing.

        Ties to other methods
        Used by the `find-text`, `click-text`, and `wait-for-text` endpoints.

        Why this exists
        OCR text search should be reusable across all text-targeted automation operations.
        """

        query = self._required_str(payload, "text")
        ocr_payload = self._structured_ocr(session_id, payload)
        matches = find_text_matches(
            ocr_payload,
            query=query,
            case_sensitive=bool(payload.get("case_sensitive", False)),
            exact=bool(payload.get("exact", False)),
        )
        return {
            "session_id": session_id,
            "query": query,
            "match_count": len(matches),
            "matches": matches,
            "ocr": ocr_payload,
        }

    def _click_text(self, session_id: str, payload: dict[str, Any]) -> dict[str, Any]:
        """
        Summary
        Find OCR text and click the first matching result in the session coordinate space.

        Inputs
        Session id and text matching payload.

        Outputs
        A JSON object describing the clicked match and translated screen coordinates.

        Side effects
        Runs OCR and performs a click action.

        Error handling
        Raises `DesktopDaemonError` when no text match is found.

        Ties to other methods
        Builds on `_find_text` and the session click action.

        Why this exists
        Text-driven clicking is the safest fallback when accessibility is weak.
        """

        matches_payload = self._find_text(session_id, payload)
        matches = matches_payload["matches"]
        if not matches:
            raise DesktopDaemonError("[desktopd.py::_click_text] No matching text was found.")
        match = matches[0]
        center_x = match["left"] + round(match["width"] / 2)
        center_y = match["top"] + round(match["height"] / 2)
        screen_x, screen_y = self.session_registry.translate_session_coordinates(
            session_id,
            center_x,
            center_y,
            coordinate_space="session",
        )
        _run_no_output(desktop_click_command, _namespace(x=screen_x, y=screen_y))
        return {
            "ok": True,
            "session_id": session_id,
            "match": match,
            "screen": {"x": screen_x, "y": screen_y},
            "ocr_frame_id": matches_payload["ocr"]["frame_id"],
        }

    def _wait_for_text(self, session_id: str, payload: dict[str, Any]) -> dict[str, Any]:
        """
        Summary
        Wait until OCR text appears in the session frame or a timeout elapses.

        Inputs
        Session id and text wait payload.

        Outputs
        A JSON object describing whether the text was observed before timeout.

        Side effects
        Repeatedly refreshes the session frame and runs OCR during the wait loop.

        Error handling
        Raises `DesktopDaemonError` when the query is missing.

        Ties to other methods
        Builds on `_find_text` and `SessionRegistry.refresh_session`.

        Why this exists
        GUI automation needs deterministic wait primitives before interacting with changing interfaces.
        """

        query = self._required_str(payload, "text")
        timeout_seconds = float(payload.get("timeout_seconds", 10.0))
        poll_seconds = float(payload.get("poll_seconds", 0.5))
        deadline = time.time() + timeout_seconds
        while True:
            self.session_registry.refresh_session(session_id)
            matches_payload = self._find_text(session_id, payload)
            if matches_payload["match_count"] > 0:
                return {
                    "ok": True,
                    "session_id": session_id,
                    "query": query,
                    "matches": matches_payload["matches"],
                    "ocr": matches_payload["ocr"],
                }
            if time.time() >= deadline:
                return {
                    "ok": False,
                    "session_id": session_id,
                    "query": query,
                    "matches": [],
                    "timeout_seconds": timeout_seconds,
                }
            time.sleep(max(0.05, poll_seconds))

    def _resolved_coordinates(self, session_id: str, payload: dict[str, Any]) -> tuple[int, int]:
        """
        Summary
        Resolve request coordinates into absolute screen coordinates.

        Inputs
        Session id and request payload containing x, y, and optional coordinate space.

        Outputs
        A tuple of absolute screen coordinates.

        Side effects
        Reads current session frame and window metadata when session-relative coordinates are used.

        Error handling
        Raises `DesktopDaemonError` when x or y are missing.

        Ties to other methods
        Used by click, move, drag, and color endpoints.

        Why this exists
        Session-relative coordinates are the default for isolated OCR-driven actions.
        """

        x = self._required_int(payload, "x")
        y = self._required_int(payload, "y")
        coordinate_space = self._optional_str(payload, "coordinate_space", "session")
        try:
            return self.session_registry.translate_session_coordinates(
                session_id, x, y, coordinate_space=coordinate_space
            )
        except GridError as exc:
            raise DesktopDaemonError(str(exc)) from exc

    def _session_path(self, path: str) -> tuple[str | None, str]:
        """
        Summary
        Split a request path into session id and trailing endpoint path.

        Inputs
        Raw request path.

        Outputs
        A tuple of `(session_id, tail)` where `session_id` is `None` for host-level paths.

        Side effects
        None.

        Error handling
        None.

        Ties to other methods
        Used by `do_GET`, `do_POST`, and `do_DELETE`.

        Why this exists
        Session-scoped routing is easier when the path is normalized once.
        """

        if not path.startswith("/sessions/"):
            return None, ""
        parts = [part for part in path.split("/") if part]
        if len(parts) < 2:
            return None, ""
        session_id = parts[1]
        tail = "/".join(parts[2:])
        return session_id, tail

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
        Raises `DesktopDaemonError` when the body is invalid JSON or not an object.

        Ties to other methods
        Used by all POST endpoints.

        Why this exists
        The worker daemon uses a JSON-only API for deterministic automation.
        """

        length = int(self.headers.get("Content-Length", "0"))
        raw = self.rfile.read(length) if length > 0 else b"{}"
        try:
            payload = json.loads(raw.decode("utf-8"))
        except json.JSONDecodeError as exc:
            raise DesktopDaemonError(
                f"[desktopd.py::_read_json_body] Invalid JSON body: {exc}"
            ) from exc
        if not isinstance(payload, dict):
            raise DesktopDaemonError("[desktopd.py::_read_json_body] JSON body must be an object.")
        return payload

    def _required_str(self, payload: dict[str, Any], key: str) -> str:
        """
        Summary
        Read a required non-empty string field from a JSON payload.

        Inputs
        Payload dictionary and field name.

        Outputs
        The validated string value.

        Side effects
        None.

        Error handling
        Raises `DesktopDaemonError` when the field is missing or invalid.

        Ties to other methods
        Used across session lifecycle and action endpoints.

        Why this exists
        Explicit input validation keeps worker-daemon failures actionable.
        """

        value = payload.get(key)
        if not isinstance(value, str) or not value.strip():
            raise DesktopDaemonError(
                f"[desktopd.py::_required_str] Field {key!r} must be a non-empty string."
            )
        return value

    def _optional_str(self, payload: dict[str, Any], key: str, default: str) -> str:
        """
        Summary
        Read an optional string field with a default value.

        Inputs
        Payload dictionary, field name, and default string.

        Outputs
        The provided value or default.

        Side effects
        None.

        Error handling
        Raises `DesktopDaemonError` when a provided value is not a string.

        Ties to other methods
        Used by OCR and session action endpoints.

        Why this exists
        Optional string fields still need strict typing in the worker API.
        """

        value = payload.get(key, default)
        if not isinstance(value, str):
            raise DesktopDaemonError(
                f"[desktopd.py::_optional_str] Field {key!r} must be a string."
            )
        return value

    def _required_int(self, payload: dict[str, Any], key: str) -> int:
        """
        Summary
        Read a required integer field from a JSON payload.

        Inputs
        Payload dictionary and field name.

        Outputs
        The validated integer value.

        Side effects
        None.

        Error handling
        Raises `DesktopDaemonError` when the field is missing or not an integer.

        Ties to other methods
        Used by coordinate-based action endpoints.

        Why this exists
        Desktop input should reject malformed coordinates clearly.
        """

        value = payload.get(key)
        if not isinstance(value, int):
            raise DesktopDaemonError(
                f"[desktopd.py::_required_int] Field {key!r} must be an integer."
            )
        return value

    def _json_response(self, status: HTTPStatus, payload: Any) -> None:
        """
        Summary
        Write a JSON response to the client.

        Inputs
        HTTP status code and JSON-serializable payload.

        Outputs
        None.

        Side effects
        Writes bytes to the client socket.

        Error handling
        Assumes the payload is JSON-serializable.

        Ties to other methods
        Used by all JSON endpoints.

        Why this exists
        Consistent JSON responses make the worker daemon easy to integrate with higher-level tooling.
        """

        encoded = json.dumps(payload).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(encoded)))
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        self.wfile.write(encoded)

    def _file_response(self, path: Path, content_type: str) -> None:
        """
        Summary
        Stream a file response to the client.

        Inputs
        File path and content type.

        Outputs
        None.

        Side effects
        Reads the file from disk and writes it to the client socket.

        Error handling
        Raises `DesktopDaemonError` when the file is missing.

        Ties to other methods
        Used by the session frame endpoint.

        Why this exists
        Session frame images should be retrievable directly over HTTP without another encoding layer.
        """

        if not path.exists():
            raise DesktopDaemonError(
                f"[desktopd.py::_file_response] Frame file not found: {path}"
            )
        data = path.read_bytes()
        self.send_response(HTTPStatus.OK)
        self.send_header("Content-Type", content_type)
        self.send_header("Content-Length", str(len(data)))
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        self.wfile.write(data)


def build_parser() -> argparse.ArgumentParser:
    """
    Summary
    Construct the worker daemon CLI argument parser.

    Inputs
    None.

    Outputs
    A configured `ArgumentParser`.

    Side effects
    None.

    Error handling
    None.

    Ties to other methods
    Used by `main`.

    Why this exists
    The worker daemon should be configurable without source edits.
    """

    parser = argparse.ArgumentParser(
        prog="desktopd",
        description="Persistent HTTP worker daemon for session-scoped GUI automation and structured OCR.",
    )
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=8765)
    parser.add_argument("--interval", type=float, default=1.0)
    return parser


def main() -> int:
    """
    Summary
    Start the session-based worker daemon and serve until interrupted.

    Inputs
    Process arguments from `sys.argv`.

    Outputs
    Process exit status.

    Side effects
    Starts an HTTP server with a shared session registry.

    Error handling
    Prints startup metadata and exits cleanly on keyboard interrupt.

    Ties to other methods
    Calls `build_parser`, `SessionRegistry`, and `ThreadingHTTPServer`.

    Why this exists
    A persistent worker daemon is the cleanest way to expose isolated GUI sessions to the orchestrator.
    """

    parser = build_parser()
    args = parser.parse_args()
    registry = SessionRegistry(interval_seconds=args.interval)
    handler_cls = type(
        "DesktopHandler",
        (DesktopRequestHandler,),
        {"session_registry": registry},
    )
    server = ThreadingHTTPServer((args.host, args.port), handler_cls)
    print(
        json.dumps(
            {
                "host": args.host,
                "port": args.port,
                "sessions_url": f"http://{args.host}:{args.port}/sessions",
                "health_url": f"http://{args.host}:{args.port}/health",
                "workers_supported": registry.host_status()["supported"],
            }
        ),
        flush=True,
    )
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
