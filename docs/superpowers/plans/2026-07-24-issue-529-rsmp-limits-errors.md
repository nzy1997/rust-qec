# Issue 529 rsmp Limits and Error Codes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Complete rsmp v1 archive limit enforcement and stable public error rendering for issue #529.

**Architecture:** Add one focused integration test that drives the public contract, then tighten the existing shared reader/writer limit surface and error mappings. Keep `ArchiveLimits.transform` as the only configurable copy of transform ceilings, and keep archive-specific accounting in `sample_archive`.

**Tech Stack:** Rust 2024, existing `rstim::sample_archive`, existing `MeasurementTransform`, existing `BitTable::try_new`, `zstd`, Cargo integration tests.

## Global Constraints

- `ArchiveLimits` embeds `pub transform: MeasurementTransformLimits`.
- `ArchiveLimits` exposes checked `u64` bounds for exactly 20 resources: archive bytes, block count, total shots, the nine nested transform fields, rank, free width, compressed bytes per frame, uncompressed bytes per frame, aggregate compressed frame bytes, aggregate uncompressed frame bytes, Zstandard window bytes, and Zstandard decoder memory bytes.
- Reader and writer use the exact caller-provided `ArchiveLimits.transform`; no widening or default substitution.
- Archive-controlled values must pass checked scalar, product, aggregate, and conversion limits before allocation, Zstandard initialization, or transform reconstruction.
- Stable public codes are exactly the 14 issue-listed `RSMP_*` values.
- CLI archive diagnostics are exactly `rsmp error [<CODE>]: <plain-language detail>`.
- Required verification command: `cargo test --locked -p rstim --test rsmp_limits_and_errors -- --nocapture`.
- Required verification output line: `PASS rsmp limits fields=20 overflow=1 prepayload=1 zstd_window=1 aggregate=1 public_codes=14 cli_snapshots=14`.

---

### Task 1: Add Contract Test

**Files:**
- Create: `rstim/tests/rsmp_limits_and_errors.rs`

**Interfaces:**
- Consumes: `ArchiveLimits`, `MeasurementTransformLimits`, `SampleArchiveReader`, `SampleArchiveWriter`, `SampleArchiveErrorCode`, rsmp fixed field offsets, and CLI binary path.
- Produces: Failing issue #529 regression coverage before production code changes.

- [ ] **Step 1: Write failing integration test**

Create `rstim/tests/rsmp_limits_and_errors.rs` with one top-level test `rsmp_limits_and_error_contract`. It must build small valid archives, mutate structural fields, assert exact `SampleArchiveErrorCode` values, test no-payload reads after over-limit declared lengths, exercise each of the 20 `ArchiveLimits` resources with lowered limits, and check one exact CLI stderr snapshot per public code.

- [ ] **Step 2: Run test to verify RED**

Run: `cargo test --locked -p rstim --test rsmp_limits_and_errors -- --nocapture`

Expected: FAIL because the test file or assertions cover behavior not yet implemented.

### Task 2: Complete Limit Surface and Shared Accounting

**Files:**
- Modify: `rstim/src/sample_archive/limits.rs`
- Modify: `rstim/src/sample_archive/writer.rs`
- Modify: `rstim/src/sample_archive/reader.rs`
- Modify: `rstim/src/measurement_transform.rs`

**Interfaces:**
- Consumes: the existing reader/writer constructors and `MeasurementTransform::validate_actual_usage`.
- Produces: `ArchiveLimits` fields for archive bytes and block count plus checked shared accounting before allocation/read/decode.

- [ ] **Step 1: Add archive byte and block count limits**

Extend `ArchiveLimits` with `max_archive_bytes` and `max_block_count`, keep defaults conservative, and update tests/helpers constructing `ArchiveLimits`.

- [ ] **Step 2: Enforce writer totals**

Before writing block payloads or trailer, check header plus block header/frame/trailer aggregate archive bytes, block count, total shots, rank, free width, stream/frame totals, transform retained bytes, and per-block live memory with checked arithmetic.

- [ ] **Step 3: Enforce reader totals before payload reads**

Track archive bytes read, block count, shots, compressed frame bytes, and uncompressed frame bytes. Reject over-limit declared block lengths before `read_stream`, and update counters only through checked arithmetic.

- [ ] **Step 4: Preserve exact nested transform limits**

Keep writer actual-usage checks and reader transform construction wired to the exact `limits.transform`, and ensure all transform allocation paths use checked estimates and `BitTable::try_new`.

### Task 3: Freeze Error Mapping and Zstandard Precedence

**Files:**
- Modify: `rstim/src/sample_archive/format.rs`
- Modify: `rstim/src/sample_archive/reader.rs`
- Modify: `rstim/src/sample_archive/zstd_frame.rs`
- Modify: `rstim/src/main.rs`
- Modify: `rstim/src/cli.rs`

**Interfaces:**
- Consumes: `SampleArchiveErrorCode::as_str`, `SampleArchiveError`, existing reader validation order, and CLI pack/unpack commands.
- Produces: exact 14 public codes and exact CLI archive diagnostic lines.

- [ ] **Step 1: Make unsupported identifiers map to unsupported feature**

Unknown required flags, transform IDs, reference IDs, canonicalization/fingerprint/codec-suite IDs, and block codec-suite features return `RSMP_UNSUPPORTED_FEATURE`; malformed bytes, ordering, padding, varints, and frame multiplicity return `RSMP_MALFORMED_ARCHIVE`.

- [ ] **Step 2: Make concatenated Zstandard frames malformed**

Require one declared frame per slice; an otherwise valid first frame followed by another frame in the same declared slice returns `RSMP_MALFORMED_ARCHIVE`, while frame/decode/checksum failures return `RSMP_DECOMPRESSION_FAILED`.

- [ ] **Step 3: Enforce window and decoder memory before allocation**

Validate frame header window and decoder memory against `ArchiveLimits` before output allocation or `zstd::bulk::decompress`.

- [ ] **Step 4: Render CLI archive diagnostics exactly**

Map rsmp library errors to `rsmp error [CODE]: detail` and teach `main` not to prepend the generic `Error: ` prefix to those archive diagnostics.

### Task 4: Verification and PR

**Files:**
- Modify tests or implementation files only if verification exposes gaps.

**Interfaces:**
- Consumes: all tasks above.
- Produces: committed branch, pushed worker branch, and PR for issue #529.

- [ ] **Step 1: Run required focused verification**

Run: `cargo test --locked -p rstim --test rsmp_limits_and_errors -- --nocapture`

Expected: PASS with exactly one required `PASS rsmp limits ...` line.

- [ ] **Step 2: Run repository cargo test**

Run: `cargo test`

Expected: PASS.

- [ ] **Step 3: Review, commit, push, and open PR**

Review `git diff`, commit scoped changes, push `agent/issue-529-enforce-archive-limits-and-stable-error-codes-run-1`, and create a PR targeting `master` with `Closes #529`.
