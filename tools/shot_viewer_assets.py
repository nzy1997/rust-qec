#!/usr/bin/env python3
"""Write or verify the version-locked shot viewer asset manifest."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
ASSET_ROOT = ROOT / "rstim" / "assets" / "shot-viewer"
MANIFEST = ASSET_ROOT / "asset-manifest.json"
FILES = (
    "index.html",
    "app.js",
    "shot-viewer.css",
    "pkg/rstim_shot_web_bg.wasm",
)


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def expected_manifest() -> dict[str, object]:
    return {
        "format_version": "rstim-shot-assets-v1",
        "wasm_bindgen_version": "0.2.126",
        "files": {name: {"sha256": digest(ASSET_ROOT / name)} for name in FILES},
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--write", action="store_true")
    args = parser.parse_args()
    expected = expected_manifest()
    if args.write:
        MANIFEST.write_text(json.dumps(expected, indent=2, sort_keys=True) + "\n")
        return 0
    if not MANIFEST.exists():
        raise SystemExit(f"missing shot viewer asset manifest: {MANIFEST}")
    actual = json.loads(MANIFEST.read_text())
    if actual != expected:
        raise SystemExit("shot viewer assets do not match asset-manifest.json; run make build-shot-viewer")
    print("shot viewer assets: verified")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
