#!/usr/bin/env python3

from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any

from grid_common import GRID_ROOT, GridError, ensure_directory
from grid_ocr import build_ocr_payload


class DesktopCtlError(RuntimeError):
    """Raised when a desktop bridge command cannot complete."""


INSTALL_ROOT = Path.home() / ".codex-desktop-bridge"
WINDOW_HELPER_PATH = INSTALL_ROOT / "window-info"
DEFAULT_CAPTURE_DIR = GRID_ROOT / "captures"


def _resolve_tool(name: str, fallback: str | None = None) -> str:
    """
    Summary
    Resolve an executable path in shell and launchd environments.

    Inputs
    `name` as the executable name and `fallback` as an optional absolute fallback path.

    Outputs
    A runnable executable path.

    Side effects
    Reads the current PATH.

    Error handling
    Raises `DesktopCtlError` when the executable cannot be found.

    Ties to other methods
    Used by click, OCR, and input commands that depend on Homebrew-installed tools.

    Why this exists
    Launch agents do not inherit the interactive shell PATH, so desktop tooling must resolve its own binaries.
    """

    resolved = shutil.which(name)
    if resolved:
        return resolved
    if fallback and Path(fallback).exists():
        return fallback
    raise DesktopCtlError(
        f"[desktopctl.py::_resolve_tool] Required executable {name!r} was not found on PATH."
    )


def _cliclick_bin() -> str:
    """Resolve the optional click helper only when an input command needs it."""

    return _resolve_tool("cliclick", "/opt/homebrew/bin/cliclick")


def _tesseract_bin() -> str:
    """Resolve the optional OCR helper only when an OCR command needs it."""

    return _resolve_tool("tesseract", "/opt/homebrew/bin/tesseract")


def _ensure_capture_dir() -> Path:
    """
    Summary
    Create the default capture directory used by screenshot commands.

    Inputs
    None.

    Outputs
    A writable `Path` for desktop bridge captures.

    Side effects
    Creates directories under the current user's home directory when needed.

    Error handling
    Raises `DesktopCtlError` when the directory cannot be created.

    Ties to other methods
    Used by `screenshot_command`.

    Why this exists
    Screenshot commands need a stable default output location that survives across sessions.
    """

    try:
        return ensure_directory(DEFAULT_CAPTURE_DIR)
    except GridError as exc:
        raise DesktopCtlError(
            f"[desktopctl.py::_ensure_capture_dir] Failed to create capture directory: {exc}"
        ) from exc


def _run_checked(command: list[str], capture_output: bool = True) -> subprocess.CompletedProcess[str]:
    """
    Summary
    Execute a subprocess command and raise a contextual error on failure.

    Inputs
    `command` as the argv list and `capture_output` to control stdout capture.

    Outputs
    A completed subprocess result.

    Side effects
    Spawns child processes.

    Error handling
    Raises `DesktopCtlError` with stderr or stdout context when the command exits non-zero.

    Ties to other methods
    Used by every command handler that shells out to native tools.

    Why this exists
    The bridge should fail with clear, local command context instead of raw subprocess errors.
    """

    result = subprocess.run(
        command,
        check=False,
        text=True,
        capture_output=capture_output,
    )
    if result.returncode != 0:
        detail = (result.stderr or result.stdout or "").strip()
        raise DesktopCtlError(
            f"[desktopctl.py::_run_checked] Command failed ({' '.join(command)}): {detail}"
        )
    return result


def _window_helper_command() -> list[str]:
    """
    Summary
    Resolve the installed native window-helper command.

    Inputs
    None.

    Outputs
    A command argv list pointing at the compiled Swift helper.

    Side effects
    None.

    Error handling
    Raises `DesktopCtlError` when the helper binary has not been installed yet.

    Ties to other methods
    Used by `_list_windows`.

    Why this exists
    Window inspection relies on Quartz metadata that is exposed through the companion native helper.
    """

    if not WINDOW_HELPER_PATH.exists():
        raise DesktopCtlError(
            "[desktopctl.py::_window_helper_command] Window helper is not installed. Run scripts/install_desktopctl.sh first."
        )
    return [str(WINDOW_HELPER_PATH)]


def _list_windows() -> list[dict[str, Any]]:
    """
    Summary
    Read the current visible window list from the native helper.

    Inputs
    None.

    Outputs
    A JSON-decoded list of window dictionaries.

    Side effects
    Executes the native helper binary.

    Error handling
    Raises `DesktopCtlError` when the helper output is missing or malformed.

    Ties to other methods
    Used by `windows_command`, `_resolve_window`, and `screenshot_command`.

    Why this exists
    The bridge needs structured window metadata for targeting and window-scoped screenshots.
    """

    result = _run_checked(_window_helper_command())
    try:
        payload = json.loads(result.stdout)
    except json.JSONDecodeError as exc:
        raise DesktopCtlError(
            f"[desktopctl.py::_list_windows] Failed to parse helper JSON: {exc}"
        ) from exc
    if not isinstance(payload, list):
        raise DesktopCtlError("[desktopctl.py::_list_windows] Helper returned non-list JSON.")
    return payload


def _resolve_window(app_name: str, title_contains: str | None) -> dict[str, Any]:
    """
    Summary
    Find the best matching visible window for an app and optional title filter.

    Inputs
    `app_name` as the owning application name and `title_contains` as an optional title substring.

    Outputs
    The matched window dictionary.

    Side effects
    Reads current window metadata from the native helper.

    Error handling
    Raises `DesktopCtlError` when no matching visible window exists.

    Ties to other methods
    Used by `screenshot_command`.

    Why this exists
    Window-scoped screenshots need deterministic matching instead of relying on arbitrary window order.
    """

    app_name_lower = app_name.lower()
    title_lower = title_contains.lower() if title_contains else None
    candidates = [
        window
        for window in _list_windows()
        if str(window.get("ownerName", "")).lower() == app_name_lower
        and int(window.get("width", 0)) > 0
        and int(window.get("height", 0)) > 0
        and int(window.get("layer", 0)) == 0
    ]
    if title_lower:
        candidates = [
            window
            for window in candidates
            if title_lower in str(window.get("windowName", "")).lower()
        ]
    if not candidates:
        detail = f"{app_name!r}"
        if title_contains:
            detail += f" title containing {title_contains!r}"
        raise DesktopCtlError(
            f"[desktopctl.py::_resolve_window] No visible window found for {detail}."
        )
    return candidates[0]


def _print_json(payload: Any) -> None:
    """
    Summary
    Serialize a Python value as formatted JSON to stdout.

    Inputs
    Any JSON-serializable value.

    Outputs
    None.

    Side effects
    Writes to stdout.

    Error handling
    Raises `DesktopCtlError` when JSON serialization fails.

    Ties to other methods
    Used by window and accessibility inspection commands.

    Why this exists
    Machine-readable command output should stay consistent across bridge actions.
    """

    try:
        print(json.dumps(payload, indent=2))
    except (TypeError, ValueError) as exc:
        raise DesktopCtlError(
            f"[desktopctl.py::_print_json] Failed to encode JSON output: {exc}"
        ) from exc


def windows_command(args: argparse.Namespace) -> int:
    """
    Summary
    Print the current visible window list as JSON.

    Inputs
    Parsed CLI args with an optional app filter.

    Outputs
    Process exit code.

    Side effects
    Reads global window metadata.

    Error handling
    Raises `DesktopCtlError` when helper execution fails.

    Ties to other methods
    Calls `_list_windows` and `_print_json`.

    Why this exists
    Window enumeration is the starting point for targeting screenshots and focused app interactions.
    """

    windows = _list_windows()
    if args.app:
        app_name_lower = args.app.lower()
        windows = [
            window
            for window in windows
            if str(window.get("ownerName", "")).lower() == app_name_lower
        ]
    _print_json(windows)
    return 0


def screenshot_command(args: argparse.Namespace) -> int:
    """
    Summary
    Capture the full screen or a specific app window to an image file.

    Inputs
    Parsed CLI args containing output path and optional app or title filters.

    Outputs
    Process exit code and the saved file path on stdout.

    Side effects
    Captures screen pixels to disk using macOS screen capture APIs.

    Error handling
    Raises `DesktopCtlError` when capture fails or no matching window exists.

    Ties to other methods
    Calls `_ensure_capture_dir`, `_resolve_window`, and `_run_checked`.

    Why this exists
    The bridge needs a simple way to inspect what a user would actually see on screen.
    """

    output_path = Path(args.output) if args.output else None
    if output_path is None:
        output_path = _ensure_capture_dir() / "desktop-shot.png"
    output_path = output_path.expanduser().resolve()
    output_path.parent.mkdir(parents=True, exist_ok=True)

    if args.app:
        window = _resolve_window(args.app, args.title)
        x = int(window["x"])
        y = int(window["y"])
        width = int(window["width"])
        height = int(window["height"])
        region = f"{x},{y},{width},{height}"
        _run_checked(["screencapture", "-x", "-R", region, str(output_path)], capture_output=True)
    else:
        _run_checked(["screencapture", "-x", str(output_path)], capture_output=True)

    print(str(output_path))
    return 0


def activate_command(args: argparse.Namespace) -> int:
    """
    Summary
    Bring the requested app to the foreground.

    Inputs
    Parsed CLI args containing the application name.

    Outputs
    Process exit code.

    Side effects
    Activates the target application through AppleScript.

    Error handling
    Raises `DesktopCtlError` when activation fails.

    Ties to other methods
    Independent foregrounding helper for click and typing workflows.

    Why this exists
    Mouse and keyboard events should be directed at the intended app before interaction begins.
    """

    _run_checked(["osascript", "-e", f'tell application "{args.app}" to activate'])
    return 0


def click_command(args: argparse.Namespace) -> int:
    """
    Summary
    Perform a single left click at screen coordinates.

    Inputs
    Parsed CLI args containing `x` and `y`.

    Outputs
    Process exit code.

    Side effects
    Moves the pointer and clicks through `cliclick`.

    Error handling
    Raises `DesktopCtlError` when the native click command fails.

    Ties to other methods
    Used by external workflows that want direct coordinate clicks.

    Why this exists
    Some apps expose poor accessibility metadata and still need raw pointer interaction.
    """

    _run_checked([_cliclick_bin(), f"c:{args.x},{args.y}"])
    return 0


def right_click_command(args: argparse.Namespace) -> int:
    """
    Summary
    Perform a single right click at screen coordinates.

    Inputs
    Parsed CLI args containing `x` and `y`.

    Outputs
    Process exit code.

    Side effects
    Moves the pointer and right-clicks through `cliclick`.

    Error handling
    Raises `DesktopCtlError` when the native click command fails.

    Ties to other methods
    Used by external workflows that need context-menu access.

    Why this exists
    Many desktop workflows rely on context menus rather than visible buttons.
    """

    _run_checked([_cliclick_bin(), f"rc:{args.x},{args.y}"])
    return 0


def double_click_command(args: argparse.Namespace) -> int:
    """
    Summary
    Perform a double-click at screen coordinates.

    Inputs
    Parsed CLI args containing `x` and `y`.

    Outputs
    Process exit code.

    Side effects
    Moves the pointer and double-clicks through `cliclick`.

    Error handling
    Raises `DesktopCtlError` when the native click command fails.

    Ties to other methods
    Used by external workflows that need standard double-click behavior.

    Why this exists
    File open and row activation flows often require a true double-click.
    """

    _run_checked([_cliclick_bin(), f"dc:{args.x},{args.y}"])
    return 0


def drag_command(args: argparse.Namespace) -> int:
    """
    Summary
    Drag from one screen coordinate to another.

    Inputs
    Parsed CLI args containing start and end coordinates.

    Outputs
    Process exit code.

    Side effects
    Presses, moves, and releases the pointer through `cliclick`.

    Error handling
    Raises `DesktopCtlError` when drag execution fails.

    Ties to other methods
    Used by scrollbars, selection, slider, and file-drop workflows.

    Why this exists
    Raw desktop control is incomplete without drag support.
    """

    _run_checked(
        [
            _cliclick_bin(),
            f"dd:{args.x1},{args.y1}",
            f"dm:{args.x2},{args.y2}",
            f"du:{args.x2},{args.y2}",
        ]
    )
    return 0


def move_command(args: argparse.Namespace) -> int:
    """
    Summary
    Move the pointer to screen coordinates without clicking.

    Inputs
    Parsed CLI args containing `x` and `y`.

    Outputs
    Process exit code.

    Side effects
    Moves the pointer through `cliclick`.

    Error handling
    Raises `DesktopCtlError` when the move command fails.

    Ties to other methods
    Useful before color sampling, screenshots, or drag workflows.

    Why this exists
    Pointer positioning should be available independently from click actions.
    """

    _run_checked([_cliclick_bin(), f"m:{args.x},{args.y}"])
    return 0


def cursor_command(args: argparse.Namespace) -> int:
    """
    Summary
    Print the current pointer position.

    Inputs
    Parsed CLI args, which are unused.

    Outputs
    Process exit code and the current pointer coordinates on stdout.

    Side effects
    Reads the current pointer position through `cliclick`.

    Error handling
    Raises `DesktopCtlError` when the native query fails.

    Ties to other methods
    Useful before click, drag, and OCR targeting workflows.

    Why this exists
    Coordinate-based automation needs a stable way to inspect the current pointer location.
    """

    result = _run_checked([_cliclick_bin(), "p"])
    print(result.stdout.strip())
    return 0


def type_command(args: argparse.Namespace) -> int:
    """
    Summary
    Type text into the frontmost application.

    Inputs
    Parsed CLI args containing the text payload.

    Outputs
    Process exit code.

    Side effects
    Sends keyboard text events through `cliclick`.

    Error handling
    Raises `DesktopCtlError` when text injection fails.

    Ties to other methods
    Often paired with `activate_command`.

    Why this exists
    Fast text entry is essential for realistic end-user GUI testing.
    """

    _run_checked([_cliclick_bin(), f"t:{args.text}"])
    return 0


def key_command(args: argparse.Namespace) -> int:
    """
    Summary
    Press a named special key in the frontmost application.

    Inputs
    Parsed CLI args containing the key name accepted by `cliclick`.

    Outputs
    Process exit code.

    Side effects
    Sends a special-key event through `cliclick`.

    Error handling
    Raises `DesktopCtlError` when key injection fails.

    Ties to other methods
    Used for navigation, submit, escape, and tab-based workflows.

    Why this exists
    GUI automation needs more than plain text typing.
    """

    _run_checked([_cliclick_bin(), f"kp:{args.key}"])
    return 0


def color_command(args: argparse.Namespace) -> int:
    """
    Summary
    Print the RGB color at a screen coordinate.

    Inputs
    Parsed CLI args containing `x` and `y`.

    Outputs
    Process exit code and the sampled RGB triplet on stdout.

    Side effects
    Samples the screen through `cliclick`.

    Error handling
    Raises `DesktopCtlError` when color sampling fails.

    Ties to other methods
    Useful for state checks when accessibility metadata is unavailable.

    Why this exists
    Poorly exposed apps can still be probed visually through pixel state.
    """

    result = _run_checked([_cliclick_bin(), f"cp:{args.x},{args.y}"])
    print(result.stdout.strip())
    return 0


def ax_buttons_command(args: argparse.Namespace) -> int:
    """
    Summary
    Print the accessible button names for the target app window.

    Inputs
    Parsed CLI args containing the app name and optional window index.

    Outputs
    Process exit code and a JSON array of button names.

    Side effects
    Reads the app accessibility tree through `System Events`.

    Error handling
    Raises `DesktopCtlError` when AppleScript execution fails.

    Ties to other methods
    Provides a quick accessibility smoke test before attempting scripted clicks.

    Why this exists
    Apps with good accessibility trees are dramatically easier to automate reliably.
    """

    script = (
        'tell application "System Events"\n'
        f'  tell process "{args.app}"\n'
        f'    set buttonNames to name of every button of window {args.window}\n'
        "  end tell\n"
        "end tell\n"
        "return buttonNames"
    )
    result = _run_checked(["osascript", "-e", script])
    names = [line.strip() for line in result.stdout.split(",") if line.strip()]
    _print_json(names)
    return 0


def ocr_command(args: argparse.Namespace) -> int:
    """
    Summary
    Run OCR over an image file and print the extracted text.

    Inputs
    Parsed CLI args containing the image path.

    Outputs
    Process exit code and OCR text on stdout.

    Side effects
    Spawns `tesseract`.

    Error handling
    Raises `DesktopCtlError` when OCR fails or the image path is missing.

    Ties to other methods
    Complements screenshot workflows when an app has weak accessibility metadata.

    Why this exists
    OCR keeps the bridge useful even when UI elements are not properly exposed to macOS accessibility.
    """

    image_path = Path(args.image).expanduser().resolve()
    if not image_path.exists():
        raise DesktopCtlError(
            f"[desktopctl.py::ocr_command] Image path does not exist: {image_path}"
        )
    if args.json:
        tsv_result = _run_checked([_tesseract_bin(), str(image_path), "stdout", "tsv"])
        region = None
        if args.x is not None and args.y is not None and args.width is not None and args.height is not None:
            region = {
                "left": args.x,
                "top": args.y,
                "width": args.width,
                "height": args.height,
            }
        around = None
        if args.around_x is not None and args.around_y is not None:
            around = {
                "x": args.around_x,
                "y": args.around_y,
                "radius": args.radius,
            }
        payload = build_ocr_payload(
            image_path=image_path,
            tsv_text=tsv_result.stdout,
            mode=args.mode,
            region=region,
            around=around,
        )
        _print_json(payload)
        return 0
    result = _run_checked([_tesseract_bin(), str(image_path), "stdout"])
    print(result.stdout.rstrip())
    return 0


def build_parser() -> argparse.ArgumentParser:
    """
    Summary
    Construct the command-line argument parser for desktop bridge actions.

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
    A single parser keeps the command surface discoverable and stable.
    """

    parser = argparse.ArgumentParser(
        prog="desktopctl",
        description="Lightweight macOS desktop automation bridge for screenshots, windows, mouse, keyboard, and OCR.",
    )
    subparsers = parser.add_subparsers(dest="command", required=True)

    windows_parser = subparsers.add_parser("windows", help="List visible windows as JSON.")
    windows_parser.add_argument("--app", help="Filter by exact app name.")
    windows_parser.set_defaults(func=windows_command)

    screenshot_parser = subparsers.add_parser("screenshot", help="Capture the full screen or an app window.")
    screenshot_parser.add_argument("--app", help="Exact app name to capture.")
    screenshot_parser.add_argument("--title", help="Optional window title substring filter.")
    screenshot_parser.add_argument("--output", help="Output image path.")
    screenshot_parser.set_defaults(func=screenshot_command)

    activate_parser = subparsers.add_parser("activate", help="Bring an app to the foreground.")
    activate_parser.add_argument("app", help="Exact app name.")
    activate_parser.set_defaults(func=activate_command)

    click_parser = subparsers.add_parser("click", help="Click screen coordinates.")
    click_parser.add_argument("x", type=int)
    click_parser.add_argument("y", type=int)
    click_parser.set_defaults(func=click_command)

    right_click_parser = subparsers.add_parser("right-click", help="Right-click screen coordinates.")
    right_click_parser.add_argument("x", type=int)
    right_click_parser.add_argument("y", type=int)
    right_click_parser.set_defaults(func=right_click_command)

    double_click_parser = subparsers.add_parser("double-click", help="Double-click screen coordinates.")
    double_click_parser.add_argument("x", type=int)
    double_click_parser.add_argument("y", type=int)
    double_click_parser.set_defaults(func=double_click_command)

    drag_parser = subparsers.add_parser("drag", help="Drag from one coordinate to another.")
    drag_parser.add_argument("x1", type=int)
    drag_parser.add_argument("y1", type=int)
    drag_parser.add_argument("x2", type=int)
    drag_parser.add_argument("y2", type=int)
    drag_parser.set_defaults(func=drag_command)

    move_parser = subparsers.add_parser("move", help="Move the pointer to screen coordinates.")
    move_parser.add_argument("x", type=int)
    move_parser.add_argument("y", type=int)
    move_parser.set_defaults(func=move_command)

    cursor_parser = subparsers.add_parser("cursor", help="Print the current pointer position.")
    cursor_parser.set_defaults(func=cursor_command)

    type_parser = subparsers.add_parser("type", help="Type text into the frontmost app.")
    type_parser.add_argument("text", help="Text to type.")
    type_parser.set_defaults(func=type_command)

    key_parser = subparsers.add_parser("key", help="Press a named special key.")
    key_parser.add_argument("key", help="cliclick special key name, for example return or tab.")
    key_parser.set_defaults(func=key_command)

    color_parser = subparsers.add_parser("color", help="Print the RGB color at screen coordinates.")
    color_parser.add_argument("x", type=int)
    color_parser.add_argument("y", type=int)
    color_parser.set_defaults(func=color_command)

    ax_parser = subparsers.add_parser("ax-buttons", help="List accessible button names for an app window.")
    ax_parser.add_argument("app", help="Exact app name.")
    ax_parser.add_argument("--window", type=int, default=1, help="1-based window index.")
    ax_parser.set_defaults(func=ax_buttons_command)

    ocr_parser = subparsers.add_parser("ocr", help="Run OCR on an image.")
    ocr_parser.add_argument("image", help="Image path.")
    ocr_parser.add_argument("--json", action="store_true", help="Emit structured OCR JSON instead of plain text.")
    ocr_parser.add_argument(
        "--mode",
        choices=["full_frame", "active_window", "region", "around_point"],
        default="full_frame",
        help="Structured OCR mode.",
    )
    ocr_parser.add_argument("--x", type=int, help="Region left coordinate.")
    ocr_parser.add_argument("--y", type=int, help="Region top coordinate.")
    ocr_parser.add_argument("--width", type=int, help="Region width.")
    ocr_parser.add_argument("--height", type=int, help="Region height.")
    ocr_parser.add_argument("--around-x", type=int, help="Center x for around-point OCR.")
    ocr_parser.add_argument("--around-y", type=int, help="Center y for around-point OCR.")
    ocr_parser.add_argument("--radius", type=int, default=120, help="Radius for around-point OCR.")
    ocr_parser.set_defaults(func=ocr_command)

    return parser


def main() -> int:
    """
    Summary
    Parse CLI arguments, dispatch the selected command, and print stable error output.

    Inputs
    Process arguments from `sys.argv`.

    Outputs
    Process exit status.

    Side effects
    Executes desktop automation commands and writes to stdout or stderr.

    Error handling
    Converts bridge and unexpected failures into explicit stderr messages with non-zero exit codes.

    Ties to other methods
    Calls `build_parser` and the selected subcommand handler.

    Why this exists
    The bridge needs a single stable entrypoint that can be used directly from terminal or future automation layers.
    """

    parser = build_parser()
    args = parser.parse_args()
    try:
        return int(args.func(args))
    except DesktopCtlError as exc:
        print(str(exc), file=sys.stderr)
        return 1
    except Exception as exc:  # noqa: BLE001
        print(f"[desktopctl.py::main] Unexpected failure: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
