# Issue 530 rsmp Corruption Corpus Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build and verify the complete rsmp v1 corruption corpus against the pinned two-block compatibility fixture.

**Architecture:** Add terminal error latching to `SampleArchiveReader`, then add one shared `sample_archive::corruption_corpus` verifier used by both the reviewer-facing example and the integration tests. The verifier reads `catalog.json` and `manifest.toml`, derives archive layout from the fixture bytes, materializes named recipes, exhaustive truncations, and catalogued bit flips, then compares exact public `RSMP_*` codes.

**Tech Stack:** Rust 2024, existing `rstim::sample_archive`, `serde`, `serde_json`, `toml`, `sha2`, `zstd`, Cargo examples, and Cargo integration tests.

## Global Constraints

- Base archive bytes must come from `rstim/tests/fixtures/rsmp/v1/compat-v1.rsmp`.
- Catalog path is exactly `rstim/tests/fixtures/rsmp/catalog.json`.
- Fixture manifest path is exactly `rstim/tests/fixtures/rsmp/v1/manifest.toml`.
- Summary output path for the reviewer command is `benchmarks/out/rsmp-v1/corruption-summary.json`.
- Stable public codes are the 14 `RSMP_*` values from `SampleArchiveErrorCode::as_str`.
- Example stdout on success is exactly `PASS rsmp corruption corpus valid=1 invalid>=12`.
- Exhaustive truncation points are every byte length from `0` through `archive_len - 1`.
- Named invalid count excludes truncations and generated bit flips.
- After any `SampleArchiveReader` error, later `next_block()` and `finish()` calls must not return data or success.

---

### Task 1: Terminal Reader Errors

**Files:**
- Modify: `rstim/src/sample_archive/reader.rs`
- Test: `rstim/tests/rsmp_corruption_corpus.rs`

**Interfaces:**
- Consumes: `SampleArchiveReader::open`, `SampleArchiveReader::next_block`, `SampleArchiveReader::finish`, `SampleArchiveError`.
- Produces: terminal error behavior for later corpus tests.

- [ ] **Step 1: Write failing terminal behavior test**

Create `rstim/tests/rsmp_corruption_corpus.rs` with a helper that loads the v1 fixture, flips one byte in block 0's compressed stream, recomputes the trailer digest, opens `SampleArchiveReader`, and asserts that the first `next_block()` errors and the second `next_block()` errors instead of returning a block.

- [ ] **Step 2: Run focused test to verify RED**

Run: `cargo test --locked -p rstim --test rsmp_corruption_corpus terminal_reader_error_is_latched -- --exact --nocapture`

Expected: FAIL because the integration test file or terminal latching behavior is not present.

- [ ] **Step 3: Implement terminal latch**

Add `terminal_error: Option<SampleArchiveError>` to `SampleArchiveReader`. Move the current `next_block` body into `next_block_impl`; `next_block` returns the latched error when set, otherwise calls `next_block_impl`, stores any error, and returns it. At the start of `finish`, return the latched error if present.

- [ ] **Step 4: Run focused test to verify GREEN**

Run: `cargo test --locked -p rstim --test rsmp_corruption_corpus terminal_reader_error_is_latched -- --exact --nocapture`

Expected: PASS.

- [ ] **Step 5: Commit**

Run:

```bash
git add rstim/src/sample_archive/reader.rs rstim/tests/rsmp_corruption_corpus.rs
git commit -m "fix: latch rsmp reader errors"
```

### Task 2: Shared Corpus Verifier And Example

**Files:**
- Modify: `rstim/Cargo.toml`
- Modify: `rstim/src/sample_archive/mod.rs`
- Create: `rstim/src/sample_archive/corruption_corpus.rs`
- Create: `rstim/examples/rsmp_corruption_corpus.rs`
- Modify: `rstim/tests/rsmp_corruption_corpus.rs`

**Interfaces:**
- Consumes: `SampleArchiveReader`, `ArchiveLimits`, `SampleArchiveErrorCode`, `BlockHeader`, `GlobalHeader`, `ArchiveTrailer`, `compress_frame`, `decompress_frame`.
- Produces: `CorruptionCorpusOptions`, `CorruptionCorpusSummary`, `CorruptionCaseResult`, `run_corruption_corpus`, `write_summary_json`, and `PASS_LINE`.

- [ ] **Step 1: Extend the test to require the public verifier**

Add tests that call `run_corruption_corpus` on the committed catalog/manifest and assert `status == "pass"`, `valid_archives == 1`, `named_recipes >= 12`, `truncation_points == fixture_byte_length`, `bit_flips > 0`, and all failure buckets are zero.

- [ ] **Step 2: Run focused test to verify RED**

Run: `cargo test --locked -p rstim --test rsmp_corruption_corpus exact_recipe_mapping_and_summary_pass -- --exact --nocapture`

Expected: FAIL because the verifier module does not exist.

- [ ] **Step 3: Add verifier data types**

Move `toml = "0.8"` into `[dependencies]` in `rstim/Cargo.toml`. Export `pub mod corruption_corpus` from `rstim/src/sample_archive/mod.rs`. Define serializable summary/result structs with JSON field names exactly matching the issue: `status`, `fixture_hash`, `fixture_byte_length`, `valid_archives`, `named_recipes`, `truncation_points`, `bit_flips`, `counts_by_error_code`, `unexpected_successes`, `wrong_error_codes`, `panics`, and `timeouts`.

- [ ] **Step 4: Implement fixture and catalog loading**

In `corruption_corpus.rs`, parse the manifest with `toml`, require repo-relative paths, verify the archive SHA-256, parse the circuit with `parse_lines`, and parse `corruption_recipes` plus `bit_flips` from `catalog.json`. Accept `expected_error` as authoritative and accept `expected_code` only as a backward-compatible duplicate when it matches.

- [ ] **Step 5: Implement layout-aware mutations**

Derive block and stream ranges from the valid archive bytes. Implement named recipe builders for bad magic, unsupported version, required feature, reserved field, circuit mismatch, unsupported sweep, shape mismatch, duplicate/omitted/skipped/reordered blocks, dense padding, sparse varint/index malformation, Zstandard frame corruption, header digest mismatch, logical payload digest mismatch, archive digest mismatch, declared length mismatches, custom resource limit, and trailing data. Recompute only header/trailer digests and stream lengths needed by each recipe.

- [ ] **Step 6: Implement generated truncations and bit flips**

Generate one truncation case for each prefix length `0..archive_len`, all expecting `RSMP_TRUNCATED`. Generate bit flips from the catalogued semantic locators and compare each expected code separately from named recipe counts.

- [ ] **Step 7: Implement example wrapper**

Create `rstim/examples/rsmp_corruption_corpus.rs` that accepts `--catalog`, `--fixture-manifest`, and `--out`, runs the shared verifier, writes the summary JSON, prints only `PASS_LINE` on success, and exits nonzero with concise diagnostics on failure.

- [ ] **Step 8: Run focused tests and reviewer example**

Run:

```bash
cargo test --locked -p rstim --test rsmp_corruption_corpus exact_recipe_mapping_and_summary_pass -- --exact --nocapture
cargo run --locked --quiet -p rstim --example rsmp_corruption_corpus -- --catalog rstim/tests/fixtures/rsmp/catalog.json --fixture-manifest rstim/tests/fixtures/rsmp/v1/manifest.toml --out benchmarks/out/rsmp-v1/corruption-summary.json
```

Expected: test PASS; example exits 0 and prints exactly `PASS rsmp corruption corpus valid=1 invalid>=12`.

- [ ] **Step 9: Commit**

Run:

```bash
git add rstim/Cargo.toml rstim/src/sample_archive/mod.rs rstim/src/sample_archive/corruption_corpus.rs rstim/examples/rsmp_corruption_corpus.rs rstim/tests/rsmp_corruption_corpus.rs benchmarks/out/rsmp-v1/corruption-summary.json
git commit -m "test: add rsmp corruption corpus verifier"
```

### Task 3: Catalog Metadata And Negative Controls

**Files:**
- Modify: `rstim/tests/fixtures/rsmp/catalog.json`
- Modify: `rstim/tests/rsmp_corruption_corpus.rs`

**Interfaces:**
- Consumes: verifier APIs from Task 2.
- Produces: catalogued `expected_error` recipe metadata, catalogued bit flips, and negative-control tests.

- [ ] **Step 1: Update catalog recipe metadata**

For every corruption recipe, add `fixture_id = "compat_v1_two_block_sparse_dense"`, `expected_error`, `locator`, `kind`, and explicit recomputation metadata. Retain `expected_code` with the same value for the existing catalog checker.

- [ ] **Step 2: Add catalogued bit flips**

Add a top-level `bit_flips` array with semantic locators covering the global header, block 0 header, block 1 header, sparse syndrome stream, dense syndrome stream, block logical digest, trailer prefix, and archive digest.

- [ ] **Step 3: Add exact integration tests**

Add tests named `exhaustive_truncation_mapping`, `format_aware_bit_flips`, `corrupt_current_block_is_not_returned`, `already_returned_prefix_requires_finish`, and `wrong_expected_code_is_rejected`.

- [ ] **Step 4: Run focused integration tests**

Run: `cargo test --locked -p rstim --test rsmp_corruption_corpus -- --nocapture`

Expected: PASS and include the corpus success line once.

- [ ] **Step 5: Run negative control**

Run: `cargo test --locked -p rstim --test rsmp_corruption_corpus wrong_expected_code_is_rejected -- --exact --nocapture`

Expected: PASS; the test proves the real verifier exits/fails, names the recipe, reports expected and actual codes, and does not print the PASS line.

- [ ] **Step 6: Commit**

Run:

```bash
git add rstim/tests/fixtures/rsmp/catalog.json rstim/tests/rsmp_corruption_corpus.rs
git commit -m "test: catalog rsmp corruption cases"
```

### Task 4: Verification And Pull Request

**Files:**
- Modify only files needed to fix verification failures.

**Interfaces:**
- Consumes: all previous tasks.
- Produces: passing verification, pushed branch, and PR targeting `master`.

- [ ] **Step 1: Run reviewer-facing corpus command**

Run:

```bash
cargo run --locked --quiet -p rstim --example rsmp_corruption_corpus -- --catalog rstim/tests/fixtures/rsmp/catalog.json --fixture-manifest rstim/tests/fixtures/rsmp/v1/manifest.toml --out benchmarks/out/rsmp-v1/corruption-summary.json
```

Expected stdout exactly `PASS rsmp corruption corpus valid=1 invalid>=12`.

- [ ] **Step 2: Run focused integration tests**

Run:

```bash
cargo test --locked -p rstim --test rsmp_corruption_corpus -- --nocapture
cargo test --locked -p rstim --test rsmp_corruption_corpus wrong_expected_code_is_rejected -- --exact --nocapture
```

Expected: PASS.

- [ ] **Step 3: Run repository cargo test**

Run: `cargo test`

Expected: PASS.

- [ ] **Step 4: Review, commit, push, and open PR**

Run `git status --short`, review the diff, commit any remaining changes, push `agent/issue-530-verify-the-complete-corruption-corpus-run-1`, and create a PR targeting `master` with `Closes #530`.
