#!/usr/bin/env python3
"""Stage canonical contracts inside Zola's data root; never maintain a second source."""
from pathlib import Path
import json
import shutil
import sys

if __package__ in {None, ""}:
    sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from tools.site_versions import build_context


def prepare(repo_root: Path) -> None:
    destination = repo_root / 'site/generated'
    destination.mkdir(parents=True, exist_ok=True)
    _, version = build_context(repo_root)
    (destination / 'docs-version.json').write_text(json.dumps(version, ensure_ascii=False))
    for source, filename in (
        ('rstim/doc/QP101-ZY.md', 'qp101-protocol.md'),
        ('docs/support-compatibility.md', 'support-compatibility.md'),
    ):
        shutil.copyfile(repo_root / source, destination / filename)


if __name__ == '__main__':
    prepare(Path(__file__).resolve().parent.parent)
