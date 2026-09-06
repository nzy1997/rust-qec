#!/usr/bin/env python3
"""Stage canonical contracts inside Zola's data root; never maintain a second source."""
from pathlib import Path
import shutil


def prepare(repo_root: Path) -> None:
    destination = repo_root / 'site/generated'
    destination.mkdir(parents=True, exist_ok=True)
    for source, filename in (
        ('rstim/doc/QP101-ZY.md', 'qp101-protocol.md'),
        ('docs/support-compatibility.md', 'support-compatibility.md'),
    ):
        shutil.copyfile(repo_root / source, destination / filename)


if __name__ == '__main__':
    prepare(Path(__file__).resolve().parent.parent)
