# Issue 368 Static Site Build Checker Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `python3 tools/check_site_build.py _site`, a reviewer-readable built-site checker that validates the static site and prints PASS/WARN/FAIL output.

**Architecture:** Keep manifest policy in `tools.check_site_manifest`; the new checker wraps that validator and adds site-local checks for required files, local links, anchors, QP101 assets, claims-policy copy, checked artifacts, and local-only/future classifications. The checker exposes small testable functions plus a CLI and self-test mutation harness.

**Tech Stack:** Python 3 standard library, existing `tools.check_site_manifest`, `unittest`, `make build-site`, `cargo test`.

## Global Constraints

- Use Python standard library only.
- Reuse `tools/check_site_manifest.py` for manifest and artifact checks rather than duplicating manifest rules.
- Command interface: `python3 tools/check_site_build.py --self-test` and `python3 tools/check_site_build.py _site`.
- The checker validates built `_site/`, site manifest, site HTML, copied benchmark artifacts, QP101 schema/protocol files, and gallery assets.
- The checker prints a reviewer-readable PASS/WARN/FAIL summary and exits 0 only when there are no failures.
- PASS summary must name QP101, workspace overview, benchmark methodology, checked benchmark artifacts, and local-only/future benchmark classifications.
- Negative controls must reject at least one missing QP101 schema file, one missing checked benchmark plot, one site HTML fixture missing the claims-policy caveat, and one mismatch where built site links a checked artifact that is not present in the manifest.
- Do not add browser automation or external HTTP link checking.

---

## File Structure

- Create `tools/check_site_build.py`: CLI and validation logic for the built site.
- Create `tools/test_check_site_build.py`: focused unit tests for the checker and self-test mutations.
- Keep `tools/check_site_manifest.py` unchanged unless integration requires a narrow reusable helper; manifest and checked-artifact policy remain there.

---

### Task 1: Static Site Build Checker

**Files:**
- Create: `tools/check_site_build.py`
- Create: `tools/test_check_site_build.py`

**Interfaces:**
- Consumes:
  - `tools.check_site_manifest.validate_manifest(repo_root: Path, manifest_path: Path, site_root: Path | None = None) -> list[str]`
  - `tools.check_site_manifest.validate_site_root(site_root: Path, manifest_path: Path) -> list[str]`
  - `tools.check_site_manifest.load_json(path: Path) -> Any`
  - `tools.check_site_manifest.iter_checked_artifact_paths(manifest: dict[str, Any]) -> list[tuple[str, str]]`
- Produces:
  - `CheckResult(status: str, area: str, detail: str)`
  - `check_site_build(site_root: Path, repo_root: Path | None = None) -> list[CheckResult]`
  - `run_self_test() -> list[str]`
  - `main() -> int`

- [ ] **Step 1: Write the failing unit tests**

Create `tools/test_check_site_build.py` with tests that call the checker as a Python module and through self-test. The tests must exercise a valid fixture and all required mutations:

```python
#!/usr/bin/env python3
from __future__ import annotations

import unittest

import tools.check_site_build as check_site_build


class SiteBuildCheckerTest(unittest.TestCase):
    def test_self_test_exercises_required_mutations(self) -> None:
        self.assertEqual(check_site_build.run_self_test(), [])

    def test_valid_fixture_prints_required_pass_summary_areas(self) -> None:
        fixture = check_site_build.make_fixture_site()
        self.addCleanup(fixture.cleanup)

        results = check_site_build.check_site_build(fixture.site_root, repo_root=fixture.repo_root)
        output = check_site_build.format_summary(results)

        self.assertNotIn("FAIL", output)
        for marker in [
            "PASS QP101 assets",
            "PASS workspace overview",
            "PASS benchmark methodology",
            "PASS checked benchmark artifacts",
            "PASS local-only/future classifications",
            "SUMMARY: PASS",
        ]:
            self.assertIn(marker, output)

    def test_rejects_missing_qp101_schema(self) -> None:
        fixture = check_site_build.make_fixture_site()
        self.addCleanup(fixture.cleanup)
        (fixture.site_root / "qp101.schema.json").unlink()

        results = check_site_build.check_site_build(fixture.site_root, repo_root=fixture.repo_root)

        self.assertTrue(
            any(result.status == "FAIL" and "QP101" in result.area and "qp101.schema.json" in result.detail for result in results),
            check_site_build.format_summary(results),
        )

    def test_rejects_missing_checked_benchmark_plot(self) -> None:
        fixture = check_site_build.make_fixture_site()
        self.addCleanup(fixture.cleanup)
        (fixture.site_root / "benchmarks/surface_decoder_compare/results/full/surface_decoder_compare.png").unlink()

        results = check_site_build.check_site_build(fixture.site_root, repo_root=fixture.repo_root)

        self.assertTrue(
            any("checked benchmark artifacts" in result.area and "surface_decoder_compare.png" in result.detail for result in results),
            check_site_build.format_summary(results),
        )

    def test_rejects_missing_claims_policy_caveat(self) -> None:
        fixture = check_site_build.make_fixture_site()
        self.addCleanup(fixture.cleanup)
        index = fixture.site_root / "index.html"
        index.write_text(index.read_text(encoding="utf-8").replace("Claims Policy", "Claims"), encoding="utf-8")

        results = check_site_build.check_site_build(fixture.site_root, repo_root=fixture.repo_root)

        self.assertTrue(
            any(result.status == "FAIL" and "benchmark methodology" in result.area and "Claims Policy" in result.detail for result in results),
            check_site_build.format_summary(results),
        )

    def test_rejects_unmanifested_checked_artifact_link(self) -> None:
        fixture = check_site_build.make_fixture_site()
        self.addCleanup(fixture.cleanup)
        index = fixture.site_root / "index.html"
        index.write_text(
            index.read_text(encoding="utf-8")
            + '<a href="benchmarks/surface_decoder_compare/results/full/not-in-manifest.csv">bad</a>\n',
            encoding="utf-8",
        )

        results = check_site_build.check_site_build(fixture.site_root, repo_root=fixture.repo_root)

        self.assertTrue(
            any("not listed as a checked manifest artifact" in result.detail for result in results),
            check_site_build.format_summary(results),
        )


if __name__ == "__main__":
    unittest.main()
```

- [ ] **Step 2: Run the focused tests and verify RED**

Run:

```sh
python3 -m unittest tools.test_check_site_build -q
```

Expected: FAIL before implementation because `tools.check_site_build` does not exist.

- [ ] **Step 3: Implement the checker**

Create `tools/check_site_build.py` with:

- `CheckResult` dataclass.
- `SiteFixture` dataclass for self-tests.
- `REQUIRED_FILES`, `QP101_REQUIRED_FILES`, `REQUIRED_ANCHORS`, `CLAIMS_POLICY_PHRASES`, and `CHECKED_ARTIFACT_REFERENCE_RE` constants.
- `HtmlCollector(HTMLParser)` that records `id`, `href`, and `src` attributes from built HTML.
- `check_site_build(site_root, repo_root=None)` that:
  - validates required files and QP101 files are present and non-empty;
  - parses `index.html` for required anchors and local links;
  - scans `index.html` and `app.js` for local string-literal paths such as `data/benchmark-site.json`, `qp101.schema.json`, `examples/*.qp101.json`, and `gallery/*.svg`;
  - calls `check_site_manifest.validate_manifest(repo_root, manifest_path, site_root=site_root)` and `check_site_manifest.validate_site_root(site_root, manifest_path)`;
  - loads the manifest and checks copied checked artifact paths;
  - checks claims-policy phrases;
  - checks qec-code local-only/partial and rstim-vs-stim future classifications.
- `format_summary(results)` that prints one `PASS/WARN/FAIL <area>: <detail>` line per result and a final `SUMMARY: ...` line.
- `make_fixture_site()` that creates the temporary git fixture for tests and self-test.
- `run_self_test()` that validates the fixture and required mutations.
- `main()` that implements `--self-test`, optional `--repo-root`, and `site_root`.

Implementation notes:

- Treat external `http://` and `https://` links as out of scope and do not check them.
- Treat `#anchor` links as same-page anchors and require the anchor ID to exist.
- Strip query strings and URL fragments before resolving local paths.
- When manifest validation reports errors, add one FAIL result with area `manifest and copied artifacts` and semicolon-joined detail.
- When no checked artifacts are present, fail the checked-artifact check.
- When local-only/future classifications pass, include both `qec-code` and `rstim-vs-stim` in the detail text.

- [ ] **Step 4: Run focused tests and verify GREEN**

Run:

```sh
python3 -m unittest tools.test_check_site_build -q
```

Expected: PASS.

- [ ] **Step 5: Run checker self-test**

Run:

```sh
python3 tools/check_site_build.py --self-test
```

Expected: exit 0 with a PASS self-test line.

- [ ] **Step 6: Run built-site verification**

Run:

```sh
make build-site
python3 tools/check_site_build.py _site
```

Expected: both commands exit 0. The checker output contains PASS lines naming QP101 assets, workspace overview, benchmark methodology, checked benchmark artifacts, and local-only/future classifications.

- [ ] **Step 7: Run required Rust verification**

Run:

```sh
cargo test
```

Expected: PASS.

- [ ] **Step 8: Commit**

Run:

```sh
git add tools/check_site_build.py tools/test_check_site_build.py docs/superpowers/plans/2026-07-07-issue-368-static-site-build-checker.md
git commit -m "feat: add static site build checker"
```

Expected: commit succeeds.

---

## Self Review

- Spec coverage: Task 1 covers the checker CLI, manifest reuse, local links,
  required anchors, QP101 assets, copied checked artifacts, claims-policy
  phrases, local-only/future classifications, summary output, self-test, and
  required verification.
- Placeholder scan: no TBD/TODO/implement-later markers are present.
- Type consistency: function and dataclass names match the interfaces block and
  test snippets.
