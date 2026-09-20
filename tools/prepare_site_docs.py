#!/usr/bin/env python3
"""Stage canonical contracts inside Zola's data root; never maintain a second source."""
from pathlib import Path
import json
import shutil
import sys
import tomllib

if __package__ in {None, ""}:
    sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from tools.site_versions import build_context


PUBLIC_PACKAGES = {
    'qec-ilp-core',
    'qec-code',
    'rbposd',
    'rilpqec',
    'rmatching',
    'rsinter',
    'rstim',
    'rustqec-cli',
}


def package_versions(repo_root: Path) -> dict[str, dict[str, str]]:
    workspace = tomllib.loads((repo_root / 'Cargo.toml').read_text())['workspace']
    packages: dict[str, dict[str, str]] = {}
    for member in workspace['members']:
        manifest = repo_root / member / 'Cargo.toml'
        package = tomllib.loads(manifest.read_text())['package']
        name = package['name']
        if name not in PUBLIC_PACKAGES:
            continue
        key = name.replace('-', '_')
        packages[key] = {
            'name': name,
            'version': package['version'],
            'rustdoc': name.replace('-', '_'),
        }
    missing = PUBLIC_PACKAGES - {entry['name'] for entry in packages.values()}
    if missing:
        raise ValueError(f"Missing public package manifests: {', '.join(sorted(missing))}")
    return dict(sorted(packages.items()))


def prepare(repo_root: Path) -> None:
    destination = repo_root / 'site/generated'
    destination.mkdir(parents=True, exist_ok=True)
    _, registered_version = build_context(repo_root)
    version = dict(registered_version)
    version['packages'] = package_versions(repo_root)
    (destination / 'docs-version.json').write_text(json.dumps(version, ensure_ascii=False))
    for source, filename in (
        ('rstim/doc/QP101-ZY.md', 'qp101-protocol.md'),
        ('docs/support-compatibility.md', 'support-compatibility.md'),
        ('docs/maintainer-reference.md', 'maintainer-reference.md'),
    ):
        shutil.copyfile(repo_root / source, destination / filename)


if __name__ == '__main__':
    prepare(Path(__file__).resolve().parent.parent)
