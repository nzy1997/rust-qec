#!/usr/bin/env python3
"""Rebuild the approved 01C vector identity; requires fonttools for outlining."""

from __future__ import annotations

import argparse
import hashlib
from pathlib import Path

from fontTools.pens.boundsPen import BoundsPen
from fontTools.pens.svgPathPen import SVGPathPen
from fontTools.pens.transformPen import TransformPen
from fontTools.ttLib import TTFont
from fontTools.varLib.instancer import instantiateVariableFont


FONT_SHA256 = "ef9512f92f6d579e5dc75af59a5a4b1b8b47d2eda89e00b954d44520e5369027"
COPPER = "#c56536"
PALE_COPPER = "#e5a07b"
INK = "#282828"
PAPER = "#f4efe8"

# Four full faces and four inset half faces. The latter sit along complete
# sloping boundary segments, not at the diamond's cardinal tips. Coordinates
# include clear channels so the mark works on any background without strokes.
FACES = (
    ("x-north", COPPER, "320,24 464,168 320,312 176,168"),
    ("z-east", PALE_COPPER, "472,176 616,320 472,464 328,320"),
    ("x-south", COPPER, "320,328 464,472 320,616 176,472"),
    ("z-west", PALE_COPPER, "168,176 312,320 168,464 24,320"),
    ("z-boundary-northwest", PALE_COPPER, "176,24 302,24 176,150"),
    ("x-boundary-northeast", COPPER, "616,176 616,302 490,176"),
    ("z-boundary-southeast", PALE_COPPER, "464,616 338,616 464,490"),
    ("x-boundary-southwest", COPPER, "24,464 24,338 150,464"),
)


def mark(monochrome: bool = False) -> str:
    return "\n".join(
        f'    <polygon id="{name}" fill="{INK if monochrome else color}" points="{points}"/>'
        for name, color, points in FACES
    )


def svg(width: int, height: int, description: str, body: str) -> str:
    return (
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}"'
        f' viewBox="0 0 {width} {height}" role="img" aria-labelledby="title desc">\n'
        '  <title id="title">RustQEC</title>\n'
        f'  <desc id="desc">{description}</desc>\n'
        '  <!-- Approved concept 01C. Wordmark outlines: EB Garamond Medium, SIL OFL 1.1. -->\n'
        f'{body}\n</svg>\n'
    )


def wordmark(font_path: Path) -> tuple[str, tuple[float, float, float, float]]:
    if hashlib.sha256(font_path.read_bytes()).hexdigest() != FONT_SHA256:
        raise ValueError("Font differs from the pinned EB Garamond source; see docs/branding.md")
    font = instantiateVariableFont(TTFont(font_path), {"wght": 500}, inplace=True)
    glyphs = font.getGlyphSet()
    cmap = font.getBestCmap()
    paths = SVGPathPen(glyphs)
    bounds = BoundsPen(glyphs)
    advance = 0
    for char in "RustQEC":
        glyph = glyphs[cmap[ord(char)]]
        transform = (1, 0, 0, -1, advance, 0)
        glyph.draw(TransformPen(paths, transform))
        glyph.draw(TransformPen(bounds, transform))
        advance += glyph.width
    assert bounds.bounds is not None
    return paths.getCommands(), bounds.bounds


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--font", type=Path, required=True)
    parser.add_argument(
        "--output-dir", type=Path,
        default=Path(__file__).resolve().parents[1] / "site/static/brand",
    )
    args = parser.parse_args()
    path, (left, top, right, bottom) = wordmark(args.font)
    args.output_dir.mkdir(parents=True, exist_ok=True)

    def lettering(x: float, y: float, width: float, color: str) -> tuple[str, float]:
        scale = width / (right - left)
        transform = f"translate({x - left * scale:.4f} {y - top * scale:.4f}) scale({scale:.6f})"
        return f'  <path fill="{color}" transform="{transform}" d="{path}"/>', (bottom - top) * scale

    description = (
        "A diamond surface-code patch with four square bulk checks and four triangular "
        "two-body boundary checks. Two copper tones distinguish the check families."
    )
    for filename, mono in (("rustqec-mark.svg", False), ("rustqec-mark-mono.svg", True)):
        desc = "A single-color diamond surface-code patch with four boundary checks." if mono else description
        (args.output_dir / filename).write_text(svg(640, 640, desc, mark(mono)), encoding="utf-8")

    for filename, color in (("rustqec-logo.svg", INK), ("rustqec-logo-dark.svg", PAPER)):
        text, text_height = lettering(40, 698, 880, color)
        body = f'  <g transform="translate(160 16)">\n{mark()}\n  </g>\n{text}'
        (args.output_dir / filename).write_text(
            svg(960, round(738 + text_height), description + " RustQEC in a serif wordmark.", body),
            encoding="utf-8",
        )

    text, text_height = lettering(256, 0, 664, INK)
    body = (
        f'  <g transform="translate(8 8) scale(0.35)">\n{mark()}\n  </g>\n'
        f'  <g transform="translate(0 {(240 - text_height) / 2:.4f})">\n{text}\n  </g>'
    )
    (args.output_dir / "rustqec-lockup.svg").write_text(
        svg(960, 240, description + " RustQEC in a horizontal serif wordmark.", body), encoding="utf-8",
    )


if __name__ == "__main__":
    main()
