# Issue 523 Dense rsmp Archive Core Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the dense, zero-or-one-block `rsmp` v1 archive library API with safe limits, Zstandard frames, circuit binding, and integrity validation.

**Architecture:** Keep `sample_archive::format` as the frozen envelope and add focused modules for limits, dense packing, integrity, Zstandard framing, writer, and reader. The writer owns an existing `MeasurementTransform` and accepts raw measurement tables only; the reader constructs its transform from the supplied circuit with `ArchiveLimits.transform` and separates block-local `next_block()` validation from whole-archive `finish()` validation.

**Tech Stack:** Rust 2024, existing `BitTable`, existing `MeasurementTransform`, `sha2`, new `zstd` crate, Cargo integration tests.

## Global Constraints

- Do not redefine the fixed-width `sample_archive::format` envelope.
- Public writer input is `write_measurements(&BitTable)` only; `EncodedMeasurementBlock` is never a public writer input.
- `total_shots = 0` writes zero blocks and permits no measurement write; positive total shots requires exactly one measurement write.
- Dense logical stream order is `(shot, selected_detector)` and `(shot, free_measurement)`, LSB-first, no per-shot byte padding, zero final padding.
- Zstandard streams are independent frames with content size and checksum; writer default compression level is numeric `3`.
- Reader bounds declared lengths and Zstandard window before allocation and decompression.
- Header and trailer digest mismatches map to `RSMP_CHECKSUM_MISMATCH`.
- Zstandard frame/decode/checksum failures map to `RSMP_DECOMPRESSION_FAILED`.
- Canonical logical payload mismatches map to `RSMP_LOGICAL_DIGEST_MISMATCH`.
- Recognized sparse syndrome codec maps to `RSMP_UNSUPPORTED_FEATURE`.
- `ArchiveLimits` embeds one `MeasurementTransformLimits` and does not duplicate transform limits.
- Verification command is `cargo test --locked -p rstim --test rsmp_archive_dense -- --nocapture`.
- Required PASS line is exactly `PASS rsmp dense archive valid_cases=6 negative_cases=15`.

---

### Task 1: Dense Archive Test Contract

**Files:**
- Create: `rstim/tests/rsmp_archive_dense.rs`

**Interfaces:**
- Consumes: future `rstim::sample_archive::{ArchiveLimits, SampleArchiveOptions, SampleArchiveReader, SampleArchiveWriter}`.
- Produces: failing integration test for six valid cases, fifteen negative cases, exact error-code mapping, and the required PASS line.

- [ ] **Step 1: Write the integration test**

Create `rstim/tests/rsmp_archive_dense.rs` with helpers that parse small fixture circuits, build measurement `BitTable` values, write archives only through `SampleArchiveWriter::write_measurements`, read with `SampleArchiveReader::open`, call `next_block()` and `finish()`, mutate archive bytes through fixed field offsets, and assert exact `SampleArchiveErrorCode` values.

- [ ] **Step 2: Verify RED**

Run:

```bash
cargo test -p rstim --test rsmp_archive_dense -- --nocapture
```

Expected: FAIL because the dense archive API is not implemented yet.

### Task 2: Transform Usage Accessors and Limits

**Files:**
- Modify: `rstim/src/measurement_transform.rs`
- Create: `rstim/src/sample_archive/limits.rs`
- Modify: `rstim/src/sample_archive/mod.rs`

**Interfaces:**
- Consumes: existing `MeasurementTransform` identity, limits, and working-byte estimates.
- Produces: `ArchiveLimits`, transform actual-usage accessors, and validation helpers used by writer and reader.

- [ ] **Step 1: Add transform traversal usage fields**

Store expanded-instruction count, parity-term count, and maximum repeat depth used in `MeasurementTransform`, expose read-only accessors, and validate an already-compiled transform against a supplied `MeasurementTransformLimits`.

- [ ] **Step 2: Add `ArchiveLimits`**

Add conservative defaults with `pub transform: MeasurementTransformLimits` and archive-specific fields for total shots, detector rank, free measurements, compressed/decompressed stream/archive bytes, and Zstandard bounds.

### Task 3: Dense Codec and Zstandard Frame Helpers

**Files:**
- Create: `rstim/src/sample_archive/dense.rs`
- Create: `rstim/src/sample_archive/zstd_frame.rs`
- Modify: `rstim/Cargo.toml`
- Modify: `Cargo.lock`
- Modify: `rstim/src/sample_archive/mod.rs`

**Interfaces:**
- Consumes: `BitTable`, fixed stream codec IDs, `ArchiveLimits`.
- Produces: dense pack/unpack helpers, padding validation, Zstandard compression with content size/checksum, frame-header validation, and bounded decompression.

- [ ] **Step 1: Implement dense packing**

Pack and unpack `BitTable` data in the canonical no-per-shot-padding order with checked length arithmetic and fallible table allocation.

- [ ] **Step 2: Implement Zstandard helpers**

Compress with level `3`, content size, checksum, single-threaded settings, and no long-distance matching; parse frame headers to require declared content size/checksum and enforce window bounds before bounded decompression.

### Task 4: Writer, Reader, and Integrity

**Files:**
- Create: `rstim/src/sample_archive/integrity.rs`
- Create: `rstim/src/sample_archive/writer.rs`
- Create: `rstim/src/sample_archive/reader.rs`
- Modify: `rstim/src/sample_archive/format.rs`
- Modify: `rstim/src/sample_archive/mod.rs`

**Interfaces:**
- Consumes: `ArchiveLimits`, dense codec, Zstandard helpers, envelope structs, `MeasurementTransform`.
- Produces: public `SampleArchiveWriter` and `SampleArchiveReader` APIs with exact state-machine, digest, and error-code behavior.

- [ ] **Step 1: Add digest helpers**

Compute the header digest over header bytes excluding `header_sha256`; compute archive digest over full header, block bytes, and trailer prefix.

- [ ] **Step 2: Implement writer state machine**

Write header on `new`, accept zero or one raw measurement block through `write_measurements`, enforce one-block shape and limits, compress dense streams, write block bytes, and write trailer on `finish`.

- [ ] **Step 3: Implement reader state machine**

Read and validate header on `open`, construct and compare transform identity, validate and decode one optional block on `next_block`, and validate trailer digest/counts/EOF/trailing data on `finish`.

### Task 5: Verification, Review, and PR

**Files:**
- Modify as needed from prior tasks only.

**Interfaces:**
- Consumes: all implementation changes.
- Produces: passing required tests, cargo test evidence, reviewed commit, pushed branch, and PR.

- [ ] **Step 1: Run focused verification**

Run:

```bash
cargo test --locked -p rstim --test rsmp_archive_dense -- --nocapture
```

Expected: exit 0 and exactly one required PASS line.

- [ ] **Step 2: Run required broader verification**

Run:

```bash
cargo test
```

Expected: exit 0.

- [ ] **Step 3: Review, commit, push, and open PR**

Run the Superpowers code-review and finishing workflow, choose `Push and create a Pull Request`, push `agent/issue-523-implement-the-dense-rsmp-archive-core-run-1`, and create a PR against `master` closing #523.
