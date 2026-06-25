# Issue 215 rstim CLI DEM Pipeline Showcase Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a runnable rstim CLI and detector-error-model pipeline showcase with a contract test for its documented bad-input example.

**Architecture:** Keep the user-facing walkthrough in one showcase Markdown page under `docs/showcases/`. Keep command behavior enforcement in the existing Rust CLI integration test file by extracting the documented invalid input from the page and feeding it to `rstim stats`.

**Tech Stack:** Markdown, Python showcase checker, Rust integration tests, existing `rstim` CLI.

## Global Constraints

- Add `docs/showcases/rstim-cli-dem-pipeline.md`.
- Modify `rstim/tests/cli_integration.rs` only for the focused doc/CLI contract test.
- The showcase page must include these exact `##` sections: `What This Shows`, `Run It`, `Expected Result`, `Code`, `Verification`, and `Limits`.
- Link from the page to `rstim/doc/cli.md`, `rstim/tests/cli_stats.rs`, `rstim/tests/cli_sample_dem.rs`, and `rstim/tests/cli_integration.rs`.
- Use tiny deterministic examples only.
- Prefer exact fields/counts over broad claims about simulator parity.
- Do not claim full Stim parity or add new simulator features.
- Include one deliberately invalid CLI example only because the Rust contract test mechanically verifies that the documented input still fails.
- Required issue verification commands:
  - `python3 tools/check_showcase_docs.py docs/showcases/rstim-cli-dem-pipeline.md`
  - `cargo test -p rstim --test cli_stats --test cli_sample_dem --test cli_integration -q`
- Required repository workflow command when applicable:
  - `cargo test`

---

### Task 1: Add Showcase Page And Bad-Input Contract Test

**Files:**
- Create: `docs/showcases/rstim-cli-dem-pipeline.md`
- Modify: `rstim/tests/cli_integration.rs`

**Interfaces:**
- Consumes: `run_with_stdin(args: &[&str], stdin_data: &str) -> std::process::Output` from `rstim/tests/cli_integration.rs`.
- Produces: `showcase_documented_bad_stats_input_still_fails()` Rust integration test.
- Produces: `extract_marked_stim_block(markdown: &str, start_marker: &str, end_marker: &str) -> String` test helper.
- Produces: `docs/showcases/rstim-cli-dem-pipeline.md` accepted by `tools/check_showcase_docs.py`.

- [ ] **Step 1: Write the failing Rust contract test**

Add this helper and test to the end of `rstim/tests/cli_integration.rs`:

```rust
fn extract_marked_stim_block(markdown: &str, start_marker: &str, end_marker: &str) -> String {
    let after_start = markdown
        .split(start_marker)
        .nth(1)
        .unwrap_or_else(|| panic!("missing start marker {start_marker}"));
    let marked = after_start
        .split(end_marker)
        .next()
        .unwrap_or_else(|| panic!("missing end marker {end_marker}"));
    let fence_start = marked
        .find("```stim")
        .unwrap_or_else(|| panic!("missing stim fence after {start_marker}"));
    let after_fence = &marked[fence_start + "```stim".len()..];
    let fence_end = after_fence
        .find("```")
        .unwrap_or_else(|| panic!("missing closing stim fence after {start_marker}"));
    let mut block = after_fence[..fence_end].trim().to_owned();
    block.push('\n');
    block
}

#[test]
fn showcase_documented_bad_stats_input_still_fails() {
    let doc_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../docs/showcases/rstim-cli-dem-pipeline.md");
    let markdown = std::fs::read_to_string(&doc_path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", doc_path.display()));
    let input = extract_marked_stim_block(
        &markdown,
        "<!-- rstim-cli-dem-pipeline-bad-input-start -->",
        "<!-- rstim-cli-dem-pipeline-bad-input-end -->",
    );

    let output = run_with_stdin(&["stats"], &input);

    assert!(
        !output.status.success(),
        "documented bad stats input unexpectedly succeeded with stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("bad repeat count"), "stderr: {stderr}");
    assert!(!stderr.contains("panicked"), "stderr: {stderr}");
}
```

- [ ] **Step 2: Run the focused test and verify it fails red**

Run:

```sh
cargo test -p rstim --test cli_integration showcase_documented_bad_stats_input_still_fails -q
```

Expected: FAIL because `docs/showcases/rstim-cli-dem-pipeline.md` does not exist yet, or because the required markers are absent.

- [ ] **Step 3: Add the showcase page**

Create `docs/showcases/rstim-cli-dem-pipeline.md` with exactly this content:

````markdown
# rstim CLI DEM Pipeline

This showcase runs one tiny noisy circuit through the existing `rstim` CLI:
inspect it, sample detector events, extract a detector error model, and sample
from that DEM.

## What This Shows

The circuit has one qubit, one deterministic `X_ERROR(1)`, one measurement,
one detector, and one observable. That makes every command output small enough
to inspect directly while still exercising the detector-event and DEM path.

The pipeline is:

1. `stats` counts the circuit structure.
2. `detect` samples detector events from the circuit.
3. `analyze_errors` writes a detector error model.
4. `sample_dem` samples detector events from that model.

## Run It

Run these commands from the repository root. The commands use `cargo run -q`
so they exercise the workspace CLI without requiring a separately installed
`rstim` binary.

```sh
workdir="${TMPDIR:-/tmp}/rstim-cli-dem-pipeline"
rm -rf "$workdir"
mkdir -p "$workdir"

cat > "$workdir/pipeline.stim" <<'STIM'
R 0
X_ERROR(1) 0
M 0
DETECTOR rec[-1]
OBSERVABLE_INCLUDE(0) rec[-1]
STIM

cargo run -q -p rstim -- stats --in "$workdir/pipeline.stim"
cargo run -q -p rstim -- detect --shots 1 --out_format dets --in "$workdir/pipeline.stim"
cargo run -q -p rstim -- analyze_errors --in "$workdir/pipeline.stim" --out "$workdir/pipeline.dem"
cat "$workdir/pipeline.dem"
cargo run -q -p rstim -- sample_dem --shots 1 --out_format dets --in "$workdir/pipeline.dem"
```

The documented failure case uses an invalid repeat count:

<!-- rstim-cli-dem-pipeline-bad-input-start -->
```stim
REPEAT two {
  M 0
}
```
<!-- rstim-cli-dem-pipeline-bad-input-end -->

```sh
cat > "$workdir/bad-repeat.stim" <<'STIM'
REPEAT two {
  M 0
}
STIM

cargo run -q -p rstim -- stats --in "$workdir/bad-repeat.stim"
```

## Expected Result

`stats` prints exact field counts for this circuit:

```text
instruction_count: 5
repeat_blocks: 0
max_repeat_depth: 0
num_qubits: 1
num_measurements: 1
num_detectors: 1
num_observables: 1
num_ticks: 0
num_sweep_bits: 0
```

`detect --out_format dets` prints one deterministic detector event and one
observable flip:

```text
shot D0 L0
```

`analyze_errors` writes this DEM:

```text
error(1) D0 L0
```

`sample_dem --out_format dets` samples that DEM back to the same detector and
observable labels:

```text
shot D0 L0
```

The invalid repeat-count example exits nonzero and writes this stderr snippet:

```text
Error: line 1: bad repeat count
```

## Code

Primary CLI documentation:

- [`rstim/doc/cli.md`](rstim/doc/cli.md)

Tests that cover the showcased command families:

- [`rstim/tests/cli_stats.rs`](rstim/tests/cli_stats.rs)
- [`rstim/tests/cli_sample_dem.rs`](rstim/tests/cli_sample_dem.rs)
- [`rstim/tests/cli_integration.rs`](rstim/tests/cli_integration.rs)

## Verification

Validate this page's section structure and repo-relative links:

```sh
python3 tools/check_showcase_docs.py docs/showcases/rstim-cli-dem-pipeline.md
```

Run the CLI tests that cover the showcased commands and the documented
bad-input contract:

```sh
cargo test -p rstim --test cli_stats --test cli_sample_dem --test cli_integration -q
```

Expected: the checker prints `ok:` for this page, and the Cargo command exits
0. The `cli_integration` suite fails if the documented invalid repeat-count
input is replaced by a valid circuit.

## Limits

This is a CLI data-path smoke example, not a simulator parity claim. It uses a
single deterministic error mechanism so the stdout snippets stay exact and
reviewable. It does not cover random-noise statistics, packed binary output
formats, large circuits, or decoder performance.
````

- [ ] **Step 4: Run the focused test and verify it passes green**

Run:

```sh
cargo test -p rstim --test cli_integration showcase_documented_bad_stats_input_still_fails -q
```

Expected: PASS.

- [ ] **Step 5: Run the showcase checker**

Run:

```sh
python3 tools/check_showcase_docs.py docs/showcases/rstim-cli-dem-pipeline.md
```

Expected output includes:

```text
ok: docs/showcases/rstim-cli-dem-pipeline.md
```

- [ ] **Step 6: Run the issue-required CLI test set**

Run:

```sh
cargo test -p rstim --test cli_stats --test cli_sample_dem --test cli_integration -q
```

Expected: PASS.

- [ ] **Step 7: Run the repository workflow test command**

Run:

```sh
cargo test
```

Expected: PASS, unless the local sandbox cannot resolve Cargo registry dependencies. If registry access fails before compiling project code, record the exact environment failure in the final risk log.

- [ ] **Step 8: Commit the implementation**

Run:

```sh
git add docs/showcases/rstim-cli-dem-pipeline.md rstim/tests/cli_integration.rs
git commit -m "docs: add rstim cli dem pipeline showcase"
```

Expected: commit succeeds with only the showcase page and focused CLI integration test.
