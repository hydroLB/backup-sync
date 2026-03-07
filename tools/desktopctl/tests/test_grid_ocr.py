from __future__ import annotations

import tempfile
import unittest
import zlib
from pathlib import Path

from grid_ocr import build_ocr_payload, find_text_matches, parse_tesseract_tsv


def write_png(path: Path, width: int, height: int) -> None:
    """
    Summary
    Write a tiny valid RGBA PNG without external image dependencies.

    Inputs
    Target path and image width and height.

    Outputs
    None.

    Side effects
    Writes PNG bytes to disk.

    Error handling
    Propagates filesystem failures to the caller.

    Ties to other methods
    Used by OCR tests that need a real image file with known dimensions.

    Why this exists
    Structured OCR payload generation reads PNG metadata directly from the image file.
    """

    def chunk(tag: bytes, data: bytes) -> bytes:
        return (
            len(data).to_bytes(4, "big")
            + tag
            + data
            + zlib.crc32(tag + data).to_bytes(4, "big")
        )

    row = b"\x00" + (b"\xff\xff\xff\xff" * width)
    raw = row * height
    ihdr = width.to_bytes(4, "big") + height.to_bytes(4, "big") + b"\x08\x06\x00\x00\x00"
    png = (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", ihdr)
        + chunk(b"IDAT", zlib.compress(raw))
        + chunk(b"IEND", b"")
    )
    path.write_bytes(png)


class GridOcrTests(unittest.TestCase):
    """
    Summary
    Validate structured OCR parsing and match selection.

    Inputs
    Standard unittest lifecycle hooks.

    Outputs
    Deterministic test assertions.

    Side effects
    Creates temporary PNG files for OCR payload generation.

    Error handling
    Unittest captures assertion failures and unexpected exceptions.

    Ties to other methods
    Exercises `parse_tesseract_tsv`, `build_ocr_payload`, and `find_text_matches`.

    Why this exists
    Structured OCR is the backbone of text-based fallback automation.
    """

    TSV_TEXT = """level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext
5\t1\t1\t1\t1\t1\t10\t12\t30\t8\t95.5\tBackup
5\t1\t1\t1\t1\t2\t45\t12\t20\t8\t91.0\tSync
5\t1\t1\t1\t2\t1\t14\t30\t40\t8\t88.0\tRunning
"""

    def test_parse_tesseract_tsv_returns_words(self) -> None:
        """
        Summary
        Confirm TSV parsing yields normalized OCR words with bounds.

        Inputs
        None.

        Outputs
        Assertion results.

        Side effects
        None.

        Error handling
        Unittest captures assertion failures.

        Ties to other methods
        Exercises `parse_tesseract_tsv`.

        Why this exists
        OCR payload generation depends on reliable TSV normalization.
        """

        words = parse_tesseract_tsv(self.TSV_TEXT)
        self.assertEqual(3, len(words))
        self.assertEqual("Backup", words[0]["text"])
        self.assertEqual(40, words[0]["right"])

    def test_build_ocr_payload_includes_lines_boxes_and_frame_id(self) -> None:
        """
        Summary
        Confirm structured OCR payloads include text, boxes, lines, and frame metadata.

        Inputs
        None.

        Outputs
        Assertion results.

        Side effects
        Writes a temporary PNG file to disk.

        Error handling
        Unittest captures assertion failures.

        Ties to other methods
        Exercises `build_ocr_payload`.

        Why this exists
        Worker-daemon OCR endpoints need a stable structured schema.
        """

        with tempfile.TemporaryDirectory() as tmp:
            image_path = Path(tmp) / "frame.png"
            write_png(image_path, 100, 50)
            payload = build_ocr_payload(image_path=image_path, tsv_text=self.TSV_TEXT)
        self.assertEqual("Backup Sync\nRunning", payload["text"])
        self.assertEqual({"width": 100, "height": 50}, payload["image_size"])
        self.assertEqual(3, payload["word_count"])
        self.assertEqual(2, payload["line_count"])
        self.assertTrue(payload["frame_id"])

    def test_find_text_matches_filters_case_insensitively(self) -> None:
        """
        Summary
        Confirm OCR text matching finds words regardless of case by default.

        Inputs
        None.

        Outputs
        Assertion results.

        Side effects
        Writes a temporary PNG file to disk.

        Error handling
        Unittest captures assertion failures.

        Ties to other methods
        Exercises `find_text_matches`.

        Why this exists
        Text-targeted automation should work with human-style queries.
        """

        with tempfile.TemporaryDirectory() as tmp:
            image_path = Path(tmp) / "frame.png"
            write_png(image_path, 100, 50)
            payload = build_ocr_payload(image_path=image_path, tsv_text=self.TSV_TEXT)
        matches = find_text_matches(payload, "sync")
        self.assertEqual(1, len(matches))
        self.assertEqual("Sync", matches[0]["text"])


if __name__ == "__main__":
    unittest.main()
