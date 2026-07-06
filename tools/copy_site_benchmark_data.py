#!/usr/bin/env python3
"""Copy benchmark site manifest data and checked artifacts into _site."""

from __future__ import annotations

import argparse
import shutil
import sys
from pathlib import Path

try:
    from tools import check_site_manifest
except ModuleNotFoundError:
    import check_site_manifest  # type: ignore[no-redef]


def copy_benchmark_site_data(repo_root: Path, manifest_path: Path, site_root: Path) -> list[str]:
    errors = check_site_manifest.validate_manifest(repo_root, manifest_path)
    if errors:
        return errors

    manifest = check_site_manifest.load_json(manifest_path)
    data_dir = site_root / "data"
    data_dir.mkdir(parents=True, exist_ok=True)
    site_manifest = data_dir / "benchmark-site.json"
    shutil.copy2(manifest_path, site_manifest)

    for _, artifact_path in check_site_manifest.iter_checked_artifact_paths(manifest):
        source = repo_root / artifact_path
        destination = site_root / artifact_path
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, destination)

    return check_site_manifest.validate_manifest(repo_root, site_manifest, site_root=site_root)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Copy benchmark manifest and checked artifacts into _site.")
    parser.add_argument("--repo-root", type=Path, default=Path("."), help="Repository root for git checks")
    parser.add_argument("--site-root", type=Path, required=True, help="Built site root, usually _site")
    parser.add_argument("manifest", type=Path, help="Source site/benchmark-site.json")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    errors = copy_benchmark_site_data(args.repo_root, args.manifest, args.site_root)
    if errors:
        for error in errors:
            print(f"error: {error}", file=sys.stderr)
        return 1
    print(f"ok: copied benchmark site data to {args.site_root}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
