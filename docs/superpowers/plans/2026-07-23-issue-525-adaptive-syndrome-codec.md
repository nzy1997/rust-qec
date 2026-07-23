# Issue 525 Adaptive Syndrome Codec Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the canonical dense/sparse adaptive syndrome codec for `rsmp v1`.

**Architecture:** Keep raw syndrome selection, ULEB128, and raw decode helpers in a new `rstim/src/sample_archive/syndrome.rs` module. `writer.rs` uses the helper to select and materialize one syndrome raw candidate before the existing bounded one-frame Zstandard compression path. `reader.rs` decodes the declared frame, validates the chosen raw codec before transform reconstruction, and hashes canonical dense syndrome bytes for the block logical digest.

**Tech Stack:** Rust 2024, existing `BitTable`, existing `sample_archive::format` constants and errors, existing `zstd_frame::{compress_frame,decompress_frame}`, `sha2`.

## Global Constraints

- Dense wire order is `shot = 0..S`, then `selected_detector = 0..R`; bit `i` is stored in bit `i % 8` of byte `i / 8`, least-significant bit first.
- Dense has no per-shot padding; only unused high bits of the final byte are padding and they must be zero.
- Dense raw length is `ceil(R * S / 8)` using checked arithmetic.
- Sparse writes, for each shot, canonical shortest-form ULEB128 hit count followed by canonical shortest-form ULEB128 hit deltas; the first value is the first detector index and later values are `current - previous - 1`.
- Sparse decode performs checked addition and rejects any count or reconstructed index outside `0..R`.
- `R == 0` or `S == 0` uses the canonical zero-length empty stream representation in the existing block format.
- Selection compares exact checked raw byte lengths before compression, and dense wins ties.
- The encoder materializes only the selected raw syndrome candidate.
- The writer compresses exactly one selected syndrome raw payload as one Zstandard frame with pledged content size and content checksum, using the existing bounded frame API.
- Malformed canonical structure, incomplete sparse records, declared-versus-decoded raw-length disagreement, and nonzero dense padding map to `RSMP_MALFORMED_ARCHIVE`.
- EOF before a declared compressed frame slice maps to `RSMP_TRUNCATED`.
- Zstandard frame, decode, or frame-checksum failure maps to `RSMP_DECOMPRESSION_FAILED`.
- Checked shape arithmetic or configured size limits map to `RSMP_LIMIT_EXCEEDED`.
- The verification command must print exactly `PASS rsmp adaptive codec known_cases=3 uleb_boundaries=4 malformed_cases=11 property_cases=4096 max_materialized_candidates=1`.

---

### Task 1: Add the Adaptive Codec Verification Test

**Files:**
- Create: `rstim/tests/rsmp_adaptive_codec.rs`

**Interfaces:**
- Consumes: planned public hidden helpers in `rstim::sample_archive::syndrome`.
- Produces: the failing integration test that defines all issue #525 acceptance criteria.

- [ ] **Step 1: Write the failing integration test**

Create `rstim/tests/rsmp_adaptive_codec.rs` with one top-level test:

```rust
#[test]
fn rsmp_adaptive_codec_contract() {
    let known_cases = verify_known_cases();
    assert_eq!(known_cases, 3);
    let uleb_boundaries = verify_uleb_boundaries();
    assert_eq!(uleb_boundaries, 4);
    let malformed_cases = verify_malformed_cases();
    assert_eq!(malformed_cases, 11);
    let property_cases = verify_property_cases();
    assert_eq!(property_cases, 4096);
    let max_materialized_candidates = max_materialized_candidates();
    assert_eq!(max_materialized_candidates, 1);
    println!("PASS rsmp adaptive codec known_cases=3 uleb_boundaries=4 malformed_cases=11 property_cases=4096 max_materialized_candidates=1");
}
```

The helper functions must assert:

```rust
// known selection/raw-byte cases
// R=12000,S=1 all-zero selects sparse.
// R=8,S=1 all-one selects dense.
// R=8,S=1 all-zero has equal one-byte candidates and selects dense.
// R=3,S=3 shots [101], [010], [110] encodes dense bytes d5 00.
// R=200,S=1 hits 0,128,199 encodes sparse bytes 03 00 7f 46.

// ULEB boundaries
// Single-hit indices 127, 128, 16383, and 16384 round-trip through sparse.

// malformed controls
// noncanonical ULEB zero; unterminated ULEB; ULEB overflow; count greater
// than R; incomplete hit list; checked delta-addition overflow;
// reconstructed index equal to R; fewer than S shot records; bytes after
// the Sth shot; declared raw-length mismatch; nonzero dense final padding.

// deterministic properties
// exactly 4096 generated tables round-trip through encode/decode, including
// zero rows, zero shots, non-byte-aligned dimensions, sparse selections, and
// dense selections.
```

- [ ] **Step 2: Run the test to verify RED**

Run:

```bash
cargo test --locked -p rstim --test rsmp_adaptive_codec -- --nocapture
```

Expected: FAIL because `rstim::sample_archive::syndrome` and its helpers do not exist.

- [ ] **Step 3: Commit is not allowed in this task**

Do not commit the failing test by itself. Continue to Task 2 and make it green before committing implementation code.

### Task 2: Implement Raw Syndrome Codec Helpers

**Files:**
- Create: `rstim/src/sample_archive/syndrome.rs`
- Modify: `rstim/src/sample_archive/mod.rs`

**Interfaces:**
- Consumes: `BitTable`, `SampleArchiveError`, `SampleArchiveErrorCode`, and stream codec constants.
- Produces:
  - `pub struct SyndromeEncoding { pub codec_id: u16, pub raw_len: u64, pub raw: Vec<u8>, pub dense_len: u64, pub sparse_len: u64 }`
  - `pub fn encode_syndrome(table: &BitTable) -> Result<SyndromeEncoding, SampleArchiveError>`
  - `pub fn decode_syndrome_raw(codec_id: u16, declared_raw_len: u64, raw: &[u8], rows: usize, shots: usize) -> Result<BitTable, SampleArchiveError>`
  - `pub fn for_each_sparse_syndrome_hit(raw: &[u8], declared_raw_len: u64, rows: u64, shots: u64, on_hit: impl FnMut(u64, u64)) -> Result<(), SampleArchiveError>`
  - `pub fn reset_materialization_telemetry()`
  - `pub fn max_materialized_candidates() -> usize`
  - `pub(crate) fn update_dense_syndrome_hash(table: &BitTable, hasher: &mut Sha256) -> Result<(), SampleArchiveError>`

- [ ] **Step 1: Implement length planning without allocation**

Add `checked_sparse_len(table: &BitTable) -> Result<u64, SampleArchiveError>` that walks shots and rows, adds ULEB byte lengths with `checked_add`, and returns `RSMP_LIMIT_EXCEEDED` on arithmetic overflow.

- [ ] **Step 2: Implement candidate materialization with telemetry**

Add a small atomic counter that resets at the beginning of `encode_syndrome`, increments when a complete raw dense or sparse candidate is materialized, and records the maximum materializations observed in one encode call. `encode_syndrome` must call only the selected materializer.

- [ ] **Step 3: Implement canonical ULEB helpers**

Encode ULEB128 with shortest-form bytes. Decode rejects `80 00` for zero, unterminated encodings, and values that overflow `u64`, returning `RSMP_MALFORMED_ARCHIVE`.

- [ ] **Step 4: Implement sparse validation before reconstruction callbacks**

`for_each_sparse_syndrome_hit` first validates the entire raw byte stream with no callback invocations, then replays the validated records and invokes `on_hit(shot, detector)`. This makes every malformed control fail before reconstruction work starts.

- [ ] **Step 5: Run the adaptive test to verify GREEN for raw helpers**

Run:

```bash
cargo test --locked -p rstim --test rsmp_adaptive_codec -- --nocapture
```

Expected: remaining failures only where archive writer/reader still lack integration.

### Task 3: Integrate Syndrome Selection With Archive Writer and Reader

**Files:**
- Modify: `rstim/src/sample_archive/writer.rs`
- Modify: `rstim/src/sample_archive/reader.rs`
- Modify: `rstim/tests/rsmp_archive_dense.rs`

**Interfaces:**
- Consumes: `encode_syndrome`, `decode_syndrome_raw`, and `update_dense_syndrome_hash`.
- Produces: adaptive syndrome codec IDs and raw lengths in archive block headers, and reader support for dense, sparse, and empty syndrome streams.

- [ ] **Step 1: Update writer selection**

Replace dense-only syndrome packing with:

```rust
let syndrome = encode_syndrome(&encoded.selected_detectors)?;
validate_decompressed_streams(syndrome.raw_len, free_len, self.limits)?;
let free = pack_dense(&encoded.free_measurements)?;
let mut logical_hasher = Sha256::new();
update_dense_syndrome_hash(&encoded.selected_detectors, &mut logical_hasher)?;
logical_hasher.update(&free);
```

Set `block_header.syndrome_codec_id = syndrome.codec_id`, `syndrome_uncompressed_len = syndrome.raw_len`, and compress/write only `syndrome.raw`.

- [ ] **Step 2: Update reader dispatch**

Remove the sparse-unsupported guard. After bounded frame decompression, call:

```rust
let selected = decode_syndrome_raw(
    block.syndrome_codec_id,
    block.syndrome_uncompressed_len,
    &syndrome,
    self.transform.rank(),
    shot_count,
)?;
```

Compute the logical digest with `update_dense_syndrome_hash(&selected, &mut logical_hasher)` instead of hashing sparse raw bytes when sparse was selected.

- [ ] **Step 3: Update the dense archive regression**

The old #523 negative that marked sparse syndrome as unsupported should now expect `RSMP_MALFORMED_ARCHIVE` when a block header is changed to sparse without changing the dense payload into valid sparse payload.

- [ ] **Step 4: Run focused verification**

Run:

```bash
cargo test --locked -p rstim --test rsmp_adaptive_codec -- --nocapture
cargo test --locked -p rstim --test rsmp_archive_dense -- --nocapture
```

Expected: both commands exit 0, and the adaptive command prints the exact required PASS line once.

- [ ] **Step 5: Commit implementation**

Run:

```bash
git add rstim/src/sample_archive rstim/tests/rsmp_adaptive_codec.rs rstim/tests/rsmp_archive_dense.rs
git commit -m "feat: add adaptive rsmp syndrome codec"
```
