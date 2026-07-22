#!/usr/bin/env python3
from __future__ import annotations

import argparse
import shlex
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path


@dataclass(frozen=True)
class GalleryEntry:
    source: Path
    output: str
    extra_args: tuple[str, ...] = ()


GALLERY_ENTRIES = (
    GalleryEntry(Path("qp101-viz/examples/basic.stim"), "basic-site.svg"),
    GalleryEntry(Path("qp101-viz/examples/repeat-detector.stim"), "repeat-detector-site.svg"),
    GalleryEntry(
        Path("qp101-viz/examples/atom-loss-sample.stim"),
        "atom-loss-sample.svg",
        ("--sample_shot", "--seed", "7"),
    ),
)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Build QP101 gallery SVGs with rstim render_svg."
    )
    parser.add_argument("--repo-root", type=Path, default=Path("."))
    parser.add_argument("--out-dir", type=Path, default=Path("_site/gallery"))
    parser.add_argument(
        "--rstim-cmd",
        default="cargo run --locked -p rstim --bin rstim --",
        help="Command prefix used before render_svg. Shell-style splitting is supported.",
    )
    return parser.parse_args()


def command_prefix(rstim_cmd: str) -> list[str]:
    parts = shlex.split(rstim_cmd)
    if not parts:
        raise SystemExit("--rstim-cmd must not be empty")
    return parts


def render_entry(repo_root: Path, out_dir: Path, rstim_prefix: list[str], entry: GalleryEntry) -> None:
    source = repo_root / entry.source
    if not source.is_file():
        raise FileNotFoundError(f"missing gallery source fixture: {source}")

    target = out_dir / entry.output
    target.parent.mkdir(parents=True, exist_ok=True)
    cmd = [
        *rstim_prefix,
        "render_svg",
        *entry.extra_args,
        "--in",
        str(source),
        "--out",
        str(target),
    ]
    subprocess.run(cmd, cwd=repo_root, check=True)


def main() -> int:
    args = parse_args()
    repo_root = args.repo_root.resolve()
    out_dir = args.out_dir.resolve()
    rstim_prefix = command_prefix(args.rstim_cmd)

    try:
        for entry in GALLERY_ENTRIES:
            render_entry(repo_root, out_dir, rstim_prefix, entry)
    except (FileNotFoundError, subprocess.CalledProcessError) as exc:
        print(f"error: {exc}", file=sys.stderr)
        if isinstance(exc, subprocess.CalledProcessError):
            return exc.returncode
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
