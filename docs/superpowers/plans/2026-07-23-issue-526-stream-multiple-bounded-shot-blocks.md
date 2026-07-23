# Issue 526 Stream Multiple Bounded Shot Blocks Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extend the RSMP archive library to write canonical multi-block archives, read them sequentially with mandatory `finish`, and prove per-block mutable memory does not grow with archive block count.

**Architecture:** Keep the fixed RSMP v1 envelope and adaptive codec helpers. Add a writer-owned block-sized carry table, crate-private transform prefix encoding, sequential reader counters, and test-visible byte telemetry that records formulas and high-water marks without retaining completed block bytes.

**Tech Stack:** Rust 2024, `rstim` crate, `BitTable`, `MeasurementTransform`, `sha2`, `zstd`, existing integration tests under `rstim/tests`.

## Global Constraints

- Public writer boundary remains `SampleArchiveWriter::write_measurements(&BitTable)`.
- Default `max_shots_per_block` remains `4096`.
- Caller chunk boundaries must not affect archive block boundaries or archive bytes.
- Writer emits full configured blocks as soon as available and at most one shorter final block from `finish`.
- Reader API remains `open`, `next_block`, `finish`; only `finish` establishes whole-archive success.
- Zero-shot archives contain zero blocks and a valid trailer.
- Reader accepts structurally valid bounded nonempty blocks with zero-based contiguous block numbers and checked `first_shot` sums.
- Test diagnostics must expose byte formulas and checked arithmetic used by memory accounting.
- Required focused verification command: `cargo test --locked -p rstim --test rsmp_archive_streaming -- --nocapture`.
- Required PASS line: `PASS rsmp streaming boundary_cases=4 partition_invariant=1 malformed_cases=10 max_buffered_shots=4096 max_live_decoded_blocks=1 max_transform_payloads=2 total_block_growth_bytes=0`.

---

### Task 1: Add the Streaming Contract Test

**Files:**
- Create: `rstim/tests/rsmp_archive_streaming.rs`

**Interfaces:**
- Consumes: existing `SampleArchiveWriter::write_measurements`, `SampleArchiveReader::next_block`, `SampleArchiveReader::finish`, `MeasurementTransform`, `read_shots_b8`, `write_shots_b8`, `sample_archive::format` field offsets.
- Produces: a failing integration test that defines the required #526 behavior and final PASS line.

- [ ] **Step 1: Write the failing test**

Create `rstim/tests/rsmp_archive_streaming.rs` with one top-level test:

```rust
#[test]
fn rsmp_archive_streaming_contract() {
    let boundary_cases = verify_boundary_cases();
    assert_eq!(boundary_cases, 4);
    let partition_invariant = verify_partition_invariant();
    assert_eq!(partition_invariant, 1);
    let malformed_cases = verify_malformed_cases();
    assert_eq!(malformed_cases, 10);
    let memory = verify_bounded_memory();
    assert_eq!(memory.max_buffered_shots, 4096);
    assert_eq!(memory.max_live_decoded_blocks, 1);
    assert_eq!(memory.max_transform_payloads, 2);
    assert_eq!(memory.total_block_growth_bytes, 0);
    println!(
        "PASS rsmp streaming boundary_cases=4 partition_invariant=1 malformed_cases=10 max_buffered_shots=4096 max_live_decoded_blocks=1 max_transform_payloads=2 total_block_growth_bytes=0"
    );
}
```

The helper bodies should:

- build an `M=3, D=2, L=1` circuit and deterministic `BitTable` inputs for `4095`, `4096`, `4097`, and `8193` shots;
- assert archive block counts are `1`, `1`, `2`, and `3`;
- decode all blocks with `while let Some(block) = reader.next_block()?`;
- concatenate decoded tables and compare measurements, detections, and observables bit-for-bit against `m2d`;
- compare archive bytes for partitions `[8193]`, `[4096, 4096, 1]`, and `[1, 4095, 4097]`;
- mutate valid archives to cover the ten named negative controls and assert exact `SampleArchiveErrorCode` values; and
- print `DIAG rsmp memory ...` lines from `sample_archive::telemetry::diagnostic_lines()` before the PASS line.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --locked -p rstim --test rsmp_archive_streaming -- --nocapture`

Expected: FAIL because the test file references streaming behavior and telemetry not yet implemented.

- [ ] **Step 3: Commit only the failing test if it compiles far enough to prove missing behavior**

If the initial test fails only because production APIs are missing or current one-block checks reject the cases, commit it:

```bash
git add rstim/tests/rsmp_archive_streaming.rs
git commit -m "test: cover streaming rsmp archive blocks"
```

If it fails because of test typos, fix the test and rerun Step 2 first.

### Task 2: Add Test-Visible Byte Telemetry and Transform Prefix Encoding

**Files:**
- Modify: `rstim/src/measurement_transform.rs`
- Modify: `rstim/src/sample_archive/mod.rs`
- Create: `rstim/src/sample_archive/telemetry.rs`

**Interfaces:**
- Consumes: `BitTable`, `checked_bit_table_storage_size`, `ArchiveLimits`.
- Produces: `pub mod telemetry`, `reset_archive_telemetry`, `archive_telemetry`, `diagnostic_lines`, and crate-private `MeasurementTransform::encode_block_prefix`.

- [ ] **Step 1: Write focused unit tests**

Add tests proving that:

- telemetry checked multiplication and addition reject overflow and record formula text;
- `encode_block_prefix(&buffer, shots)` encodes only the first `shots` columns from a larger block-sized buffer; and
- `encode_block(&table)` remains equivalent to prefix encoding with all columns.

- [ ] **Step 2: Run focused tests to verify they fail**

Run: `cargo test --locked -p rstim --lib sample_archive::telemetry measurement_transform::tests::encode_block_prefix_uses_only_requested_shots`

Expected: FAIL because telemetry and prefix encoding do not exist.

- [ ] **Step 3: Implement minimal telemetry and prefix encoding**

Add `rstim/src/sample_archive/telemetry.rs` with a public `ArchiveTelemetrySnapshot` and checked formula helpers. Use atomics for high-water counters and a `Mutex<Vec<String>>` for diagnostics. Expose only stable test helpers, not mutation of archive state:

```rust
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ArchiveTelemetrySnapshot {
    pub max_buffered_shots: u64,
    pub max_live_decoded_blocks: u64,
    pub max_transform_payloads: u64,
    pub max_writer_live_bytes: u64,
    pub max_reader_live_bytes: u64,
    pub transform_retained_bytes: u64,
}
```

Add crate-private recording helpers for writer and reader code. Each helper must use checked `u64` arithmetic and push formula diagnostics such as `mul bit_table_words_per_row = (shots + 63) / 64` and `add writer_live_bytes = buffered + encoded + raw + compressed + zstd`.

In `MeasurementTransform`, add:

```rust
pub(crate) fn encode_block_prefix(
    &self,
    measurements: &BitTable,
    shots: usize,
) -> Result<EncodedMeasurementBlock, MeasurementTransformError>
```

It must require `measurements.num_major() == self.num_measurements()` and `shots <= measurements.num_minor()`, validate `shots` against block limits, and run the same selected-detector/free-column logic over `0..shots`. Make the existing public `encode_block` call `encode_block_prefix(measurements, measurements.num_minor())`.

- [ ] **Step 4: Run focused tests to verify pass**

Run: `cargo test --locked -p rstim --lib sample_archive::telemetry measurement_transform::tests::encode_block_prefix_uses_only_requested_shots`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add rstim/src/measurement_transform.rs rstim/src/sample_archive/mod.rs rstim/src/sample_archive/telemetry.rs
git commit -m "feat: add rsmp streaming memory telemetry"
```

### Task 3: Stream Canonical Blocks from the Writer

**Files:**
- Modify: `rstim/src/sample_archive/writer.rs`
- Modify: `rstim/src/sample_archive/limits.rs`
- Modify: `rstim/tests/rsmp_archive_dense.rs`

**Interfaces:**
- Consumes: `MeasurementTransform::encode_block_prefix`, telemetry recording helpers, existing `BlockHeader`, `finalize_trailer`, codec helpers.
- Produces: writer support for repeated positive chunks, chunk splitting, canonical block indexes and first shots, updated archive total defaults.

- [ ] **Step 1: Run streaming test to confirm writer failures**

Run: `cargo test --locked -p rstim --test rsmp_archive_streaming -- --nocapture`

Expected: FAIL because writing `4097` and `8193` shots currently exceeds the one-block state machine.

- [ ] **Step 2: Replace the writer one-block state with streaming counters**

Change `SampleArchiveWriter` fields to include:

```rust
next_block_index: u64,
written_shots: u64,
buffered_shots: usize,
buffer: Option<BitTable>,
```

Remove `wrote_block`. Validate writer construction against `max_total_shots`, but validate transform block working bytes with `min(total_shots, max_shots_per_block)`, not total shots. Increase `ArchiveLimits::default().max_total_shots` to at least `1_000_000` while keeping `transform.max_shots_per_block == 4096`.

- [ ] **Step 3: Implement chunk copying and block emission**

Make `write_measurements` reject zero-shot chunks, wrong measurement width, and chunks that exceed the remaining declared total. Copy caller shots into the writer-owned buffer, update `max_buffered_shots`, and call `emit_buffered_block(max_block_shots)` whenever the buffer fills. `emit_buffered_block` should encode `buffer` prefix shots, build the block header with `block_index = next_block_index`, `first_shot = written_shots`, `shot_count = shots`, write and hash the block, update telemetry, then increment counters with checked arithmetic.

- [ ] **Step 4: Emit the final short block from `finish`**

Make `finish` reject a supplied-shot total mismatch, emit the buffered final block if `buffered_shots > 0`, write a trailer using `next_block_index` and `written_shots`, flush, and return the writer.

- [ ] **Step 5: Update existing dense tests**

Update single-block expectations that are superseded by #526: a second positive `write_measurements` call may now be valid if the declared total has remaining shots, and trailer block-count mismatches now map to `RSMP_SHAPE_MISMATCH`.

- [ ] **Step 6: Run writer-relevant tests**

Run:

```bash
cargo test --locked -p rstim --test rsmp_archive_dense -- --nocapture
cargo test --locked -p rstim --test rsmp_archive_streaming -- --nocapture
```

Expected: dense tests pass; streaming test now progresses past writer partition cases but may still fail on reader multi-block behavior.

- [ ] **Step 7: Commit**

```bash
git add rstim/src/sample_archive/writer.rs rstim/src/sample_archive/limits.rs rstim/tests/rsmp_archive_dense.rs rstim/tests/rsmp_archive_streaming.rs
git commit -m "feat: stream canonical rsmp archive blocks"
```

### Task 4: Read Sequential Blocks and Drain on Finish

**Files:**
- Modify: `rstim/src/sample_archive/reader.rs`
- Modify: `rstim/src/sample_archive/mod.rs`
- Modify: `rstim/tests/rsmp_archive_dense.rs`

**Interfaces:**
- Consumes: existing frame decode, `decode_syndrome_raw`, `unpack_dense`, telemetry helpers.
- Produces: `ArchiveSummary`, contiguous block validation, trailer storage, `finish` draining unread blocks.

- [ ] **Step 1: Run streaming test to confirm reader failures**

Run: `cargo test --locked -p rstim --test rsmp_archive_streaming -- --nocapture`

Expected: FAIL because `next_block` currently returns at most one block and `finish` enforces a one-block contract.

- [ ] **Step 2: Add summary and reader state**

Add:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArchiveSummary {
    pub block_count: u64,
    pub total_shots: u64,
    pub measurement_count: u64,
    pub detector_count: u64,
    pub observable_count: u64,
}
```

Change reader fields from `returned_block: bool` to:

```rust
next_block_index: u64,
next_first_shot: u64,
trailer: Option<ArchiveTrailer>,
```

- [ ] **Step 3: Generalize `next_block`**

If the trailer is already stored, return `Ok(None)`. If `next_first_shot == header.total_shots`, read and store the trailer without updating the archive hasher with the digest field, then return `Ok(None)`. Otherwise read one block header, require `block_index == next_block_index`, `first_shot == next_first_shot`, `1..=header.max_shots_per_block`, `1..=limits.transform.max_shots_per_block`, and `first_shot + shot_count <= header.total_shots` using checked arithmetic. Decode the streams and block-local digest exactly as before. Update counters only after all block-local checks pass.

- [ ] **Step 4: Generalize `finish`**

Loop while `self.trailer.is_none()` and call `self.next_block()?` to drain unread blocks. Validate trailer block count, trailer total shots, whole-archive digest, EOF, and trailing data. Return `ArchiveSummary` only on success.

- [ ] **Step 5: Fix error mapping**

Map malformed ordering, zero-shot blocks, repeated or skipped numbers, incorrect `first_shot`, and impossible dense lengths to `RSMP_MALFORMED_ARCHIVE`; checked first-shot overflow and configured-size failures to `RSMP_LIMIT_EXCEEDED`; free-stream dimension disagreement and trailer count/shot disagreement to `RSMP_SHAPE_MISMATCH`; EOF between blocks to `RSMP_TRUNCATED`.

- [ ] **Step 6: Run reader-relevant tests**

Run:

```bash
cargo test --locked -p rstim --test rsmp_archive_streaming -- --nocapture
cargo test --locked -p rstim --test rsmp_archive_dense -- --nocapture
cargo test --locked -p rstim --test rsmp_adaptive_codec -- --nocapture
```

Expected: all three pass.

- [ ] **Step 7: Commit**

```bash
git add rstim/src/sample_archive/reader.rs rstim/src/sample_archive/mod.rs rstim/tests/rsmp_archive_dense.rs rstim/tests/rsmp_archive_streaming.rs
git commit -m "feat: read sequential rsmp archive blocks"
```

### Task 5: Final Verification and Review

**Files:**
- Modify only files required by review findings.

**Interfaces:**
- Consumes: all prior tasks.
- Produces: verified branch ready for pull request.

- [ ] **Step 1: Run required focused verification**

Run:

```bash
cargo test --locked -p rstim --test rsmp_archive_streaming -- --nocapture
```

Expected: exit 0 and exactly one required PASS line.

- [ ] **Step 2: Run regression and full suite**

Run:

```bash
cargo test --locked -p rstim --test rsmp_archive_dense -- --nocapture
cargo test --locked -p rstim --test rsmp_adaptive_codec -- --nocapture
cargo test --locked -p rstim --lib sample_archive
cargo test
git diff --check master...HEAD
```

Expected: all pass.

- [ ] **Step 3: Request final code review**

Use `superpowers:requesting-code-review` with a review package from the merge base to `HEAD`. Fix Critical and Important findings, then rerun the covering tests.

- [ ] **Step 4: Finish the branch**

Use `superpowers:verification-before-completion` and `superpowers:finishing-a-development-branch`. Choose `Push and create a Pull Request` under the Standing Answer Policy. Do not merge.
