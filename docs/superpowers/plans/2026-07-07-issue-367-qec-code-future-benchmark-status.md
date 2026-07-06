# Issue 367 QEC-Code And Future Benchmark Status Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Publish site sections that classify qec-code random-window evidence as local-only/partial and `rstim` versus Stim simulator benchmarks as future work.

**Architecture:** Keep the static site data-driven through `site/benchmark-site.json`, add source HTML cards for the qec-code and future simulator sections, and lock the policy with a focused Rust contract test. Reuse the existing manifest checker to enforce tracked, non-ignored checked-artifact paths.

**Tech Stack:** Static HTML/CSS/JS, JSON manifest, Rust integration tests with `serde_json`, Python standard-library validators.

## Global Constraints

- Do not copy generated local benchmark outputs into `_site/`.
- Do not track generated benchmark output under `benchmarks/out/`.
- qec-code random-window evidence must be labelled `local-only` or `partial` unless a separate issue changes tracked evidence policy.
- Future `rstim` versus Stim simulator benchmarks must be labelled future work, not current evidence.
- Link `docs/showcases/qec-code-random-window-benchmark.md` and `benchmarks/qec_code_random_window/README.md`.
- List relevant qec-code Makefile targets: `qec-code-random-window-bench-smoke`, `qec-code-random-window-bench-full`, `qec-code-random-window-bench-no-target-smoke`, `qec-code-random-window-bench-no-target-multiseed-smoke`, `qec-code-random-window-bench-no-target-ladder-smoke`, and `qec-code-random-window-bench-issue225-readiness-smoke`.

---

### Task 1: Contract Test And Site Classifications

**Files:**
- Modify: `rstim/tests/site_contract.rs`
- Modify: `site/index.html`
- Modify: `site/benchmark-site.json`

**Interfaces:**
- Consumes: `read_repo_file(relative: &str) -> String`, `assert_contains_all(...)`, and the existing manifest JSON shape.
- Produces: Rust test `qec_code_and_future_benchmarks_are_classified`.

- [ ] **Step 1: Write the failing test**

Append this test to `rstim/tests/site_contract.rs`:

```rust
#[test]
fn qec_code_and_future_benchmarks_are_classified() {
    let index = read_repo_file("site/index.html");
    let manifest_text = read_repo_file("site/benchmark-site.json");
    let manifest: Value = serde_json::from_str(&manifest_text)
        .expect("site benchmark manifest must be valid JSON");

    assert_contains_all(
        &index,
        &[
            "id=\"qec-code-random-window-benchmark\"",
            "<code>qec-code</code>",
            "Random-Window Distance Search",
            "Local-only evidence",
            "benchmarks/out/qec_code_random_window/",
            "docs/showcases/qec-code-random-window-benchmark.md",
            "benchmarks/qec_code_random_window/README.md",
            "qec-code-random-window-bench-smoke",
            "qec-code-random-window-bench-full",
            "qec-code-random-window-bench-no-target-smoke",
            "qec-code-random-window-bench-no-target-multiseed-smoke",
            "qec-code-random-window-bench-no-target-ladder-smoke",
            "qec-code-random-window-bench-issue225-readiness-smoke",
            "id=\"future-simulator-benchmarks\"",
            "<code>rstim</code>",
            "versus Stim Simulator Benchmarks",
            "Future work",
            "sampling",
            "detection",
            "DEM extraction",
            "conversion",
            "memory footprint",
        ],
        "qec-code and future benchmark site sections",
    );

    let families = manifest["families"]
        .as_array()
        .expect("manifest families must be an array");
    let qec_family = families
        .iter()
        .find(|family| family["id"] == "qec-code-random-window")
        .expect("qec-code random-window family must exist");
    let qec_status = qec_family["status"]
        .as_str()
        .expect("qec-code family status must be a string");
    assert!(
        matches!(qec_status, "local-only" | "partial"),
        "qec-code family must be local-only or partial, got {qec_status}"
    );
    let qec_items = qec_family["evidence_items"]
        .as_array()
        .expect("qec-code family evidence_items must be an array");
    assert!(!qec_items.is_empty(), "qec-code family must list evidence items");
    for item in qec_items {
        let item_id = item["id"].as_str().unwrap_or("<missing>");
        let status = item["status"]
            .as_str()
            .unwrap_or_else(|| panic!("qec-code item {item_id} missing status"));
        assert!(
            matches!(status, "local-only" | "partial"),
            "qec-code item {item_id} must be local-only or partial, got {status}"
        );
        let artifacts = item["artifacts"]
            .as_array()
            .unwrap_or_else(|| panic!("qec-code item {item_id} artifacts must be an array"));
        assert!(
            !(status == "existing" && item["tier"] == "full" && artifacts.is_empty()),
            "qec-code item {item_id} must not claim existing checked full evidence without artifacts"
        );
    }

    let future_family = families
        .iter()
        .find(|family| family["id"] == "rstim-vs-stim-simulator")
        .expect("future simulator family must exist");
    assert_eq!(
        future_family["status"], "future",
        "rstim versus Stim simulator family must be future"
    );
    let future_items = future_family["evidence_items"]
        .as_array()
        .expect("future simulator evidence_items must be an array");
    assert!(!future_items.is_empty(), "future simulator family must list evidence items");
    for item in future_items {
        let item_id = item["id"].as_str().unwrap_or("<missing>");
        assert_eq!(
            item["status"], "future",
            "future simulator item {item_id} must be future"
        );
        assert!(
            item["artifacts"]
                .as_array()
                .is_some_and(|artifacts| artifacts.is_empty()),
            "future simulator item {item_id} must not list checked artifacts"
        );
    }

    for family in families {
        let Some(items) = family["evidence_items"].as_array() else {
            continue;
        };
        for item in items {
            let Some(artifacts) = item["artifacts"].as_array() else {
                continue;
            };
            for artifact in artifacts {
                if artifact["checked"].as_bool().unwrap_or(false) {
                    let path = artifact["path"].as_str().unwrap_or("");
                    assert!(
                        !path.starts_with("benchmarks/out/"),
                        "checked artifact must not point under benchmarks/out/: {path}"
                    );
                }
            }
        }
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p rstim --test site_contract qec_code_and_future_benchmarks_are_classified -q
```

Expected: FAIL because `site/index.html` does not yet contain the dedicated `qec-code-random-window-benchmark` and `future-simulator-benchmarks` sections.

- [ ] **Step 3: Update the site sections**

In `site/index.html`, inside the `benchmark-evidence` section after the existing two cards, add two cards:

```html
          <article id="qec-code-random-window-benchmark" class="compact-card">
            <h3><code>qec-code</code> Random-Window Distance Search</h3>
            <p>
              Local-only evidence for random-window upper-bound searches. The
              source docs, case manifests, tests, and commands are checked in,
              but generated outputs remain under
              <code>benchmarks/out/qec_code_random_window/</code> and are not
              checked site artifacts.
            </p>
            <ul class="manifest-list" aria-label="qec-code random-window Make targets">
              <li><code>make qec-code-random-window-bench-smoke</code></li>
              <li><code>make qec-code-random-window-bench-full</code></li>
              <li><code>make qec-code-random-window-bench-no-target-smoke</code></li>
              <li><code>make qec-code-random-window-bench-no-target-multiseed-smoke</code></li>
              <li><code>make qec-code-random-window-bench-no-target-ladder-smoke</code></li>
              <li><code>make qec-code-random-window-bench-issue225-readiness-smoke</code></li>
            </ul>
            <div class="docs-card-links">
              <a href="https://github.com/nzy1997/rstim/blob/master/docs/showcases/qec-code-random-window-benchmark.md">Random-window showcase</a>
              <a href="https://github.com/nzy1997/rstim/blob/master/benchmarks/qec_code_random_window/README.md">Benchmark README</a>
            </div>
          </article>
          <article id="future-simulator-benchmarks" class="compact-card">
            <h3><code>rstim</code> versus Stim Simulator Benchmarks</h3>
            <p>
              Future work for simulator-level comparisons. No current
              site-facing results are claimed here; planned coverage includes
              sampling, detection, DEM extraction, conversion, repeat-heavy
              circuits, and memory footprint.
            </p>
            <div class="docs-card-links">
              <a href="#benchmarks">Claims policy</a>
              <a href="https://github.com/nzy1997/rstim/issues/359">Benchmark direction map</a>
            </div>
          </article>
```

- [ ] **Step 4: Tighten manifest wording without adding artifacts**

In `site/benchmark-site.json`, keep `qec-code-random-window` status as `local-only`, keep the qec-code evidence item status as `local-only`, keep `artifacts: []`, and update qec-code `claims_limit` text to include `benchmarks/out/qec_code_random_window/` and "local-only evidence". Keep `rstim-vs-stim-simulator` family and item status as `future`, with no artifacts, and update the title or claims limit to include "future work" if absent.

- [ ] **Step 5: Run test to verify it passes**

Run:

```bash
cargo test -p rstim --test site_contract qec_code_and_future_benchmarks_are_classified -q
```

Expected: PASS.

- [ ] **Step 6: Run issue verification commands**

Run:

```bash
make build-site
python3 tools/check_showcase_docs.py docs/showcases/qec-code-random-window-benchmark.md
python3 -m unittest benchmarks.qec_code_random_window.tests.test_make_targets_docs -q
python3 tools/check_site_manifest.py --repo-root . --site-root _site _site/data/benchmark-site.json
cargo test -p rstim --test site_contract qec_code_and_future_benchmarks_are_classified -q
```

Expected: each command exits 0. `_site/data/benchmark-site.json` contains no checked artifact under `benchmarks/out/`.

- [ ] **Step 7: Commit**

```bash
git add rstim/tests/site_contract.rs site/index.html site/benchmark-site.json
git commit -m "feat: classify qec code benchmark site status"
```

## Self-Review

Spec coverage: The task covers qec-code local-only/partial status, future simulator status, site links, Make target list, `benchmarks/out/` checked-artifact negative control, and the focused contract test.

Marker scan: No unresolved red-flag markers are present.

Type consistency: The plan uses the existing `serde_json::Value` manifest parsing and existing site test helper functions.
