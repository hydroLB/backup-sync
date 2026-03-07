from __future__ import annotations

from pathlib import Path
from typing import Any

from grid_common import GridError, png_size, sha256_file


def _to_int(value: str) -> int:
    """
    Summary
    Convert a TSV field into an integer with a safe default.

    Inputs
    `value` as the raw TSV cell text.

    Outputs
    The parsed integer or zero.

    Side effects
    None.

    Error handling
    Returns zero when the value cannot be parsed.

    Ties to other methods
    Used by TSV row parsing.

    Why this exists
    Tesseract TSV fields are stringly typed and may contain empty or invalid values.
    """

    try:
        return int(value)
    except (TypeError, ValueError):
        return 0


def _to_float(value: str) -> float:
    """
    Summary
    Convert a TSV field into a float with a safe default.

    Inputs
    `value` as the raw TSV cell text.

    Outputs
    The parsed float or zero.

    Side effects
    None.

    Error handling
    Returns zero when the value cannot be parsed.

    Ties to other methods
    Used by TSV row parsing.

    Why this exists
    OCR confidence values should not break the parser when they are empty or malformed.
    """

    try:
        return float(value)
    except (TypeError, ValueError):
        return 0.0


def parse_tesseract_tsv(tsv_text: str) -> list[dict[str, Any]]:
    """
    Summary
    Parse Tesseract TSV output into normalized word entries.

    Inputs
    `tsv_text` as the raw TSV produced by Tesseract.

    Outputs
    A list of OCR word dictionaries with bounds and confidence.

    Side effects
    None.

    Error handling
    Raises `GridError` when the TSV header is missing or malformed.

    Ties to other methods
    Used by structured OCR payload generation.

    Why this exists
    The grid needs bounding boxes and confidence, not only raw OCR text.
    """

    lines = [line for line in tsv_text.splitlines() if line.strip()]
    if not lines:
        raise GridError("[grid_ocr.py::parse_tesseract_tsv] OCR TSV output was empty.")
    header = lines[0].split("\t")
    required = {"level", "page_num", "block_num", "par_num", "line_num", "word_num", "left", "top", "width", "height", "conf", "text"}
    if not required.issubset(set(header)):
        raise GridError("[grid_ocr.py::parse_tesseract_tsv] OCR TSV header was missing expected columns.")

    rows: list[dict[str, Any]] = []
    for line in lines[1:]:
        parts = line.split("\t")
        if len(parts) < len(header):
            parts += [""] * (len(header) - len(parts))
        row = dict(zip(header, parts))
        text = row.get("text", "").strip()
        if not text:
            continue
        width = _to_int(row.get("width", "0"))
        height = _to_int(row.get("height", "0"))
        if width <= 0 or height <= 0:
            continue
        left = _to_int(row.get("left", "0"))
        top = _to_int(row.get("top", "0"))
        rows.append(
            {
                "text": text,
                "confidence": _to_float(row.get("conf", "0")),
                "left": left,
                "top": top,
                "width": width,
                "height": height,
                "right": left + width,
                "bottom": top + height,
                "page_num": _to_int(row.get("page_num", "0")),
                "block_num": _to_int(row.get("block_num", "0")),
                "par_num": _to_int(row.get("par_num", "0")),
                "line_num": _to_int(row.get("line_num", "0")),
                "word_num": _to_int(row.get("word_num", "0")),
            }
        )
    return rows


def _group_lines(words: list[dict[str, Any]]) -> list[dict[str, Any]]:
    """
    Summary
    Group OCR words into line-level records.

    Inputs
    `words` as normalized OCR word entries.

    Outputs
    A list of OCR line dictionaries.

    Side effects
    None.

    Error handling
    None.

    Ties to other methods
    Used by `build_ocr_payload`.

    Why this exists
    Text matching and human-readable artifacts benefit from both word-level and line-level views.
    """

    grouped: dict[tuple[int, int, int, int], list[dict[str, Any]]] = {}
    for word in words:
        key = (word["page_num"], word["block_num"], word["par_num"], word["line_num"])
        grouped.setdefault(key, []).append(word)

    lines: list[dict[str, Any]] = []
    for key, line_words in grouped.items():
        ordered = sorted(line_words, key=lambda entry: (entry["left"], entry["word_num"]))
        text = " ".join(word["text"] for word in ordered)
        lines.append(
            {
                "page_num": key[0],
                "block_num": key[1],
                "par_num": key[2],
                "line_num": key[3],
                "text": text,
                "confidence": round(
                    sum(word["confidence"] for word in ordered) / max(len(ordered), 1), 2
                ),
                "left": min(word["left"] for word in ordered),
                "top": min(word["top"] for word in ordered),
                "right": max(word["right"] for word in ordered),
                "bottom": max(word["bottom"] for word in ordered),
                "width": max(word["right"] for word in ordered) - min(word["left"] for word in ordered),
                "height": max(word["bottom"] for word in ordered) - min(word["top"] for word in ordered),
                "words": ordered,
            }
        )
    return sorted(lines, key=lambda entry: (entry["top"], entry["left"]))


def _region_from_args(
    words: list[dict[str, Any]],
    mode: str,
    region: dict[str, int] | None,
    around: dict[str, int] | None,
) -> tuple[dict[str, int], list[dict[str, Any]]]:
    """
    Summary
    Resolve the OCR source region and filter the OCR words to that region.

    Inputs
    Word list, OCR mode, optional explicit region, and optional around-point parameters.

    Outputs
    The resolved source region and filtered word list.

    Side effects
    None.

    Error handling
    Raises `GridError` when an around-point request is missing coordinates.

    Ties to other methods
    Used by `build_ocr_payload`.

    Why this exists
    Region-specific OCR must stay deterministic even though Tesseract was run against the whole frame.
    """

    if not words:
        base_region = region or {"left": 0, "top": 0, "width": 0, "height": 0}
        return base_region, []

    if mode == "around_point":
        if not around or "x" not in around or "y" not in around:
            raise GridError("[grid_ocr.py::_region_from_args] around_point mode requires x and y.")
        radius = around.get("radius", 120)
        region = {
            "left": max(0, around["x"] - radius),
            "top": max(0, around["y"] - radius),
            "width": radius * 2,
            "height": radius * 2,
        }

    if region is None:
        left = min(word["left"] for word in words)
        top = min(word["top"] for word in words)
        right = max(word["right"] for word in words)
        bottom = max(word["bottom"] for word in words)
        region = {
            "left": left,
            "top": top,
            "width": max(0, right - left),
            "height": max(0, bottom - top),
        }

    region_left = region["left"]
    region_top = region["top"]
    region_right = region_left + region["width"]
    region_bottom = region_top + region["height"]

    filtered = [
        word
        for word in words
        if word["right"] >= region_left
        and word["left"] <= region_right
        and word["bottom"] >= region_top
        and word["top"] <= region_bottom
    ]
    return region, filtered


def build_ocr_payload(
    image_path: Path,
    tsv_text: str,
    mode: str = "full_frame",
    region: dict[str, int] | None = None,
    around: dict[str, int] | None = None,
) -> dict[str, Any]:
    """
    Summary
    Build a structured OCR payload for a frame image.

    Inputs
    `image_path` as the OCR source image, `tsv_text` as Tesseract TSV output, and optional region controls.

    Outputs
    A JSON-serializable OCR payload including text, lines, words, boxes, confidence, and source region.

    Side effects
    Reads the PNG header and file contents to compute image metadata and frame id.

    Error handling
    Raises `GridError` when the image metadata or TSV cannot be processed.

    Ties to other methods
    Used by the worker daemon OCR endpoint and text-based action helpers.

    Why this exists
    Downstream automation needs structured OCR with bounding boxes, not just flat text.
    """

    width, height = png_size(image_path)
    if region is None and around is None and mode in {"full_frame", "active_window"}:
        region = {"left": 0, "top": 0, "width": width, "height": height}
    words = parse_tesseract_tsv(tsv_text)
    source_region, filtered_words = _region_from_args(words, mode, region, around)
    lines = _group_lines(filtered_words)
    text = "\n".join(line["text"] for line in lines).strip()
    confidence = round(
        sum(word["confidence"] for word in filtered_words) / max(len(filtered_words), 1), 2
    ) if filtered_words else 0.0
    return {
        "frame_id": sha256_file(image_path),
        "image_path": str(image_path),
        "image_size": {"width": width, "height": height},
        "mode": mode,
        "source_region": source_region,
        "text": text,
        "confidence": confidence,
        "word_count": len(filtered_words),
        "line_count": len(lines),
        "boxes": [
            {
                "text": word["text"],
                "confidence": word["confidence"],
                "left": word["left"],
                "top": word["top"],
                "width": word["width"],
                "height": word["height"],
                "right": word["right"],
                "bottom": word["bottom"],
            }
            for word in filtered_words
        ],
        "words": filtered_words,
        "lines": lines,
        "regions": [source_region],
    }


def find_text_matches(
    payload: dict[str, Any],
    query: str,
    case_sensitive: bool = False,
    exact: bool = False,
) -> list[dict[str, Any]]:
    """
    Summary
    Find OCR line and word matches inside a structured OCR payload.

    Inputs
    OCR payload, query string, and matching flags.

    Outputs
    A list of matching OCR boxes.

    Side effects
    None.

    Error handling
    None.

    Ties to other methods
    Used by `find_text`, `click_text`, and `wait_for_text`.

    Why this exists
    Text-based automation should work across poorly accessible GUIs when OCR is available.
    """

    haystack_query = query if case_sensitive else query.lower()
    matches: list[dict[str, Any]] = []
    for word in payload.get("words", []):
        word_text = word.get("text", "")
        normalized = word_text if case_sensitive else word_text.lower()
        if exact:
            matched = normalized == haystack_query
        else:
            matched = haystack_query in normalized
        if matched:
            matches.append(word)
    return matches
