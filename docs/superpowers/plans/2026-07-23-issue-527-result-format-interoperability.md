# Issue 527 Result-Format Interoperability Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add strict streaming result-format adapters and wire `pack_samples` and `unpack_samples` to all v1-compatible result formats.

**Architecture:** Create `rstim::result_stream` as the adapter layer over `BitTable`, existing result writers, and `DecodedSampleBlock`. Route CLI pack input through `ResultBlockReader` into the #526 coalescing `SampleArchiveWriter`, and route archive-decoded blocks through configured `ResultBlockWriter` instances.

**Tech Stack:** Rust 2024, `rstim` crate, `BitTable`, `SampleArchiveReader`, `SampleArchiveWriter`, existing `output` byte writers, Cargo integration tests using the real `rstim` binary.

## Global Constraints

- Pack input formats are exactly `01`, `b8`, and `ptb64`.
- Measurement and observable output formats are exactly `01`, `b8`, `r8`, `hits`, and `ptb64`.
- Detector output formats are those five plus `dets`.
- `01` input requires exactly `M` ASCII `0`/`1` bytes plus newline per shot.
- `b8` input requires exact per-shot byte length and zero unused high bits.
- `ptb64` input requires canonical 64-shot groups and zero unused high shot bits in the final group.
- All pack formats must represent exactly `--shots`; short and extra input are errors.
- For `M == 0`, `b8` and `ptb64` input are empty while `01` has one newline per shot.
- `ptb64` output must retain pending shots across archive-block boundaries and pad only in `finish`.
- `ResultBlockWriter::write_block` validates all three decoded tables have equal shot counts before writing any bytes.
- Detector `dets` output emits both `D#` detector tokens and `L#` observable tokens.
- Argument-detectable CLI failures must occur before opening or truncating destinations.
- Required focused verification command: `cargo test --locked -p rstim --test rsmp_result_format_interop -- --nocapture`.
- Required PASS line: `PASS rsmp result formats pack_formats=3 measurement_formats=5 detector_formats=6 observable_formats=5 ptb64_cross_block=1 guarded_read=1 negative_cases=14`.

---

### Task 1: Add the Interop Contract Test

**Files:**
- Create: `rstim/tests/rsmp_result_format_interop.rs`

**Interfaces:**
- Consumes: real `rstim` binary, shared `rstim/tests/fixtures/rsmp/catalog.json`, existing result writers, `measurements_to_detections`, and archive APIs.
- Produces: a failing integration test that defines all positive, cross-block, guarded-read, and negative controls from #527.

- [ ] **Step 1: Write the failing test**

Create a single top-level test `rsmp_result_format_interop_contract` that:

```rust
#[test]
fn rsmp_result_format_interop_contract() {
    let pack_formats = verify_pack_formats();
    assert_eq!(pack_formats, 3);
    let measurement_formats = verify_measurement_outputs();
    assert_eq!(measurement_formats, 5);
    let detector_formats = verify_detector_outputs();
    assert_eq!(detector_formats, 6);
    let observable_formats = verify_observable_outputs();
    assert_eq!(observable_formats, 5);
    let ptb64_cross_block = verify_ptb64_cross_block();
    assert_eq!(ptb64_cross_block, 1);
    let guarded_read = verify_guarded_read();
    assert_eq!(guarded_read, 1);
    let negative_cases = verify_negative_cases();
    assert_eq!(negative_cases, 14);
    println!("PASS rsmp result formats pack_formats=3 measurement_formats=5 detector_formats=6 observable_formats=5 ptb64_cross_block=1 guarded_read=1 negative_cases=14");
}
```

- [ ] **Step 2: Run RED**

Run: `cargo test --locked -p rstim --test rsmp_result_format_interop -- --nocapture`

Expected: FAIL because `result_stream` APIs and non-`b8` CLI support are not implemented.

### Task 2: Implement `ResultBlockReader`

**Files:**
- Create: `rstim/src/result_stream.rs`
- Modify: `rstim/src/lib.rs`

**Interfaces:**
- Consumes: `BitTable`, `OutputFormat`, bounded `Read`.
- Produces: `ResultFormatError`, `ResultBlockReader<R>`, and strict streaming readers for `01`, `b8`, and `ptb64`.

- [ ] **Step 1: Add focused module tests first**

Add unit tests for invalid `01` bytes/newlines, short and extra input, `b8`
partial rows and padding, `ptb64` invalid length, `ptb64` final padding, extra
groups, zero-width input, and a guarded reader that rejects reads over 64 KiB.

- [ ] **Step 2: Implement minimal reader**

Implement `ResultBlockReader::new(input, width, total_shots, format, max_chunk_shots)`
and `next_block() -> Result<Option<BitTable>, ResultFormatError>`. Read at
most the next chunk, allocate one `BitTable`, and verify EOF after exactly the
declared total shots.

- [ ] **Step 3: Verify reader tests**

Run: `cargo test --locked -p rstim result_stream::tests:: -- --nocapture`

Expected: PASS.

### Task 3: Implement `ResultBlockWriter`

**Files:**
- Modify: `rstim/src/result_stream.rs`

**Interfaces:**
- Consumes: `DecodedSampleBlock`, `OutputFormat`, existing result writers.
- Produces: `ResultOutputKind`, `ResultBlockWriter<W>`, `write_block`, and `finish`.

- [ ] **Step 1: Add writer tests first**

Add unit tests for detector `dets` including `L#`, zero-detector observable
`dets`, non-`dets` detector output omitting observables, equal-shot validation
before write, and `ptb64` carry across two decoded blocks.

- [ ] **Step 2: Implement minimal writer**

Use a staging buffer inside `write_block` so shape failures leave the wrapped
writer untouched. Non-`ptb64` formats may serialize each block immediately after
shape validation. `ptb64` stores up to 63 pending shots and flushes the final
canonical group only from `finish`.

- [ ] **Step 3: Verify writer tests**

Run: `cargo test --locked -p rstim result_stream::tests:: -- --nocapture`

Expected: PASS.

### Task 4: Wire the CLI

**Files:**
- Modify: `rstim/src/cli.rs`

**Interfaces:**
- Consumes: `ResultBlockReader`, `ResultBlockWriter`, `ResultOutputKind`, `SampleArchiveReader`, `SampleArchiveWriter`.
- Produces: streaming `pack_samples` input for `01`/`b8`/`ptb64` and `unpack_samples` output for all compatible formats.

- [ ] **Step 1: Extend preflight**

Validate pack input format membership, output-kind compatibility, at least one
unpack output, at most one stdout output, and duplicate final paths before any
destination is opened.

- [ ] **Step 2: Stream pack**

Open input as `Read`, build `ResultBlockReader`, pass each block into
`SampleArchiveWriter::write_measurements`, then `finish`.

- [ ] **Step 3: Stream unpack**

Open the archive, create requested `ResultBlockWriter`s, pass each decoded
block from `SampleArchiveReader::next_block`, call archive `finish`, then call
each result writer `finish` and publish file outputs.

### Task 5: Verify, Review, and PR

**Files:**
- Modify only the files above unless a focused fix requires otherwise.

**Interfaces:**
- Produces: passing focused verification, broad `cargo test`, review evidence, pushed branch, and a PR against `master`.

- [ ] **Step 1: Run focused verification**

Run: `cargo test --locked -p rstim --test rsmp_result_format_interop -- --nocapture`

Expected: exit 0 and exactly one required PASS line.

- [ ] **Step 2: Run broader verification**

Run: `cargo test`

Expected: exit 0.

- [ ] **Step 3: Review, commit, push, and open PR**

Use `superpowers:verification-before-completion`, `superpowers:requesting-code-review`, and `superpowers:finishing-a-development-branch`. When finishing presents options, choose `Push and create a Pull Request`, push `agent/issue-527-support-streaming-result-format-interoperability-run-1`, and create the PR against `master` without merging.
