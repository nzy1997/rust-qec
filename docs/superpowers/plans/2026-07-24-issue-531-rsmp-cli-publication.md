# Issue 531 rsmp CLI Publication Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Harden `pack_samples` and `unpack_samples` per-file publication and add no-output `unpack_samples --verify_only`.

**Architecture:** Keep archive validation in `SampleArchiveReader`/`SampleArchiveWriter` and keep CLI parsing thin. Add focused CLI helpers for lexical path preflight, staged output finishing, deterministic publishing, and verify-only reporting.

**Tech Stack:** Rust 2024, clap, existing `rstim` result-stream and sample-archive APIs, `tempfile` integration tests.

## Global Constraints

- The focused verification command is `cargo test --locked -p rstim --test cli_rsmp_publication -- --nocapture`.
- The focused PASS line is exactly `PASS rsmp CLI publication pack=1 unpack=1 duplicate_paths=1 normalized_paths=4 rename_failure=1 verify_only=1`.
- The reviewer-facing verify-only command must print exactly `PASS rsmp version=1.0 shots=4 blocks=2 M=10 D=9 L=1 circuit=18a857fb71f4`.
- File outputs are staged in collision-safe sibling temporaries and renamed only after archive `finish()` succeeds.
- Deterministic unpack file publication order is measurements, detectors, observables.
- A later rename failure returns `RSMP_IO`, retains already-published files, removes unpublished temporaries, leaves not-yet-committed destinations unchanged, and names already-published paths in the diagnostic.
- `--verify_only` is mutually exclusive with `--measurements_out`, `--detectors_out`, and `--obs_out`, creates no result files or result temporary files, and performs the same reader validation as ordinary unpack.
- Lexical path comparison captures the current directory once and does not call filesystem canonicalization.

---

### Task 1: Add CLI publication regression tests

**Files:**
- Create: `rstim/tests/cli_rsmp_publication.rs`

**Interfaces:**
- Consumes: real `rstim` binary, committed rsmp fixtures, and the existing CLI command spellings.
- Produces: failing tests for pack staging, unpack staging, normalized path rejection, injected rename failure, and verify-only behavior.

- [ ] **Step 1: Write the failing test file**

Create `rstim/tests/cli_rsmp_publication.rs` with helpers that spawn `env!("CARGO_BIN_EXE_rstim")`, run with controlled temp directories, assert sentinel file bytes, compare sibling directory entries, and print the required PASS line from one top-level contract test.

- [ ] **Step 2: Run the focused test and verify RED**

Run:

```bash
cargo test --locked -p rstim --test cli_rsmp_publication -- --nocapture
```

Expected: FAIL because `--verify_only` is not recognized and normalized input/output collisions are not fully implemented.

### Task 2: Harden path preflight and verify-only parsing

**Files:**
- Modify: `rstim/src/cli.rs`

**Interfaces:**
- Consumes: `preflight_pack_samples`, `preflight_unpack_samples`, `run_unpack_samples`.
- Produces: `--verify_only`, normalized preflight collision checks, and a verify-only code path.

- [ ] **Step 1: Add the CLI flag and preflight mode**

Add `verify_only: bool` to `Commands::UnpackSamples`, pass it into `run_unpack_samples`, and extend `UnpackSamplesPreflight` with a `verify_only` field.

- [ ] **Step 2: Centralize lexical normalization**

Change preflight to capture `std::env::current_dir()` once and normalize every non-`-` path relative to that directory. Reject duplicate final output paths and normalized input/output collisions before any read or temp creation.

- [ ] **Step 3: Add verify-only execution**

When `verify_only` is true, call `reader.finish()`, then print the stable success line using the returned archive summary. Do not create `OutputTarget` values.

- [ ] **Step 4: Run the targeted verify-only tests**

Run:

```bash
cargo test --locked -p rstim --test cli_rsmp_publication verify_only_rejects_output_options -- --exact --nocapture
cargo test --locked -p rstim --test cli_rsmp_publication verify_only_matches_unpack_error_code -- --exact --nocapture
```

Expected: PASS after implementation.

### Task 3: Generalize staged output publication

**Files:**
- Modify: `rstim/src/cli.rs`

**Interfaces:**
- Consumes: `PendingOutput`, `OutputTarget`, `stream_unpack_outputs`.
- Produces: close-before-rename staging, deterministic publication, `RSMP_IO` diagnostics, and a hidden debug-build-only second-rename failure hook for tests.

- [ ] **Step 1: Close staged writers before rename**

Store `PendingOutput.file` as `Option<BufWriter<File>>`, add `finish_staging()`, and make `publish()` require the writer to be closed before `rename`.

- [ ] **Step 2: Add a publisher abstraction**

Add a small `FilePublisher` trait with a filesystem implementation. The implementation reads a hidden environment variable used only by tests to inject failure on a selected rename attempt.
Gate that environment-variable injection to debug builds so release binaries cannot be affected by `RSTIM_TEST_*` variables.

- [ ] **Step 3: Publish unpack outputs in deterministic order**

After `reader.finish()` and result-writer `finish()` succeed, finish all output targets, then publish file targets in measurements, detectors, observables order. Track every successful file publication and include that list when a later rename fails.

- [ ] **Step 4: Run rename failure and staging tests**

Run:

```bash
cargo test --locked -p rstim --test cli_rsmp_publication second_rename_failure_keeps_already_published_output -- --exact --nocapture
cargo test --locked -p rstim --test cli_rsmp_publication duplicate_unpack_paths_fail_before_open -- --exact --nocapture
```

Expected: PASS after implementation.

### Task 4: Expose named corruption materialization for CLI tests

**Files:**
- Modify: `rstim/src/sample_archive/corruption_corpus.rs`
- Modify: `rstim/tests/cli_rsmp_publication.rs`

**Interfaces:**
- Consumes: #530 corruption catalog recipes.
- Produces: a public helper that materializes one named recipe from the committed fixture so CLI tests do not recreate ad hoc mutations.

- [ ] **Step 1: Add the helper**

Add a public `MaterializedCorruption` struct containing `id`, `expected_error`, `archive`, optional `circuit_text`, and `limits`, plus a `materialize_named_corruption(catalog_path, fixture_manifest_path, id)` function.

- [ ] **Step 2: Use it from the CLI test**

Use the `trailing_data` recipe for ordinary unpack versus verify-only stable-code comparison and for late unpack failure staging checks.

- [ ] **Step 3: Run the focused test file**

Run:

```bash
cargo test --locked -p rstim --test cli_rsmp_publication -- --nocapture
```

Expected: PASS with exactly one required PASS line.

### Task 5: Final verification and PR

**Files:**
- Modify as needed from prior tasks.

**Interfaces:**
- Consumes: all implementation and tests.
- Produces: committed branch and pull request against `master`.

- [ ] **Step 1: Run required focused commands**

Run the focused publication command and all named negative controls from the issue.

- [ ] **Step 2: Run reviewer-facing verify-only command**

Run:

```bash
cargo run --locked --quiet -p rstim --bin rstim -- unpack_samples --circuit rstim/tests/fixtures/rsmp/v1/compat.stim --in rstim/tests/fixtures/rsmp/v1/compat-v1.rsmp --verify_only
```

Expected stdout: `PASS rsmp version=1.0 shots=4 blocks=2 M=10 D=9 L=1 circuit=18a857fb71f4`.

- [ ] **Step 3: Run full Rust verification**

Run:

```bash
cargo test
```

Expected: PASS.

- [ ] **Step 4: Commit, push, and create PR**

Commit the implementation, push `agent/issue-531-harden-per-file-rsmp-cli-publication-and-add-ver-run-1`, and create a PR targeting `master` with `Closes #531`.
