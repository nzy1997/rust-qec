# QP101 Gallery Built-In SVG Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build QP101 site gallery SVGs with `rstim render_svg` from committed Stim fixtures instead of `typst compile`.

**Architecture:** Add a small Python gallery build script with a fixed source-to-output manifest, then call it from `make build-site`. Keep JSON downloads as copies of the existing QP101 examples, but generate SVGs from Stim source fixtures through the public CLI path. Test the build script with the Cargo-built `rstim` binary, a masked `typst`, and an invalid-fixture negative control.

**Tech Stack:** Rust integration tests, Python 3 standard library, Make, GitHub Actions, `rstim render_svg`.

## Global Constraints

- Do not delete or rewrite `qp101-viz/`.
- Do not add a JSON-input mode to `render_svg`.
- Do not change QP101 JSON schema or example-download names.
- Do not add interactive gallery behavior or coordinate-layout rendering.
- Gallery SVGs must be generated from committed `.stim` source fixtures.
- Atom-loss sample gallery SVG must use `render_svg --sample_shot --seed 7`.
- The Pages workflow must not install or require Typst for gallery SVG generation.

---

## File Structure

- `tools/build_qp101_gallery.py`: new focused gallery renderer entry point.
- `qp101-viz/examples/basic.stim`: new plain source for `basic-site.svg`.
- `qp101-viz/examples/repeat-detector.stim`: new repeat/source fixture for `repeat-detector-site.svg`.
- `rstim/tests/site_gallery.rs`: new integration tests for the gallery build path.
- `Makefile`: replace Typst gallery commands with the Python renderer script.
- `site/index.html`: change gallery copy and source links from Typst to Stim / `render_svg`.
- `.github/workflows/deploy-pages.yml`: remove Typst setup, install Rust, and cache Cargo artifacts.

### Task 1: Gallery Renderer Script, Fixtures, And Tests

**Files:**
- Create: `tools/build_qp101_gallery.py`
- Create: `qp101-viz/examples/basic.stim`
- Create: `qp101-viz/examples/repeat-detector.stim`
- Create: `rstim/tests/site_gallery.rs`

**Interfaces:**
- Consumes: `rstim render_svg --in <source.stim> --out <target.svg>` plus optional `--sample_shot --seed 7`.
- Produces: `tools/build_qp101_gallery.py --repo-root <root> --out-dir <gallery-dir> [--rstim-cmd <path-or-command>]`.

- [ ] **Step 1: Add the failing integration tests**

Create `rstim/tests/site_gallery.rs` with this content:

```rust
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn rstim_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_rstim"))
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
}

fn copy_file(src: impl AsRef<Path>, dst: impl AsRef<Path>) {
    let dst = dst.as_ref();
    fs::create_dir_all(dst.parent().unwrap()).unwrap();
    fs::copy(src, dst).unwrap();
}

fn copy_gallery_inputs(temp_root: &Path) {
    let root = repo_root();
    for rel in [
        "qp101-viz/examples/basic.stim",
        "qp101-viz/examples/repeat-detector.stim",
        "qp101-viz/examples/atom-loss-sample.stim",
        "tools/build_qp101_gallery.py",
    ] {
        copy_file(root.join(rel), temp_root.join(rel));
    }
}

#[cfg(unix)]
fn create_failing_typst(bin_dir: &Path) {
    use std::os::unix::fs::PermissionsExt;

    fs::create_dir_all(bin_dir).unwrap();
    let typst = bin_dir.join("typst");
    fs::write(
        &typst,
        "#!/bin/sh\nprintf 'typst should not be used by gallery build\\n' >&2\nexit 127\n",
    )
    .unwrap();
    let mut perms = fs::metadata(&typst).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&typst, perms).unwrap();
}

#[test]
fn qp101_gallery_builds_without_typst() {
    let temp = tempfile::tempdir().unwrap();
    let temp_root = temp.path().join("repo");
    copy_gallery_inputs(&temp_root);
    let gallery_dir = temp_root.join("_site/gallery");
    let mask_bin = temp.path().join("mask-bin");
    create_failing_typst(&mask_bin);

    let mut command = Command::new("python3");
    command
        .arg(temp_root.join("tools/build_qp101_gallery.py"))
        .arg("--repo-root")
        .arg(&temp_root)
        .arg("--out-dir")
        .arg(&gallery_dir)
        .arg("--rstim-cmd")
        .arg(rstim_bin());

    let path = std::env::var_os("PATH").unwrap();
    let joined_path = std::env::join_paths(
        std::iter::once(mask_bin.as_path().to_path_buf()).chain(std::env::split_paths(&path)),
    )
    .unwrap();
    command.env("PATH", joined_path);

    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    for (name, markers) in [
        ("basic-site.svg", vec!["q0", "H", "M"]),
        ("repeat-detector-site.svg", vec!["repeat x2", "iter 2", "DETECTOR"]),
        ("atom-loss-sample.svg", vec!["LOSS", "DETECTOR", "data-style-preset=\"danger\""]),
    ] {
        let svg = fs::read_to_string(gallery_dir.join(name)).unwrap();
        assert!(svg.starts_with("<svg"), "{name} should start with <svg: {svg}");
        for marker in markers {
            assert!(svg.contains(marker), "{name} missing marker {marker}: {svg}");
        }
    }
}

#[test]
fn qp101_gallery_invalid_fixture_does_not_replace_existing_svg() {
    let temp = tempfile::tempdir().unwrap();
    let temp_root = temp.path().join("repo");
    copy_gallery_inputs(&temp_root);
    fs::write(temp_root.join("qp101-viz/examples/basic.stim"), "REPEAT nope {\nM 0\n}\n")
        .unwrap();
    let gallery_dir = temp_root.join("_site/gallery");
    fs::create_dir_all(&gallery_dir).unwrap();
    let protected_svg = gallery_dir.join("basic-site.svg");
    fs::write(&protected_svg, "existing gallery output should remain").unwrap();

    let output = Command::new("python3")
        .arg(temp_root.join("tools/build_qp101_gallery.py"))
        .arg("--repo-root")
        .arg(&temp_root)
        .arg("--out-dir")
        .arg(&gallery_dir)
        .arg("--rstim-cmd")
        .arg(rstim_bin())
        .output()
        .unwrap();
    assert!(
        !output.status.success(),
        "invalid fixture should fail gallery build"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("bad repeat count") || stderr.contains("line 1"),
        "stderr should include the render_svg parse error: {stderr}"
    );
    assert_eq!(
        fs::read_to_string(protected_svg).unwrap(),
        "existing gallery output should remain"
    );
}
```

- [ ] **Step 2: Run the tests to verify they fail for missing fixtures/script**

Run:

```sh
cargo test -p rstim --test site_gallery -q
```

Expected: FAIL because `tools/build_qp101_gallery.py`,
`qp101-viz/examples/basic.stim`, and `qp101-viz/examples/repeat-detector.stim`
do not exist yet.

- [ ] **Step 3: Add the Stim fixtures**

Create `qp101-viz/examples/basic.stim`:

```stim
H 0
CX 0 1
TICK
M 0 1
```

Create `qp101-viz/examples/repeat-detector.stim`:

```stim
QUBIT_COORDS(0, 0) 0
QUBIT_COORDS(1, 0) 1
QUBIT_COORDS(2, 0) 2
TICK
REPEAT 2 {
    CX 0 1
    CX 1 2
    TICK
    M 1
    DETECTOR(1, 0, 0) rec[-1]
}
OBSERVABLE_INCLUDE(0) rec[-1]
```

- [ ] **Step 4: Add the gallery build script**

Create `tools/build_qp101_gallery.py` with:

```python
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
        default="cargo run -p rstim --bin rstim --",
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
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
```

- [ ] **Step 5: Run the gallery tests**

Run:

```sh
cargo test -p rstim --test site_gallery -q
```

Expected: PASS.

- [ ] **Step 6: Commit Task 1**

Run:

```sh
git add tools/build_qp101_gallery.py qp101-viz/examples/basic.stim qp101-viz/examples/repeat-detector.stim rstim/tests/site_gallery.rs
git commit -m "test: cover built-in QP101 gallery rendering"
```

### Task 2: Wire Site Build, Workflow, And Public Copy

**Files:**
- Modify: `Makefile`
- Modify: `.github/workflows/deploy-pages.yml`
- Modify: `site/index.html`

**Interfaces:**
- Consumes: `tools/build_qp101_gallery.py --repo-root . --out-dir _site/gallery`.
- Produces: `make build-site` with `_site/gallery/*.svg` rendered by `rstim render_svg`.

- [ ] **Step 1: Replace Typst commands in the Makefile**

In `Makefile`, replace the three `typst compile` lines in `build-site` with:

```make
	python3 tools/build_qp101_gallery.py --repo-root . --out-dir _site/gallery
```

- [ ] **Step 2: Update site gallery copy and links**

In `site/index.html`:

- change the gallery eyebrow from `Rendered with qp101-viz` to `Rendered with rstim render_svg`
- replace the basic source link URL with
  `https://github.com/nzy1997/rstim/blob/master/qp101-viz/examples/basic.stim`
  and link text `Stim source`
- replace the repeat source link URL with
  `https://github.com/nzy1997/rstim/blob/master/qp101-viz/examples/repeat-detector.stim`
  and link text `Stim source`
- replace the atom-loss source link URL with
  `https://github.com/nzy1997/rstim/blob/master/qp101-viz/examples/atom-loss-sample.stim`
  and link text `Stim source`

- [ ] **Step 3: Update the Pages workflow**

In `.github/workflows/deploy-pages.yml`, remove:

```yaml
      - name: Set up Typst
        uses: typst-community/setup-typst@v4
```

Add these steps between `Configure Pages` and `Build site`:

```yaml
      - name: Install Rust toolchain
        run: rustup toolchain install stable --profile minimal && rustup default stable

      - uses: Swatinem/rust-cache@v2
```

- [ ] **Step 4: Verify no supported site path invokes Typst**

Run:

```sh
rg -n "typst compile|setup-typst" Makefile .github/workflows/deploy-pages.yml site
```

Expected: no matches.

- [ ] **Step 5: Run issue verification**

Run:

```sh
make build-site
python3 tools/validate_qp101_schema.py _site/qp101.schema.json _site/examples/basic.qp101.json _site/examples/repeat-detector.qp101.json _site/examples/atom-loss-sample.qp101.json
find _site/gallery -maxdepth 1 -type f -name '*.svg' -print
python3 - <<'PY'
from pathlib import Path
markers = {
    "basic-site.svg": ["q0", "H", "M"],
    "repeat-detector-site.svg": ["repeat x2", "iter 2", "DETECTOR"],
    "atom-loss-sample.svg": ["LOSS", "DETECTOR"],
}
for name, expected in markers.items():
    text = Path("_site/gallery", name).read_text()
    assert text.startswith("<svg"), name
    for marker in expected:
        assert marker in text, (name, marker)
PY
cargo test
```

Expected: all commands exit 0. The `find` command prints at least:

```text
_site/gallery/basic-site.svg
_site/gallery/repeat-detector-site.svg
_site/gallery/atom-loss-sample.svg
```

- [ ] **Step 6: Run whitespace check**

Run:

```sh
git diff --check
```

Expected: no output and exit 0.

- [ ] **Step 7: Commit Task 2**

Run:

```sh
git add Makefile .github/workflows/deploy-pages.yml site/index.html
git commit -m "feat: build QP101 gallery with rstim SVG renderer"
```
