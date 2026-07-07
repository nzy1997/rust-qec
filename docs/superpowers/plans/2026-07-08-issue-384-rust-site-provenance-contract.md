# Issue 384 Rust Site Provenance Contract Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a focused Rust site contract test that locks checked benchmark provenance as manifest-backed renderer input.

**Architecture:** Keep the check in `rstim/tests/site_contract.rs` beside the existing source-site contracts. Parse `site/benchmark-site.json`, inspect the two checked benchmark items, require canonical provenance keys and hash entries for every checked artifact, require the `site/app.js` provenance renderer hook, and reject hard-coded checked provenance in `site/index.html`.

**Tech Stack:** Rust integration test, `serde_json::Value`, existing static-site source files, Cargo workspace.

## Global Constraints

- In: `site/benchmark-site.json`, `site/app.js`, `site/index.html`.
- Out: `cargo test -p rstim --test site_contract` fails if checked evidence provenance fields or renderer hooks are removed.
- Add a focused test named `checked_benchmark_provenance_is_manifest_backed`.
- Parse `site/benchmark-site.json`.
- Inspect `surface-decoder-full` and `bb-circuit-full`.
- Require the canonical provenance keys from #380.
- Require hash entries for every checked artifact covered by #381.
- Require `site/app.js` to reference `item.provenance` through a renderer helper from #382.
- Reject hard-coded checked provenance values in `site/index.html`.
- Keep the test source-level and fast; do not run the full site build inside the Rust test.
- Do not reimplement the Python SHA-256 checker in Rust.
- The Rust contract should prove the source manifest and source renderer stay wired, while #381 and #383 prove digest correctness and reviewer-facing built-site behavior.
- Out of scope: full workspace test expansion beyond the required verification, recomputing artifact SHA-256 digests in Rust, benchmark reruns, and site architecture changes.

---

### Task 1: Focused Rust Site Contract For Checked Provenance

**Files:**
- Modify: `rstim/tests/site_contract.rs`

**Interfaces:**
- Consumes: `find_evidence_item(manifest: &Value, item_id: &str) -> (&Value, &Value)`.
- Consumes: `read_repo_file(relative: &str) -> String`.
- Produces: `checked_benchmark_provenance_is_manifest_backed()` Rust integration test.
- Produces: helper assertions local to `rstim/tests/site_contract.rs`:
  - `checked_artifact_paths(item: &Value) -> Vec<&str>`
  - `assert_canonical_provenance(item_id: &str, item: &Value)`

- [ ] **Step 1: Add provenance helper functions and the focused test**

In `rstim/tests/site_contract.rs`, add this constant and helpers after
`assert_item_has_text_list_marker`:

```rust
const CANONICAL_PROVENANCE_KEYS: &[&str] = &[
    "schema_version",
    "artifact_date",
    "source_commit",
    "commands",
    "os",
    "cpu_model",
    "rust_version",
    "python_version",
    "dependency_versions",
    "external_repository_commits",
    "seed_policy",
    "build_profile",
    "shots_or_error_budget",
    "artifact_hashes",
];

fn checked_artifact_paths(item: &Value) -> Vec<&str> {
    item["artifacts"]
        .as_array()
        .unwrap_or_else(|| panic!("evidence item artifacts must be an array"))
        .iter()
        .filter(|artifact| artifact["checked"].as_bool().unwrap_or(false))
        .map(|artifact| {
            artifact["path"]
                .as_str()
                .unwrap_or_else(|| panic!("checked artifact must carry a path: {artifact:?}"))
        })
        .collect()
}

fn assert_canonical_provenance(item_id: &str, item: &Value) {
    let provenance = item["provenance"]
        .as_object()
        .unwrap_or_else(|| panic!("{item_id} must carry canonical provenance"));

    for key in CANONICAL_PROVENANCE_KEYS {
        assert!(
            provenance.contains_key(*key),
            "{item_id} provenance is missing key {key}"
        );
    }
    assert_eq!(
        provenance["schema_version"].as_i64(),
        Some(1),
        "{item_id} provenance schema_version must be 1"
    );

    for key in CANONICAL_PROVENANCE_KEYS
        .iter()
        .copied()
        .filter(|key| *key != "schema_version")
    {
        let entry = provenance[key]
            .as_object()
            .unwrap_or_else(|| panic!("{item_id} provenance.{key} must be an object"));
        let status = entry
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or_else(|| panic!("{item_id} provenance.{key} must carry status"));
        assert!(
            matches!(status, "recorded" | "not_recorded"),
            "{item_id} provenance.{key} has unsupported status {status}"
        );
        if status == "not_recorded" {
            assert!(
                entry
                    .get("reason")
                    .and_then(Value::as_str)
                    .is_some_and(|reason| !reason.trim().is_empty()),
                "{item_id} provenance.{key} not_recorded entries must carry a reason"
            );
        }
    }

    let artifact_hashes = provenance["artifact_hashes"]
        .as_object()
        .unwrap_or_else(|| panic!("{item_id} provenance.artifact_hashes must be an object"));
    assert_eq!(
        artifact_hashes.get("status").and_then(Value::as_str),
        Some("recorded"),
        "{item_id} provenance.artifact_hashes must be recorded"
    );
    let hash_values = artifact_hashes
        .get("value")
        .and_then(Value::as_object)
        .unwrap_or_else(|| panic!("{item_id} provenance.artifact_hashes.value must be an object"));

    for path in checked_artifact_paths(item) {
        let hash_entry = hash_values
            .get(path)
            .unwrap_or_else(|| panic!("{item_id} provenance.artifact_hashes is missing checked artifact {path}"));
        assert!(
            hash_entry
                .get("sha256")
                .and_then(Value::as_str)
                .is_some_and(|digest| !digest.trim().is_empty()),
            "{item_id} checked artifact {path} must carry provenance.artifact_hashes sha256"
        );
    }
}
```

Then add this test after `checked_benchmark_artifacts_are_linked()`:

```rust
#[test]
fn checked_benchmark_provenance_is_manifest_backed() {
    let app = read_repo_file("site/app.js");
    let index = read_repo_file("site/index.html");
    let manifest_text = read_repo_file("site/benchmark-site.json");
    let manifest: Value =
        serde_json::from_str(&manifest_text).expect("site benchmark manifest must be valid JSON");

    assert_contains_all(
        &app,
        &[
            "renderProvenance",
            "renderProvenance(item.provenance)",
            "item.provenance",
            "recorded",
            "not_recorded",
            "artifact_hashes",
        ],
        "checked benchmark provenance renderer",
    );

    for hardcoded in [
        "schema_version",
        "artifact_hashes",
        "source_commit",
        "cpu_model",
        "benchmarks/surface_decoder_compare/results/full/results.csv",
        "benchmarks/surface_decoder_compare/results/full/surface_decoder_compare.png",
        "benchmarks/bb_circuit_bposd_compare/results/full/results.csv",
        "benchmarks/bb_circuit_bposd_compare/results/full/summary.md",
        "benchmarks/bb_circuit_bposd_compare/results/full/bb_circuit_bposd_compare.png",
        "benchmarks/bb_circuit_bposd_compare/results/full/reference_gap_report.md",
    ] {
        assert!(
            !index.contains(hardcoded),
            "checked provenance value {hardcoded} must come from the manifest renderer, not index.html"
        );
    }

    for item_id in ["surface-decoder-full", "bb-circuit-full"] {
        let (_, item) = find_evidence_item(&manifest, item_id);
        assert_canonical_provenance(item_id, item);
    }
}
```

- [ ] **Step 2: Run the focused test on the valid source to confirm GREEN**

Run:

```sh
cargo test -p rstim --test site_contract checked_benchmark_provenance_is_manifest_backed -q
```

Expected: PASS on the current valid source tree.

- [ ] **Step 3: Run negative control for missing manifest hash provenance**

Temporarily delete the `"artifact_hashes"` entry from
`surface-decoder-full.provenance` in `site/benchmark-site.json`, then run:

```sh
cargo test -p rstim --test site_contract checked_benchmark_provenance_is_manifest_backed -q
```

Expected: FAIL and output names `surface-decoder-full`, `artifact_hashes`, and
the missing key. Restore `site/benchmark-site.json` before proceeding.

- [ ] **Step 4: Run negative control for missing renderer hook**

Temporarily remove `renderProvenance(item.provenance)` from `site/app.js`, then
run:

```sh
cargo test -p rstim --test site_contract checked_benchmark_provenance_is_manifest_backed -q
```

Expected: FAIL and output names `renderProvenance(item.provenance)`. Restore
`site/app.js` before proceeding.

- [ ] **Step 5: Run the site contract suite**

Run:

```sh
cargo test -p rstim --test site_contract -q
```

Expected: PASS with the new focused contract included.

- [ ] **Step 6: Commit**

```sh
git add rstim/tests/site_contract.rs
git commit -m "test: lock checked benchmark provenance contract"
```

## Self Review

- Spec coverage: Task 1 covers the manifest parsing, checked item inspection,
  canonical keys, checked artifact hash-entry coverage, renderer hook, and
  static HTML hard-code rejection required by #384.
- Placeholder scan: no placeholders remain.
- Type consistency: helper signatures and `serde_json::Value` usage match the
  existing `site_contract.rs` style.
