#!/usr/bin/env python3
"""Validate repository-local links in Markdown documentation."""

from __future__ import annotations

import argparse
import re
import sys
from collections import defaultdict
from pathlib import Path
from urllib.parse import unquote


INLINE_LINK = re.compile(r"!?\[[^\]]*\]\(\s*(?P<target><[^>]+>|[^)\s]+)")
HTML_SOURCE = re.compile(r"<(?:img|source)\b[^>]*\b(?:src|href)=[\"'](?P<target>[^\"']+)", re.I)
REFERENCE_LINK = re.compile(r"^\s*\[[^\]]+\]:\s*(?P<target><[^>]+>|\S+)")
FENCE = re.compile(r"^\s*(`{3,}|~{3,})")
HEADING = re.compile(r"^\s{0,3}#{1,6}\s+(?P<text>.+?)\s*#*\s*$")
EXTERNAL = re.compile(r"^[a-zA-Z][a-zA-Z0-9+.-]*:")

DEFAULT_TOP_LEVEL = (
    "README.md",
    "CONTRIBUTING.md",
    "SECURITY.md",
    "CHANGELOG.md",
    ".github/pull_request_template.md",
)


def markdown_lines(path: Path):
    """Yield non-fenced Markdown lines with one-based line numbers."""

    in_fence = False
    fence_char = ""
    fence_len = 0
    for line_number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        match = FENCE.match(line)
        if match:
            marker = match.group(1)
            if not in_fence:
                in_fence = True
                fence_char = marker[0]
                fence_len = len(marker)
            elif marker[0] == fence_char and len(marker) >= fence_len:
                in_fence = False
            continue
        if not in_fence:
            yield line_number, line


def github_anchor(text: str) -> str:
    text = re.sub(r"<[^>]+>", "", text)
    text = re.sub(r"[`*_~]", "", text).strip().lower()
    text = re.sub(r"[^\w\- ]", "", text)
    return re.sub(r"\s+", "-", text)


def anchors_for(path: Path) -> set[str]:
    anchors: set[str] = set()
    occurrences: defaultdict[str, int] = defaultdict(int)
    for _, line in markdown_lines(path):
        match = HEADING.match(line)
        if not match:
            continue
        base = github_anchor(match.group("text"))
        index = occurrences[base]
        occurrences[base] += 1
        anchors.add(base if index == 0 else f"{base}-{index}")
    return anchors


def discover_files(root: Path, requested: list[str]) -> tuple[list[Path], list[str]]:
    errors: list[str] = []
    if requested:
        files = []
        for item in requested:
            candidate = (root / item).resolve()
            if not candidate.is_file():
                errors.append(f"{item}: documentation file does not exist")
            else:
                files.append(candidate)
        return sorted(set(files)), errors

    files = [(root / item).resolve() for item in DEFAULT_TOP_LEVEL if (root / item).is_file()]
    docs_root = root / "docs"
    if docs_root.is_dir():
        files.extend(path.resolve() for path in docs_root.rglob("*.md"))
    return sorted(set(files)), errors


def link_targets(line: str):
    for pattern in (INLINE_LINK, HTML_SOURCE):
        for match in pattern.finditer(line):
            yield match.group("target")
    reference = REFERENCE_LINK.match(line)
    if reference:
        yield reference.group("target")


def validate(root: Path, files: list[Path]) -> tuple[int, list[str]]:
    errors: list[str] = []
    checked = 0
    anchor_cache: dict[Path, set[str]] = {}

    for source in files:
        for line_number, line in markdown_lines(source):
            for raw_target in link_targets(line):
                target = raw_target.strip("<>")
                if not target or target.startswith("//") or EXTERNAL.match(target):
                    continue

                checked += 1
                path_text, _, fragment = target.partition("#")
                path_text = unquote(path_text.split("?", 1)[0])
                if path_text.startswith("/"):
                    destination = (root / path_text.lstrip("/")).resolve()
                elif path_text:
                    destination = (source.parent / path_text).resolve()
                else:
                    destination = source

                try:
                    destination.relative_to(root)
                except ValueError:
                    errors.append(
                        f"{source.relative_to(root)}:{line_number}: local link escapes repository: {target}"
                    )
                    continue

                if not destination.exists():
                    errors.append(
                        f"{source.relative_to(root)}:{line_number}: missing local target: {target}"
                    )
                    continue

                if fragment and destination.is_file() and destination.suffix.lower() == ".md":
                    anchors = anchor_cache.setdefault(destination, anchors_for(destination))
                    decoded_fragment = unquote(fragment).lower()
                    if decoded_fragment not in anchors:
                        errors.append(
                            f"{source.relative_to(root)}:{line_number}: missing anchor "
                            f"#{fragment} in {destination.relative_to(root)}"
                        )

    return checked, errors


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("files", nargs="*", help="Markdown files relative to --root")
    parser.add_argument("--root", type=Path, default=Path.cwd(), help="repository root")
    args = parser.parse_args()

    root = args.root.resolve()
    files, errors = discover_files(root, args.files)
    checked, validation_errors = validate(root, files)
    errors.extend(validation_errors)

    if errors:
        for error in errors:
            print(f"[docs] ERROR: {error}", file=sys.stderr)
        return 1

    print(f"[docs] OK: checked {checked} local links across {len(files)} Markdown files")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
