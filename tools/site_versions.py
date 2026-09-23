#!/usr/bin/env python3
"""Build isolated documentation snapshots and a shared version navigation catalog."""
from __future__ import annotations

import argparse
from html import escape
from html.parser import HTMLParser
import json
import os
from pathlib import Path, PurePosixPath
import posixpath
import re
import shutil
import subprocess
import tempfile
import tomllib


REPO = Path(__file__).resolve().parent.parent
EXTERNAL_ARTIFACT_PATHS = (
    Path("docs/test-reports"),
    Path("site/static/data/atom-loss"),
    Path("site/static/rsmp-v1-showcase/og.png"),
)


def copy_external_artifacts(checkout: Path) -> None:
    """Restore generated inputs removed from historical source snapshots.

    Historical documentation commits predate the external artifact repository,
    so their source trees no longer contain the data required by templates.
    The primary checkout has already fetched and verified the pinned artifact
    bundle; copy only those managed paths into the temporary historical clone.
    """
    for relative in EXTERNAL_ARTIFACT_PATHS:
        source = REPO / relative
        if not source.exists():
            continue
        destination = checkout / relative
        if source.is_dir():
            shutil.copytree(source, destination, dirs_exist_ok=True)
        else:
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(source, destination)


def read_catalog(path: Path) -> dict:
    catalog = json.loads(path.read_text())
    versions = catalog["versions"]
    ids = [v["id"] for v in versions]
    paths = [v["path"] for v in versions]
    if not versions or len(ids) != len(set(ids)) or len(paths) != len(set(paths)):
        raise ValueError("Documentation versions need unique IDs and paths")
    if catalog["default"] not in ids or paths[ids.index(catalog["default"])] != "":
        raise ValueError("The default documentation version must occupy the site root")
    for version in versions:
        if (not re.fullmatch(r"[A-Za-z0-9._-]+", version["id"])
                or version["id"] in {".", ".."} or not version["label"]):
            raise ValueError("Invalid documentation version ID or label")
        path = version["path"]
        if path and (not re.fullmatch(r"[A-Za-z0-9_-]+(?:/[A-Za-z0-9._-]+)*", path)
                     or any(p in {".", ".."} for p in PurePosixPath(path).parts)):
            raise ValueError("Version paths must be relative directories without traversal")
        if version["id"] != catalog["default"] and not re.fullmatch(r"[0-9a-f]{40}", version["ref"]):
            raise ValueError("Additional documentation versions must pin a full commit SHA")
    for a in filter(None, paths):
        if any(b.startswith(a + "/") for b in paths if b != a):
            raise ValueError("Version directories must not overlap")
    return catalog


def build_context(repo_root: Path = REPO, config_path: Path | None = None) -> tuple[dict, dict]:
    catalog = read_catalog(config_path or Path(os.environ.get("DOCS_VERSIONS_FILE", repo_root / "site/versions.json")))
    current = os.environ.get("DOCS_VERSION", catalog["default"])
    version = next((v for v in catalog["versions"] if v["id"] == current), None)
    if version is None:
        raise ValueError(f"Unregistered documentation version: {current}")
    return catalog, version


class PageAnchors(HTMLParser):
    def __init__(self):
        super().__init__()
        self.anchors = set()

    def handle_starttag(self, tag, attrs):
        for key, value in attrs:
            if key == "id" and value:
                self.anchors.add(value)


def page_routes(site: Path) -> dict:
    pages = {}
    for page in sorted(site.rglob("*.html")):
        html = page.read_text()
        parser = PageAnchors()
        parser.feed(html)
        # Only template-rendered documentation participates (not bundled examples).
        if 'data-docs-version="' not in html:
            continue
        route = page.relative_to(site).as_posix()
        if route.endswith("index.html"):
            route = route[:-len("index.html")]
        pages[route] = sorted(parser.anchors)
    if "" not in pages:
        raise ValueError(f"Missing version-aware documentation homepage in {site}")
    return pages


def write_indexes(catalog: dict, sites: dict[str, Path], pages: dict | None = None) -> None:
    if set(sites) != {v["id"] for v in catalog["versions"]}:
        raise ValueError("Every advertised documentation version must have a built snapshot")
    pages = pages or {key: page_routes(site) for key, site in sites.items()}
    for current in catalog["versions"]:
        entries = []
        for version in catalog["versions"]:
            relative = posixpath.relpath(version["path"] or ".", current["path"] or ".")
            entries.append({"id": version["id"], "label": version["label"],
                            "root": relative + "/", "pages": pages[version["id"]]})
        (sites[current["id"]] / "versions.json").write_text(
            json.dumps({"current": current["id"], "versions": entries}, ensure_ascii=False) + "\n")


def synchronize_version_navigation(primary: Path, snapshot: Path, version: dict) -> None:
    """Add the current edition controls around otherwise frozen documentation content."""
    source_script = primary / "js/versions.js"
    if not source_script.is_file():
        raise ValueError(f"Missing shared version navigation script: {source_script}")
    destination_script = snapshot / "js/versions.js"
    destination_script.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(source_script, destination_script)
    for page in snapshot.rglob("*.html"):
        html = page.read_text()
        if f'data-docs-version="{version["id"]}"' not in html:
            continue
        root_match = re.search(r'data-root="([^"]+)"', html)
        if not root_match:
            raise ValueError(f"Version-aware page is missing data-root: {page}")
        root = escape(root_match.group(1), quote=True)
        label = escape(version["label"])
        additions = []
        if 'id="docs-version"' not in html:
            additions.append(f'''\n    <div class="version-strip"><div class="version-inner">
      <div class="docs-version-control">
        <span data-version-label>{label}</span>
        <label class="visually-hidden" for="docs-version" hidden>Documentation version</label>
        <select id="docs-version" hidden></select>
      </div>
      <a href="{root}/get-started/#versions">About these docs</a>
    </div></div>\n''')
        if version.get("channel") == "stable" and 'id="docs-edition-notice"' not in html:
            release_line = escape(version["release_line"])
            native_release = escape(version["native_release"])
            additions.append(f'''\n    <aside class="version-strip docs-edition-notice" id="docs-edition-notice" aria-label="Documentation edition">
      <div class="version-inner"><strong>Frozen stable edition:</strong> RustQEC {release_line}, coordinated release v{native_release}. Page text is preserved from that release; this edition banner defines its publication status.</div>
    </aside>\n''')
        if not additions:
            continue
        insertion = html.find('<div class="site-frame')
        if insertion < 0:
            body = re.search(r'<body\b[^>]*>', html)
            if body is None:
                raise ValueError(f"Version-aware page is missing body markup: {page}")
            insertion = body.end()
        page.write_text(html[:insertion] + "".join(additions) + html[insertion:])


def assemble(catalog: dict, sites: dict[str, Path], output: Path) -> None:
    if output.exists():
        raise ValueError(f"Output already exists; choose an empty output path: {output}")
    if any(output.resolve().is_relative_to(site.resolve()) for site in sites.values()):
        raise ValueError("Publication output must be outside the input snapshots")
    if set(sites) != {v["id"] for v in catalog["versions"]}:
        raise ValueError("Every advertised documentation version must have a built snapshot")
    primary = sites[catalog["default"]]
    for version in catalog["versions"]:
        if version["path"] and (primary / version["path"]).exists():
            raise ValueError(f"Version directory collides with existing site content: {version['path']}")
        marker = f'data-docs-version="{version["id"]}"'
        if marker not in (sites[version["id"]] / "index.html").read_text():
            raise ValueError(f"Snapshot version does not match its registration: {version['id']}")
    pages = {key: page_routes(site) for key, site in sites.items()}
    shutil.copytree(primary, output)
    published_sites = {catalog["default"]: output}
    for version in catalog["versions"]:
        if version["id"] != catalog["default"]:
            snapshot = output / version["path"]
            shutil.copytree(sites[version["id"]], snapshot)
            synchronize_version_navigation(output, snapshot, version)
            published_sites[version["id"]] = snapshot
    # Write cross-version indexes only into the publication copies. Isolated
    # input snapshots must remain independently testable and deployable.
    write_indexes(catalog, published_sites, pages)


def build_versions(config: Path, primary: Path, output: Path) -> None:
    catalog = read_catalog(config)
    sites = {catalog["default"]: primary}
    with tempfile.TemporaryDirectory(prefix="rustqec-docs-") as temp:
        scratch = Path(temp)
        for version in catalog["versions"]:
            if version["id"] == catalog["default"]:
                continue
            workspace = scratch / version["id"]
            workspace.mkdir()
            checkout = workspace / "checkout"
            # Benchmark provenance checks need a real Git index. A local shared
            # clone preserves it without changing the caller's worktree or refs.
            subprocess.run(["git", "clone", "--quiet", "--shared", "--no-checkout", str(REPO), str(checkout)],
                           check=True)
            subprocess.run(["git", "checkout", "--quiet", "--detach", version["ref"]], cwd=checkout, check=True)
            if not (checkout / "tools/site_versions.py").is_file():
                raise ValueError("Snapshot must include the versioned documentation infrastructure")
            copy_external_artifacts(checkout)
            base_url = tomllib.loads((REPO / "site/config.toml").read_text())["base_url"].rstrip("/")
            env = dict(os.environ, DOCS_VERSION=version["id"], DOCS_VERSIONS_FILE=str(config.resolve()),
                       DOCS_SITE_BASE_URL=f'{base_url}/{version["path"]}')
            subprocess.run(["make", "build-site"], cwd=checkout, env=env, check=True)
            subprocess.run(["python3", "tools/check_site_build.py", "_site"], cwd=checkout, check=True)
            sites[version["id"]] = checkout / "_site"
        assemble(catalog, sites, output)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=["index", "build"])
    parser.add_argument("--site-root", type=Path, default=Path("_site"))
    parser.add_argument("--config", type=Path, default=Path(os.environ.get("DOCS_VERSIONS_FILE", REPO / "site/versions.json")))
    parser.add_argument("--output", type=Path, default=Path("_pages"))
    args = parser.parse_args()
    if args.command == "index":
        _, current = build_context(config_path=args.config)
        # Standalone builds advertise only the snapshot they actually contain.
        write_indexes({"versions": [dict(current, path="")]}, {current["id"]: args.site_root})
    else:
        build_versions(args.config, args.site_root, args.output)


if __name__ == "__main__":
    main()
