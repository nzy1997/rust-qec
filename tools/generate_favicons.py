#!/usr/bin/env python3
"""Rasterize the checked-in SVG mark into PNG and multi-size ICO fallbacks.

Requires CairoSVG and Pillow only when regenerating the committed assets.
"""

from __future__ import annotations

import argparse
from io import BytesIO
from pathlib import Path

import cairosvg
from PIL import Image


ROOT = Path(__file__).resolve().parents[1]


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source', type=Path, default=ROOT / 'site/static/brand/rustqec-mark.svg')
    parser.add_argument('--output-dir', type=Path, default=ROOT / 'site/static')
    args = parser.parse_args()
    raster = cairosvg.svg2png(url=str(args.source), output_width=384, output_height=384)
    with Image.open(BytesIO(raster)) as rendered:
        mark = rendered.convert('RGBA')
    brand = args.output_dir / 'brand'
    brand.mkdir(parents=True, exist_ok=True)
    mark.resize((32, 32), Image.Resampling.LANCZOS).save(brand / 'favicon-32.png')
    mark.resize((48, 48), Image.Resampling.LANCZOS).save(
        args.output_dir / 'favicon.ico', format='ICO',
        sizes=[(16, 16), (32, 32), (48, 48)], bitmap_format='bmp',
    )


if __name__ == '__main__':
    main()
