# Issue 532 rsmp v1 Compatibility Fixture Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Commit one immutable two-block `rsmp v1` archive and a reader-only compatibility test that pins its provenance and decoded outputs.

**Architecture:** The committed fixture is data-first: human-readable circuit and expected `01` files, exact measurement `b8` input, exact archive bytes, and a TOML manifest with every digest. The integration test parses the manifest and catalog, validates archive structure and hashes, then decodes with `SampleArchiveReader`; it never constructs `SampleArchiveWriter`.

**Tech Stack:** Rust 2024 integration tests, `rstim::sample_archive`, `rstim::measurement_transform`, `rstim::output`, `sha2`, `toml`, `serde_json`, and `tempfile`.

## Global Constraints

- Fixture ID: `compat_v1_two_block_sparse_dense`.
- Fixture directory: `rstim/tests/fixtures/rsmp/v1/`.
- Required files: `compat.stim`, `compat-measurements.01`, `compat-measurements.b8`, `compat-v1.rsmp`, `compat-expected-detectors.01`, `compat-expected-observables.01`, `manifest.toml`, and `README.md`.
- Circuit canonical SHA-256: `18a857fb71f44eb28144a6a0e3aad17cce675daa2737690432efe305ed5777a2`.
- Shape: `M=10`, `D=9`, `L=1`, `rank=9`, `free_measurements=1`, `shots=4`, `blocks=2`, `block_shots=2`.
- Measurement shots: `1000000000`, `0111111111`, `1101010101`, `0010101010`.
- Expected detector shots: `000000000`, `000000000`, `111111111`, `111111111`.
- Expected observable shots: `0`, `1`, `1`, `0`.
- Syndrome codec order: block 0 sparse, block 1 dense.
- Compatibility test success line: `PASS rsmp v1 compatibility fixtures=1 blocks=2 codecs=sparse,dense`.
- The compatibility test must not call `SampleArchiveWriter`, pack_samples, or any archive regeneration helper.
- Fixture maintenance is append-only after merge; existing bytes and decoded hashes are not updated merely to make a reader failure pass.

---

### Task 1: Fixture Bytes And Provenance

**Files:**
- Create: `rstim/tests/fixtures/rsmp/v1/compat.stim`
- Create: `rstim/tests/fixtures/rsmp/v1/compat-measurements.01`
- Create: `rstim/tests/fixtures/rsmp/v1/compat-measurements.b8`
- Create: `rstim/tests/fixtures/rsmp/v1/compat-expected-detectors.01`
- Create: `rstim/tests/fixtures/rsmp/v1/compat-expected-observables.01`
- Create: `rstim/tests/fixtures/rsmp/v1/compat-v1.rsmp`
- Create: `rstim/tests/fixtures/rsmp/v1/manifest.toml`
- Create: `rstim/tests/fixtures/rsmp/v1/README.md`

**Interfaces:**
- Consumes: public `SampleArchiveWriter` only in a one-time generation command outside the compatibility test path.
- Produces: committed fixture files and manifest fields consumed by Task 2.

- [ ] **Step 1: Add reviewable text fixtures**

Write `compat.stim` with the exact circuit from #532. Write `compat-measurements.01` with the four measurement shots, `compat-expected-detectors.01` with the four detector shots, and `compat-expected-observables.01` with the four observable shots. Each text file ends with a newline.

- [ ] **Step 2: Add exact measurement b8 bytes**

Write `compat-measurements.b8` as these eight bytes, one two-byte row per 10-bit shot:

```text
01 00 fe 03 ab 02 54 01
```

- [ ] **Step 3: Generate the one immutable archive**

Use a temporary helper outside the compatibility test path that constructs `MeasurementTransform::from_circuit`, sets `ArchiveLimits::default().transform.max_shots_per_block = 2`, reads `compat-measurements.b8` with `read_shots_b8`, and writes `compat-v1.rsmp` with `SampleArchiveWriter` and `SampleArchiveOptions::default()`. Record the exact argv in `manifest.toml`.

- [ ] **Step 4: Compute manifest values**

Run:

```bash
shasum -a 256 Cargo.lock rstim/tests/fixtures/rsmp/v1/compat.stim rstim/tests/fixtures/rsmp/v1/compat-measurements.01 rstim/tests/fixtures/rsmp/v1/compat-measurements.b8 rstim/tests/fixtures/rsmp/v1/compat-v1.rsmp rstim/tests/fixtures/rsmp/v1/compat-expected-detectors.01 rstim/tests/fixtures/rsmp/v1/compat-expected-observables.01
```

Parse `compat-v1.rsmp` block headers to record each block's codec IDs, compressed and uncompressed stream lengths, and `logical_payload_sha256`. Record the internal trailer `archive_sha256` separately from the whole-file SHA-256.

- [ ] **Step 5: Write manifest and policy README**

`manifest.toml` records schema version, fixture ID, repo-relative POSIX paths, all source/archive/expected-output SHA-256 values, canonical circuit hash, shape, format identifiers, block codec IDs, logical payload digests, whole archive SHA-256, trailer archive digest, exact generation argv, generator revision, `Cargo.lock` SHA-256, Zstandard crate version, intended consumers, and the statement that the generation command records provenance but future writers need not reproduce identical bytes. `README.md` documents the fixture purpose, source vectors, and append-only policy.

- [ ] **Step 6: Commit task 1**

Run:

```bash
git add rstim/tests/fixtures/rsmp/v1
git commit -m "test: pin rsmp v1 compatibility fixture"
```

### Task 2: Catalog Entry

**Files:**
- Modify: `rstim/tests/fixtures/rsmp/catalog.json`

**Interfaces:**
- Consumes: manifest/archive hashes from Task 1.
- Produces: one additive catalog case used by the compatibility test and future corruption corpus work.

- [ ] **Step 1: Add catalog case**

Append a case with ID `compat_v1_two_block_sparse_dense`, purpose `Immutable two-block rsmp v1 reader compatibility fixture.`, semantic role `v1_reader_compatibility`, source circuit path/hash, measurement input path/hash, shape values, and consumers `compatibility`, `corruption_corpus`, `cli_publication_tests`, and `readiness`.

- [ ] **Step 2: Pin archive and manifest references**

Add an `rsmp_v1_compatibility` object in that case with:

```json
{
  "manifest_path": "rstim/tests/fixtures/rsmp/v1/manifest.toml",
  "manifest_sha256": "the lowercase SHA-256 printed by `shasum -a 256 rstim/tests/fixtures/rsmp/v1/manifest.toml` after the manifest is final",
  "archive_path": "rstim/tests/fixtures/rsmp/v1/compat-v1.rsmp",
  "archive_sha256": "the lowercase SHA-256 printed by `shasum -a 256 rstim/tests/fixtures/rsmp/v1/compat-v1.rsmp`",
  "block_shots": 2,
  "blocks": 2,
  "syndrome_codecs": ["sparse", "dense"]
}
```

- [ ] **Step 3: Verify existing catalog checker**

Run:

```bash
python3 tools/check_rsmp_fixture_catalog.py
```

Expected final line remains:

```text
PASS rsmp fixture catalog valid_cases=7 known_answers=4 benchmark_cases=1 corruption_recipes>=12
```

- [ ] **Step 4: Commit task 2**

Run:

```bash
git add rstim/tests/fixtures/rsmp/catalog.json
git commit -m "test: register rsmp v1 compatibility fixture"
```

### Task 3: Reader-Only Compatibility Test

**Files:**
- Create: `rstim/tests/rsmp_v1_compatibility.rs`

**Interfaces:**
- Consumes: Task 1 fixture files and Task 2 catalog entry.
- Produces: positive compatibility verification and two negative controls.

- [ ] **Step 1: Write failing tests**

Create three integration tests:

```rust
#[test]
fn rsmp_v1_compatibility_fixture_decodes() {
    let report = verify_fixture(repo_root(), repo_root()).expect("compatibility fixture");
    assert_eq!(report.success_line, "PASS rsmp v1 compatibility fixtures=1 blocks=2 codecs=sparse,dense");
    println!("{}", report.success_line);
}

#[test]
fn changed_archive_payload_byte_is_rejected() {
    let temp = copied_fixture_tree();
    flip_first_compressed_stream_byte(&temp.archive_path);
    assert!(verify_fixture(temp.root.path(), temp.root.path()).is_err());
}

#[test]
fn changed_expected_measurement_hash_is_rejected() {
    let temp = copied_fixture_tree();
    rewrite_manifest_measurement_sha(&temp.manifest_path, "0000000000000000000000000000000000000000000000000000000000000000");
    assert!(verify_fixture(temp.root.path(), temp.root.path()).is_err());
}
```

Run:

```bash
cargo test --locked -p rstim --test rsmp_v1_compatibility -- --nocapture
```

Expected: FAIL before helper implementation because `verify_fixture` and fixture parsing do not exist.

- [ ] **Step 2: Implement manifest and catalog validation helpers**

Implement helpers that load TOML and JSON, require exact scalar/string/list fields, reject absolute or parent-traversing repo paths, compute SHA-256, and verify the catalog case's manifest/archive hashes match the committed files.

- [ ] **Step 3: Implement archive structure checks**

Parse `GlobalHeader`, every `BlockHeader`, and `ArchiveTrailer` from `compat-v1.rsmp`. Check v1 format constants, transform identifiers, `M/D/L/rank`, total shots, block count, block order, `first_shot`, `shot_count`, codec IDs, stream lengths, block logical-payload digests, trailer archive digest, and whole-file SHA-256 against the manifest.

- [ ] **Step 4: Implement reader decode checks**

Read `compat.stim` with `parse_lines`, serialize it with `circuit_to_string`, and verify the canonical SHA-256. Decode `compat-v1.rsmp` with `SampleArchiveReader`, stitch block tables, call `finish`, and compare decoded measurements, detectors, and observable flips bit-by-bit against `compat-measurements.01`, `compat-expected-detectors.01`, and `compat-expected-observables.01`.

- [ ] **Step 5: Implement negative-control copy and mutation helpers**

Copy only `rstim/tests/fixtures/rsmp/catalog.json` and `rstim/tests/fixtures/rsmp/v1/*` into a `tempfile::TempDir` preserving repo-relative paths. Flip a byte inside a compressed stream after a block header, and rewrite only the temporary manifest hash for the measurement `01` file.

- [ ] **Step 6: Verify focused commands**

Run:

```bash
cargo test --locked -p rstim --test rsmp_v1_compatibility -- --nocapture
cargo test --locked -p rstim --test rsmp_v1_compatibility changed_archive_payload_byte_is_rejected -- --exact --nocapture
cargo test --locked -p rstim --test rsmp_v1_compatibility changed_expected_measurement_hash_is_rejected -- --exact --nocapture
```

The first command exits 0 and prints exactly one success line.

- [ ] **Step 7: Commit task 3**

Run:

```bash
git add rstim/tests/rsmp_v1_compatibility.rs
git commit -m "test: verify rsmp v1 compatibility fixture"
```
