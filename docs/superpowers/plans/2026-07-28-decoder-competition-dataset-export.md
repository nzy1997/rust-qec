# Decoder Competition Dataset Export Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `rstim export_decoder_dataset` so organizers can publish either detector-event datasets or logically blinded measurement datasets while keeping per-shot answers private.

**Architecture:** Keep RSMP unchanged and add a separate `decoder_dataset` module for policy, validation, sampling, manifests, and directory publication. Keep `cli.rs` thin: clap parses the new command, reads the circuit file, parses `--logical_x_qubits`, and calls the exporter.

**Tech Stack:** Rust 2024, clap, rand 0.8 `StdRng`, serde/serde_json, sha2 `Sha256`, existing `BitTable`, `sample_batch_with_options`, `measurements_to_detections`, `write_shots_b8`, and CLI integration tests using `tempfile`.

## Global Constraints

- The new command is exactly `rstim export_decoder_dataset`.
- Required CLI flags are `--circuit`, `--shots`, `--mode`, `--public_out`, and `--private_out`.
- Supported modes are exactly `detectors` and `measurements_blinded`.
- `measurements_blinded` requires `--logical_x_qubits`.
- `detectors` rejects `--logical_x_qubits`.
- `--shots` must be positive.
- No stdin or stdout support is added for this command.
- Public output directory contains exactly `manifest.json`, `circuit.stim`, and `shots.b8`.
- Private output directory contains exactly `manifest.json`, `answers.b8`, and only for `measurements_blinded`, `masks.b8`.
- The first version supports exactly one observable.
- Circuits with zero observables, multiple observables, or sweep bits are rejected.
- `measurements_blinded` requires exactly one standalone top-level marker line: `# RSTIM_LOGICAL_FLIP_POINT`.
- The injected private producer operation is one ideal `X` instruction immediately after the marker.
- The public circuit is the unmodified logical-zero circuit text supplied by the organizer.
- Logical validation requires `D_public(m_ref_0) = D_public(m_ref_1)` and `O_public(m_ref_0) XOR O_public(m_ref_1) = 1`.
- Without `--seed`, generation uses OS randomness; with `--seed`, output bytes are deterministic.
- Public manifests never contain the seed, masks, answers, private paths, producer-circuit label, or row permutation.
- The public directory is renamed last and is the publication commit point.
- Focused verification command is `cargo test --locked -p rstim --test cli_decoder_dataset -- --nocapture`.
- Full verification command is `cargo test --locked -p rstim`.

---

## File Structure

- Create `rstim/src/decoder_dataset.rs`: owns dataset modes, public/private manifest structs, logical marker scanning, logical support validation, RNG domain separation, artifact generation, b8 byte construction, SHA-256 digests, staged directory publication, and exporter-level tests.
- Modify `rstim/src/lib.rs`: expose `pub mod decoder_dataset;`.
- Modify `rstim/src/cli.rs`: add the `export_decoder_dataset` clap subcommand and a `run_export_decoder_dataset` wrapper.
- Create `rstim/tests/cli_decoder_dataset.rs`: black-box CLI tests for public/private directory contracts, deterministic seeds, rejection behavior, publication failure injection, and public-manifest leakage scanning.
- Modify `rstim/doc/rsmp-cli.md`: document that RSMP remains lossless/private and point competition users to `export_decoder_dataset`.
- Create `rstim/doc/decoder-dataset.md`: document the public and private bundle contract, command examples, marker placement, and scoring semantics.

---

### Task 1: Core Dataset Types And Manifest Contract

**Files:**
- Create: `rstim/src/decoder_dataset.rs`
- Modify: `rstim/src/lib.rs`
- Test: `rstim/src/decoder_dataset.rs`

**Interfaces:**
- Consumes: `crate::sim::bit_table::BitTable`, `crate::output::write_shots_b8`, `serde`, `serde_json`, `sha2`.
- Produces:
  - `pub const LOGICAL_FLIP_MARKER: &str`
  - `pub enum DecoderDatasetMode`
  - `pub struct ExportDecoderDatasetConfig`
  - `pub struct DecoderDatasetSummary`
  - `#[doc(hidden)] pub fn dataset_id_material(...) -> Vec<u8>`
  - `#[doc(hidden)] pub fn sha256_hex(bytes: &[u8]) -> String`
  - `#[doc(hidden)] pub fn bit_table_to_b8_bytes(table: &BitTable) -> Result<Vec<u8>, String>`
  - `pub fn export_decoder_dataset(config: ExportDecoderDatasetConfig) -> Result<DecoderDatasetSummary, String>`

- [ ] **Step 1: Write RED tests for stable b8 bytes and dataset IDs**

Add this unit-test module to the new file:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::bit_table::BitTable;

    #[test]
    fn b8_bytes_are_lsb_first_and_zero_padded() {
        let mut table = BitTable::new(10, 2);
        table.set(0, 0, true);
        table.set(7, 0, true);
        table.set(9, 0, true);
        table.set(1, 1, true);
        table.set(8, 1, true);

        assert_eq!(bit_table_to_b8_bytes(&table).unwrap(), vec![0b1000_0001, 0b0000_0010, 0b0000_0010, 0b0000_0001]);
    }

    #[test]
    fn dataset_id_uses_only_public_material() {
        let left = dataset_id_material(
            1,
            DecoderDatasetMode::Detectors,
            "circuit-a",
            3,
            5,
            "shots-a",
        );
        let right = dataset_id_material(
            1,
            DecoderDatasetMode::Detectors,
            "circuit-a",
            3,
            5,
            "shots-a",
        );
        let changed_seed_would_not_be_an_argument = dataset_id_material(
            1,
            DecoderDatasetMode::Detectors,
            "circuit-a",
            3,
            5,
            "shots-b",
        );

        assert_eq!(left, right);
        assert_ne!(left, changed_seed_would_not_be_an_argument);
        assert!(!String::from_utf8(left).unwrap().contains("seed"));
    }
}
```

- [ ] **Step 2: Run the focused unit tests and verify RED**

Run:

```bash
cargo test --locked -p rstim decoder_dataset::tests::b8_bytes_are_lsb_first_and_zero_padded -- --exact
cargo test --locked -p rstim decoder_dataset::tests::dataset_id_uses_only_public_material -- --exact
```

Expected: FAIL because `decoder_dataset` does not exist.

- [ ] **Step 3: Add module shell, mode parsing, b8 helper, and digest helper**

Add this public shell:

```rust
use crate::sim::bit_table::BitTable;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::PathBuf;

pub const LOGICAL_FLIP_MARKER: &str = "# RSTIM_LOGICAL_FLIP_POINT";
const PUBLIC_SCHEMA_VERSION: u32 = 1;
const DATASET_FORMAT: &str = "rstim_decoder_dataset";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DecoderDatasetMode {
    Detectors,
    MeasurementsBlinded,
}

impl DecoderDatasetMode {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "detectors" => Ok(Self::Detectors),
            "measurements_blinded" => Ok(Self::MeasurementsBlinded),
            other => Err(format!("unknown decoder dataset mode: {other}")),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Detectors => "detectors",
            Self::MeasurementsBlinded => "measurements_blinded",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ExportDecoderDatasetConfig {
    pub circuit_text: String,
    pub shots: usize,
    pub mode: DecoderDatasetMode,
    pub logical_x_qubits: Vec<u32>,
    pub public_out: PathBuf,
    pub private_out: PathBuf,
    pub seed: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecoderDatasetSummary {
    pub dataset_id: String,
    pub mode: DecoderDatasetMode,
    pub shots: usize,
    pub row_bits: usize,
    pub public_out: PathBuf,
    pub private_out: PathBuf,
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write as _;
        write!(&mut out, "{byte:02x}").expect("write hex into String");
    }
    out
}

pub fn bit_table_to_b8_bytes(table: &BitTable) -> Result<Vec<u8>, String> {
    let bytes_per_shot = table
        .num_major()
        .checked_add(7)
        .ok_or_else(|| "b8 row width overflows".to_string())?
        / 8;
    let total = bytes_per_shot
        .checked_mul(table.num_minor())
        .ok_or_else(|| "b8 output size overflows".to_string())?;
    let mut bytes = Vec::with_capacity(total);
    crate::output::write_shots_b8(table, &mut bytes).map_err(|error| format!("write error: {error}"))?;
    Ok(bytes)
}

pub fn dataset_id_material(
    schema_version: u32,
    mode: DecoderDatasetMode,
    circuit_sha256: &str,
    shots: usize,
    row_bits: usize,
    shots_b8_sha256: &str,
) -> Vec<u8> {
    format!(
        "format={DATASET_FORMAT}\nschema_version={schema_version}\nmode={}\ncircuit_sha256={circuit_sha256}\nshots={shots}\nrow_bits={row_bits}\nshots_b8_sha256={shots_b8_sha256}\n",
        mode.as_str(),
    )
    .into_bytes()
}
```

Add to `rstim/src/lib.rs`:

```rust
pub mod decoder_dataset;
```

- [ ] **Step 4: Add manifest structs with explicit public/private separation**

Add serializable structs whose field names match the JSON contract:

```rust
#[derive(Debug, Serialize)]
struct PublicManifest {
    format: &'static str,
    schema_version: u32,
    dataset_id: String,
    mode: DecoderDatasetMode,
    shots: usize,
    row: PublicRowManifest,
    circuit: CircuitManifest,
    shots_file: FileManifest,
}

#[derive(Debug, Serialize)]
struct PrivateManifest {
    format: &'static str,
    schema_version: u32,
    dataset_id: String,
    mode: DecoderDatasetMode,
    shots: usize,
    answers_file: FileManifest,
    #[serde(skip_serializing_if = "Option::is_none")]
    masks_file: Option<FileManifest>,
    generation: PrivateGenerationManifest,
}

#[derive(Debug, Serialize)]
struct PublicRowManifest {
    kind: &'static str,
    bits: usize,
    encoding: &'static str,
    bit_order: &'static str,
    bytes_per_shot: usize,
}

#[derive(Debug, Serialize)]
struct CircuitManifest {
    file: &'static str,
    sha256: String,
    measurements: usize,
    detectors: usize,
    observables: usize,
    sweep_bits: usize,
}

#[derive(Debug, Serialize)]
struct FileManifest {
    file: &'static str,
    sha256: String,
    bits: usize,
    bytes_per_shot: usize,
}

#[derive(Debug, Serialize)]
struct PrivateGenerationManifest {
    rstim_version: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    seed: Option<u64>,
}
```

- [ ] **Step 5: Add stub exporter**

Add a temporary stub that compiles and returns a clear error:

```rust
pub fn export_decoder_dataset(
    _config: ExportDecoderDatasetConfig,
) -> Result<DecoderDatasetSummary, String> {
    Err("export_decoder_dataset is not implemented".to_string())
}
```

- [ ] **Step 6: Run Task 1 tests and verify GREEN**

Run:

```bash
cargo test --locked -p rstim decoder_dataset::tests::b8_bytes_are_lsb_first_and_zero_padded -- --exact
cargo test --locked -p rstim decoder_dataset::tests::dataset_id_uses_only_public_material -- --exact
```

Expected: PASS.

- [ ] **Step 7: Commit Task 1**

Run:

```bash
git add rstim/src/lib.rs rstim/src/decoder_dataset.rs
git commit -m "feat: add decoder dataset manifest types"
```

---

### Task 2: Preflight, Marker Injection, And Logical Validation

**Files:**
- Modify: `rstim/src/decoder_dataset.rs`
- Test: `rstim/src/decoder_dataset.rs`

**Interfaces:**
- Consumes: Task 1 types, `crate::parser::parse_lines`, `crate::stats::summarize`, `crate::data_path::build_reference_sample`, `crate::m2d::measurements_to_detections`.
- Produces:
  - `#[doc(hidden)] pub fn parse_logical_x_qubits(value: &str) -> Result<Vec<u32>, String>`
  - `#[doc(hidden)] pub fn circuit_with_injected_logical_x(circuit_text: &str, logical_x_qubits: &[u32]) -> Result<String, String>`
  - `#[doc(hidden)] pub fn validate_decoder_dataset_inputs(config: &ExportDecoderDatasetConfig) -> Result<ValidatedDecoderDatasetInput, String>`

- [ ] **Step 1: Write RED tests for option and circuit rejection**

Add these tests:

```rust
#[test]
fn parse_logical_x_qubits_rejects_empty_duplicate_and_bad_tokens() {
    assert_eq!(parse_logical_x_qubits("0,2,4").unwrap(), vec![0, 2, 4]);
    assert!(parse_logical_x_qubits("").unwrap_err().contains("non-empty"));
    assert!(parse_logical_x_qubits("0,2,2").unwrap_err().contains("duplicate"));
    assert!(parse_logical_x_qubits("0,nope").unwrap_err().contains("invalid"));
}

#[test]
fn marker_must_be_unique_standalone_and_top_level() {
    let good = "R 0\n# RSTIM_LOGICAL_FLIP_POINT\nM 0\nOBSERVABLE_INCLUDE(0) rec[-1]\n";
    assert!(circuit_with_injected_logical_x(good, &[0]).unwrap().contains("\nX 0\n"));

    let missing = "R 0\nM 0\nOBSERVABLE_INCLUDE(0) rec[-1]\n";
    assert!(circuit_with_injected_logical_x(missing, &[0]).unwrap_err().contains("marker"));

    let duplicate = "R 0\n# RSTIM_LOGICAL_FLIP_POINT\n# RSTIM_LOGICAL_FLIP_POINT\nM 0\nOBSERVABLE_INCLUDE(0) rec[-1]\n";
    assert!(circuit_with_injected_logical_x(duplicate, &[0]).unwrap_err().contains("exactly once"));

    let nested = "R 0\nREPEAT 2 {\n# RSTIM_LOGICAL_FLIP_POINT\nM 0\n}\nOBSERVABLE_INCLUDE(0) rec[-1]\n";
    assert!(circuit_with_injected_logical_x(nested, &[0]).unwrap_err().contains("top-level"));

    let inline = "R 0 # RSTIM_LOGICAL_FLIP_POINT\nM 0\nOBSERVABLE_INCLUDE(0) rec[-1]\n";
    assert!(circuit_with_injected_logical_x(inline, &[0]).unwrap_err().contains("standalone"));
}

#[test]
fn logical_validation_requires_observable_flip_without_detector_change() {
    let valid = "R 0\n# RSTIM_LOGICAL_FLIP_POINT\nM 0\nOBSERVABLE_INCLUDE(0) rec[-1]\n";
    let config = test_config(valid, DecoderDatasetMode::MeasurementsBlinded, vec![0]);
    assert!(validate_decoder_dataset_inputs(&config).is_ok());

    let no_flip = "R 0\n# RSTIM_LOGICAL_FLIP_POINT\nM 0\nOBSERVABLE_INCLUDE(0) rec[-1]\n";
    let config = test_config(no_flip, DecoderDatasetMode::MeasurementsBlinded, vec![]);
    assert!(validate_decoder_dataset_inputs(&config).unwrap_err().contains("--logical_x_qubits"));

    let changes_detector = "R 0 1\n# RSTIM_LOGICAL_FLIP_POINT\nM 0 1\nDETECTOR rec[-2] rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-2]\n";
    let config = test_config(changes_detector, DecoderDatasetMode::MeasurementsBlinded, vec![0]);
    assert!(validate_decoder_dataset_inputs(&config).unwrap_err().contains("changes detector"));
}
```

Add this helper inside the test module:

```rust
fn test_config(
    circuit_text: &str,
    mode: DecoderDatasetMode,
    logical_x_qubits: Vec<u32>,
) -> ExportDecoderDatasetConfig {
    ExportDecoderDatasetConfig {
        circuit_text: circuit_text.to_string(),
        shots: 1,
        mode,
        logical_x_qubits,
        public_out: std::path::PathBuf::from("public-unused"),
        private_out: std::path::PathBuf::from("private-unused"),
        seed: Some(1),
    }
}
```

- [ ] **Step 2: Run focused tests and verify RED**

Run:

```bash
cargo test --locked -p rstim decoder_dataset::tests::parse_logical_x_qubits_rejects_empty_duplicate_and_bad_tokens -- --exact
cargo test --locked -p rstim decoder_dataset::tests::marker_must_be_unique_standalone_and_top_level -- --exact
cargo test --locked -p rstim decoder_dataset::tests::logical_validation_requires_observable_flip_without_detector_change -- --exact
```

Expected: FAIL because helpers do not exist.

- [ ] **Step 3: Implement logical qubit parsing and marker injection**

Implement `parse_logical_x_qubits` using `split(',')`, `trim()`, `u32::parse`, and `BTreeSet<u32>` duplicate detection. Return errors containing the flag name `--logical_x_qubits`.

Implement `circuit_with_injected_logical_x` by scanning the raw circuit text line-by-line before calling `parse_lines`:

```rust
fn marker_depth_before_line(line: &str, current_depth: usize) -> usize {
    let code = line.split('#').next().unwrap_or("").trim();
    if code == "}" {
        current_depth.saturating_sub(1)
    } else {
        current_depth
    }
}

fn marker_depth_after_line(line: &str, current_depth: usize) -> usize {
    let code = line.split('#').next().unwrap_or("").trim();
    if code.ends_with('{') {
        current_depth + 1
    } else if code == "}" {
        current_depth.saturating_sub(1)
    } else {
        current_depth
    }
}
```

Use those helpers only for marker detection. Let `parse_lines` remain the source of truth for Stim syntax.

- [ ] **Step 4: Implement validated input struct and preflight**

Add this private struct:

```rust
struct ValidatedDecoderDatasetInput {
    public_circuit_text: String,
    public_instrs: Vec<crate::ir::StimInstr>,
    private_one_circuit_text: Option<String>,
    private_one_instrs: Option<Vec<crate::ir::StimInstr>>,
    measurements: usize,
    detectors: usize,
    observables: usize,
}
```

`validate_decoder_dataset_inputs` must:

```rust
if config.shots == 0 {
    return Err("--shots must be positive".to_string());
}
let public_instrs = crate::parser::parse_lines(&config.circuit_text)?;
let stats = crate::stats::summarize(&public_instrs);
if stats.num_observables != 1 {
    return Err(format!("export_decoder_dataset requires exactly one observable, found {}", stats.num_observables));
}
if stats.num_sweep_bits != 0 {
    return Err("export_decoder_dataset does not support sweep-bit circuits".to_string());
}
match config.mode {
    DecoderDatasetMode::Detectors if !config.logical_x_qubits.is_empty() => {
        return Err("detectors mode rejects --logical_x_qubits".to_string());
    }
    DecoderDatasetMode::MeasurementsBlinded if config.logical_x_qubits.is_empty() => {
        return Err("measurements_blinded mode requires --logical_x_qubits".to_string());
    }
    _ => {}
}
```

For `measurements_blinded`, also reject qubits where `q as usize >= stats.num_qubits`, build the private circuit with `X`, parse it, and run reference validation.

- [ ] **Step 5: Implement reference validation**

Add helpers:

```rust
fn one_shot_measurement_table(bits: &[bool]) -> Result<BitTable, String> {
    let mut table = BitTable::try_new(bits.len(), 1)
        .map_err(|err| format!("BitTable allocation failed: {err:?}"))?;
    for (bit, value) in bits.iter().copied().enumerate() {
        if value {
            table.set(bit, 0, true);
        }
    }
    Ok(table)
}

fn validate_logical_x_effect(
    public_instrs: &[crate::ir::StimInstr],
    private_instrs: &[crate::ir::StimInstr],
) -> Result<(), String> {
    let m0 = crate::data_path::build_reference_sample(
        public_instrs,
        crate::data_path::ReferenceSampleMode::SimulateNoiseless,
    )?;
    let m1 = crate::data_path::build_reference_sample(
        private_instrs,
        crate::data_path::ReferenceSampleMode::SimulateNoiseless,
    )?;
    let t0 = one_shot_measurement_table(&m0)?;
    let t1 = one_shot_measurement_table(&m1)?;
    let out0 = crate::m2d::measurements_to_detections(public_instrs, &t0)?;
    let out1 = crate::m2d::measurements_to_detections(public_instrs, &t1)?;
    for detector in 0..out0.detections.num_major() {
        if out0.detections.get(detector, 0) != out1.detections.get(detector, 0) {
            return Err("injected logical X changes detector reference values".to_string());
        }
    }
    let flips = out0.observable_flips.get(0, 0) ^ out1.observable_flips.get(0, 0);
    if !flips {
        return Err("injected logical X does not flip observable 0".to_string());
    }
    Ok(())
}
```

- [ ] **Step 6: Run Task 2 tests and verify GREEN**

Run:

```bash
cargo test --locked -p rstim decoder_dataset::tests::parse_logical_x_qubits_rejects_empty_duplicate_and_bad_tokens -- --exact
cargo test --locked -p rstim decoder_dataset::tests::marker_must_be_unique_standalone_and_top_level -- --exact
cargo test --locked -p rstim decoder_dataset::tests::logical_validation_requires_observable_flip_without_detector_change -- --exact
```

Expected: PASS.

- [ ] **Step 7: Commit Task 2**

Run:

```bash
git add rstim/src/decoder_dataset.rs
git commit -m "feat: validate decoder dataset logical blinding inputs"
```

---

### Task 3: Deterministic Artifact Generation

**Files:**
- Modify: `rstim/src/decoder_dataset.rs`
- Test: `rstim/src/decoder_dataset.rs`

**Interfaces:**
- Consumes: Task 2 `ValidatedDecoderDatasetInput`, `rand::rngs::StdRng`, `rand::{Rng, SeedableRng}`, `sample_batch_with_options`, `SampleOptions`, `SampleOutputMode`, `measurements_to_detections`.
- Produces:
  - `#[doc(hidden)] pub struct DecoderDatasetArtifacts`
  - `#[doc(hidden)] pub fn generate_decoder_dataset_artifacts(config: &ExportDecoderDatasetConfig) -> Result<DecoderDatasetArtifacts, String>`

- [ ] **Step 1: Write RED tests for detector and blinded artifacts**

Add tests:

```rust
#[test]
fn detector_artifacts_publish_detections_and_private_answers() {
    let circuit = "R 0\nX_ERROR(1) 0\nM 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n";
    let config = test_config(circuit, DecoderDatasetMode::Detectors, vec![]);
    let artifacts = generate_decoder_dataset_artifacts(&config).unwrap();

    assert_eq!(artifacts.public_row_kind, "detectors");
    assert_eq!(artifacts.public_shots.num_major(), 1);
    assert_eq!(artifacts.answers.num_major(), 1);
    assert!(artifacts.public_shots.get(0, 0));
    assert!(artifacts.answers.get(0, 0));
    assert!(artifacts.masks.is_none());
}

#[test]
fn blinded_measurement_answers_are_public_observable_xor_mask() {
    let circuit = "R 0\n# RSTIM_LOGICAL_FLIP_POINT\nX_ERROR(0.5) 0\nM 0\nOBSERVABLE_INCLUDE(0) rec[-1]\n";
    let mut config = test_config(circuit, DecoderDatasetMode::MeasurementsBlinded, vec![0]);
    config.shots = 16;
    config.seed = Some(0xdec0_de01);

    let artifacts = generate_decoder_dataset_artifacts(&config).unwrap();
    let public_interpretation = crate::m2d::measurements_to_detections(
        &artifacts.public_instrs,
        &artifacts.public_shots,
    )
    .unwrap();
    let masks = artifacts.masks.as_ref().unwrap();

    let mut saw_zero = false;
    let mut saw_one = false;
    for shot in 0..config.shots {
        let recomputed = public_interpretation.observable_flips.get(0, shot) ^ masks.get(0, shot);
        assert_eq!(artifacts.answers.get(0, shot), recomputed);
        saw_zero |= !masks.get(0, shot);
        saw_one |= masks.get(0, shot);
    }
    assert!(saw_zero);
    assert!(saw_one);
}

#[test]
fn fixed_seed_reproduces_artifacts_byte_for_byte() {
    let circuit = "R 0\n# RSTIM_LOGICAL_FLIP_POINT\nX_ERROR(0.5) 0\nM 0\nOBSERVABLE_INCLUDE(0) rec[-1]\n";
    let mut config = test_config(circuit, DecoderDatasetMode::MeasurementsBlinded, vec![0]);
    config.shots = 32;
    config.seed = Some(123);

    let a = generate_decoder_dataset_artifacts(&config).unwrap();
    let b = generate_decoder_dataset_artifacts(&config).unwrap();
    assert_eq!(bit_table_to_b8_bytes(&a.public_shots).unwrap(), bit_table_to_b8_bytes(&b.public_shots).unwrap());
    assert_eq!(bit_table_to_b8_bytes(&a.answers).unwrap(), bit_table_to_b8_bytes(&b.answers).unwrap());
    assert_eq!(bit_table_to_b8_bytes(a.masks.as_ref().unwrap()).unwrap(), bit_table_to_b8_bytes(b.masks.as_ref().unwrap()).unwrap());
}
```

- [ ] **Step 2: Run focused tests and verify RED**

Run:

```bash
cargo test --locked -p rstim decoder_dataset::tests::detector_artifacts_publish_detections_and_private_answers -- --exact
cargo test --locked -p rstim decoder_dataset::tests::blinded_measurement_answers_are_public_observable_xor_mask -- --exact
cargo test --locked -p rstim decoder_dataset::tests::fixed_seed_reproduces_artifacts_byte_for_byte -- --exact
```

Expected: FAIL because artifact generation does not exist.

- [ ] **Step 3: Add artifact struct and domain-separated RNGs**

Add:

```rust
pub struct DecoderDatasetArtifacts {
    pub public_circuit_text: String,
    pub public_instrs: Vec<crate::ir::StimInstr>,
    pub public_row_kind: &'static str,
    pub public_shots: BitTable,
    pub answers: BitTable,
    pub masks: Option<BitTable>,
    pub measurements: usize,
    pub detectors: usize,
    pub observables: usize,
}

struct DatasetRngs {
    physical: rand::rngs::StdRng,
    mask: rand::rngs::StdRng,
    permutation: rand::rngs::StdRng,
}

fn make_dataset_rngs(seed: Option<u64>) -> DatasetRngs {
    match seed {
        Some(seed) => DatasetRngs {
            physical: domain_rng(seed, b"physical-sampling"),
            mask: domain_rng(seed, b"logical-mask"),
            permutation: domain_rng(seed, b"row-permutation"),
        },
        None => DatasetRngs {
            physical: rand::rngs::StdRng::from_entropy(),
            mask: rand::rngs::StdRng::from_entropy(),
            permutation: rand::rngs::StdRng::from_entropy(),
        },
    }
}

fn domain_rng(seed: u64, domain: &[u8]) -> rand::rngs::StdRng {
    let mut hasher = Sha256::new();
    hasher.update(b"rstim-decoder-dataset-v1\n");
    hasher.update(domain);
    hasher.update(b"\n");
    hasher.update(seed.to_le_bytes());
    rand::rngs::StdRng::from_seed(hasher.finalize().into())
}
```

- [ ] **Step 4: Implement detector mode artifacts**

Use:

```rust
let result = crate::sampler::sample_batch_with_options(
    &validated.public_instrs,
    config.shots,
    &mut rngs.physical,
    crate::sampler::SampleOptions {
        output_mode: crate::sampler::SampleOutputMode::Full,
        ..crate::sampler::SampleOptions::default()
    },
)?;
```

Return `result.detections` as `public_shots`, `result.observable_flips` as `answers`, and `None` for `masks`.

- [ ] **Step 5: Implement blinded measurement mode artifacts**

Generate `source_labels: Vec<bool>` using the mask RNG, require the deterministic test fixture to contain both labels by choosing the seeds from Step 1, shuffle labels with Fisher-Yates using the permutation RNG, sample zero and one groups in counts derived from the shuffled labels, merge measurements into final shot order, compute public observables using `measurements_to_detections(&validated.public_instrs, &measurements)`, and set:

```rust
answers.set(0, shot, public_observable ^ mask_bit);
```

Use this helper for copying one shot between `BitTable`s:

```rust
fn copy_shot(src: &BitTable, src_shot: usize, dst: &mut BitTable, dst_shot: usize) {
    for row in 0..src.num_major() {
        if src.get(row, src_shot) {
            dst.set(row, dst_shot, true);
        }
    }
}
```

- [ ] **Step 6: Run Task 3 tests and verify GREEN**

Run:

```bash
cargo test --locked -p rstim decoder_dataset::tests::detector_artifacts_publish_detections_and_private_answers -- --exact
cargo test --locked -p rstim decoder_dataset::tests::blinded_measurement_answers_are_public_observable_xor_mask -- --exact
cargo test --locked -p rstim decoder_dataset::tests::fixed_seed_reproduces_artifacts_byte_for_byte -- --exact
```

Expected: PASS.

- [ ] **Step 7: Commit Task 3**

Run:

```bash
git add rstim/src/decoder_dataset.rs
git commit -m "feat: generate decoder dataset artifacts"
```

---

### Task 4: Staged Directory Publication

**Files:**
- Modify: `rstim/src/decoder_dataset.rs`
- Test: `rstim/src/decoder_dataset.rs`

**Interfaces:**
- Consumes: Task 3 artifacts and Task 1 manifest structs.
- Produces:
  - `export_decoder_dataset` writes public/private bundle directories.
  - Private directory is renamed before public.
  - `#[doc(hidden)] pub fn export_decoder_dataset_with_publisher(...)` accepts a test publisher for unit tests.
  - Debug-only env var `RSTIM_TEST_DECODER_DATASET_FAIL_RENAME_AT` injects rename failures for CLI tests.

- [ ] **Step 1: Write RED tests for bundle contents, leakage, and failure injection**

Add tests:

```rust
#[test]
fn export_writes_exact_public_and_private_files() {
    let root = tempfile::tempdir().unwrap();
    let public_out = root.path().join("public");
    let private_out = root.path().join("private");
    let circuit = "R 0\n# RSTIM_LOGICAL_FLIP_POINT\nX_ERROR(0.5) 0\nM 0\nOBSERVABLE_INCLUDE(0) rec[-1]\n";
    let mut config = test_config(circuit, DecoderDatasetMode::MeasurementsBlinded, vec![0]);
    config.shots = 8;
    config.seed = Some(7);
    config.public_out = public_out.clone();
    config.private_out = private_out.clone();

    let summary = export_decoder_dataset(config).unwrap();
    assert_eq!(summary.public_out, public_out);
    assert_eq!(sorted_entries(&public_out), vec!["circuit.stim", "manifest.json", "shots.b8"]);
    assert_eq!(sorted_entries(&private_out), vec!["answers.b8", "manifest.json", "masks.b8"]);

    let public_manifest = std::fs::read_to_string(public_out.join("manifest.json")).unwrap();
    assert_no_public_secret_words(&public_manifest);
    assert!(public_manifest.contains("\"mode\": \"measurements_blinded\""));
    assert!(std::fs::read_to_string(public_out.join("circuit.stim")).unwrap().contains(LOGICAL_FLIP_MARKER));
}

#[test]
fn public_directory_is_not_visible_when_public_rename_fails() {
    let root = tempfile::tempdir().unwrap();
    let public_out = root.path().join("public");
    let private_out = root.path().join("private");
    let circuit = "R 0\nM 0\nOBSERVABLE_INCLUDE(0) rec[-1]\n";
    let mut config = test_config(circuit, DecoderDatasetMode::Detectors, vec![]);
    config.public_out = public_out.clone();
    config.private_out = private_out.clone();
    config.seed = Some(3);

    let mut publisher = FailingDirectoryPublisher::new(2);
    let err = export_decoder_dataset_with_publisher(config, &mut publisher).unwrap_err();

    assert!(err.contains("private bundle retained"));
    assert!(private_out.exists());
    assert!(!public_out.exists());
    assert_no_decoder_dataset_temps(root.path());
}
```

Add local test helpers:

```rust
fn sorted_entries(path: &std::path::Path) -> Vec<String> {
    let mut entries: Vec<String> = std::fs::read_dir(path)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    entries.sort();
    entries
}

fn assert_no_public_secret_words(text: &str) {
    for forbidden in ["seed", "mask", "answer", "private", "producer", "permutation"] {
        assert!(!text.to_ascii_lowercase().contains(forbidden), "public manifest leaked {forbidden}: {text}");
    }
}

fn assert_no_decoder_dataset_temps(path: &std::path::Path) {
    for entry in std::fs::read_dir(path).unwrap() {
        let name = entry.unwrap().file_name().to_string_lossy().into_owned();
        assert!(!name.contains(".rstim-decoder-dataset-"), "temporary directory leaked: {name}");
    }
}

struct FailingDirectoryPublisher {
    fail_at: usize,
    calls: usize,
}

impl FailingDirectoryPublisher {
    fn new(fail_at: usize) -> Self {
        Self { fail_at, calls: 0 }
    }
}

impl DirectoryPublisher for FailingDirectoryPublisher {
    fn rename(&mut self, from: &std::path::Path, to: &std::path::Path) -> std::io::Result<()> {
        self.calls += 1;
        if self.calls == self.fail_at {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "injected decoder dataset rename failure",
            ));
        }
        std::fs::rename(from, to)
    }
}
```

- [ ] **Step 2: Run focused tests and verify RED**

Run:

```bash
cargo test --locked -p rstim decoder_dataset::tests::export_writes_exact_public_and_private_files -- --exact
cargo test --locked -p rstim decoder_dataset::tests::public_directory_is_not_visible_when_public_rename_fails -- --exact
```

Expected: FAIL because the exporter stub does not publish directories.

- [ ] **Step 3: Implement output path preflight**

Add a path descriptor:

```rust
struct NewDirectoryPath {
    final_path: std::path::PathBuf,
    parent: std::path::PathBuf,
    name: std::ffi::OsString,
}
```

Implement `resolve_new_output_directory(path: &Path) -> Result<NewDirectoryPath, String>`:

```rust
let name = path.file_name().ok_or_else(|| "output directory must have a final path component".to_string())?;
let parent = path.parent().unwrap_or_else(|| std::path::Path::new("."));
let parent = std::fs::canonicalize(parent)
    .map_err(|error| format!("failed to resolve output parent {}: {error}", parent.display()))?;
if !parent.is_dir() {
    return Err(format!("output parent is not a directory: {}", parent.display()));
}
let final_path = parent.join(name);
if final_path.try_exists().map_err(|error| format!("failed to inspect {}: {error}", final_path.display()))? {
    return Err(format!("output directory already exists: {}", final_path.display()));
}
```

After resolving public and private paths, reject equality and nesting:

```rust
if public.final_path == private.final_path {
    return Err("--public_out and --private_out resolve to the same directory".to_string());
}
if public.final_path.starts_with(&private.final_path) || private.final_path.starts_with(&public.final_path) {
    return Err("--public_out and --private_out must not be nested".to_string());
}
```

- [ ] **Step 4: Implement staged bundle writer**

Add:

```rust
pub trait DirectoryPublisher {
    fn rename(&mut self, from: &std::path::Path, to: &std::path::Path) -> std::io::Result<()>;
}

struct StagedBundle {
    final_path: std::path::PathBuf,
    temp_path: std::path::PathBuf,
    published: bool,
}
```

Create sibling temporary directories named `.{name}.rstim-decoder-dataset-{pid}-{retry}.tmp`. On Unix, create private staging directories with mode `0o700`:

```rust
#[cfg(unix)]
{
    use std::os::unix::fs::DirBuilderExt;
    std::fs::DirBuilder::new().mode(0o700).create(&temp_path)?;
}
#[cfg(not(unix))]
{
    std::fs::create_dir(&temp_path)?;
}
```

Implement `Drop` for `StagedBundle` so unpublished `temp_path` directories are removed with `std::fs::remove_dir_all`.

- [ ] **Step 5: Write artifacts, hash files, and publish private then public**

Inside `export_decoder_dataset`, construct the filesystem publisher and call a hidden helper:

```rust
pub fn export_decoder_dataset(config: ExportDecoderDatasetConfig) -> Result<DecoderDatasetSummary, String> {
    let mut publisher = FsDirectoryPublisher::from_env();
    export_decoder_dataset_with_publisher(config, &mut publisher)
}
```

Inside `export_decoder_dataset_with_publisher`:

```rust
#[doc(hidden)]
pub fn export_decoder_dataset_with_publisher(
    config: ExportDecoderDatasetConfig,
    publisher: &mut impl DirectoryPublisher,
) -> Result<DecoderDatasetSummary, String> {
    let validated_paths = validate_output_directories(&config.public_out, &config.private_out)?;
    let artifacts = generate_decoder_dataset_artifacts(&config)?;
    let public_shots_bytes = bit_table_to_b8_bytes(&artifacts.public_shots)?;
    let answers_bytes = bit_table_to_b8_bytes(&artifacts.answers)?;
    let masks_bytes = artifacts.masks.as_ref().map(bit_table_to_b8_bytes).transpose()?;
    let circuit_sha256 = sha256_hex(artifacts.public_circuit_text.as_bytes());
    let shots_sha256 = sha256_hex(&public_shots_bytes);
    let dataset_id = sha256_hex(&dataset_id_material(PUBLIC_SCHEMA_VERSION, config.mode, &circuit_sha256, config.shots, artifacts.public_shots.num_major(), &shots_sha256));
```

Continue in the same function after manifest construction:

```rust
let mut private_stage = StagedBundle::create(&validated_paths.private, true)?;
let mut public_stage = StagedBundle::create(&validated_paths.public, false)?;
write_private_bundle(
    &private_stage.temp_path,
    &private_manifest,
    &answers_bytes,
    masks_bytes.as_deref(),
)?;
write_public_bundle(
    &public_stage.temp_path,
    &public_manifest,
    artifacts.public_circuit_text.as_bytes(),
    &public_shots_bytes,
)?;
private_stage.publish_with(publisher)?;
match public_stage.publish_with(publisher) {
    Ok(_) => Ok(DecoderDatasetSummary {
        dataset_id,
        mode: config.mode,
        shots: config.shots,
        row_bits: artifacts.public_shots.num_major(),
        public_out: public_stage.final_path.clone(),
        private_out: private_stage.final_path.clone(),
    }),
    Err(error) => Err(format!(
        "{error}; private bundle retained at {}",
        private_stage.final_path.display()
    )),
}
}
```

Write all files into staging directories with `BufWriter<File>`, flush them, then rename:

```rust
private_stage.publish_with(publisher)?;
match public_stage.publish_with(publisher) {
    Ok(_) => {}
    Err(error) => {
        return Err(format!(
            "{error}; private bundle retained at {}",
            private_stage.final_path.display()
        ));
    }
}
```

- [ ] **Step 6: Run Task 4 tests and verify GREEN**

Run:

```bash
cargo test --locked -p rstim decoder_dataset::tests::export_writes_exact_public_and_private_files -- --exact
cargo test --locked -p rstim decoder_dataset::tests::public_directory_is_not_visible_when_public_rename_fails -- --exact
```

Expected: PASS.

- [ ] **Step 7: Commit Task 4**

Run:

```bash
git add rstim/src/decoder_dataset.rs
git commit -m "feat: publish decoder dataset bundles atomically"
```

---

### Task 5: CLI Command And Black-Box Contract Tests

**Files:**
- Modify: `rstim/src/cli.rs`
- Create: `rstim/tests/cli_decoder_dataset.rs`

**Interfaces:**
- Consumes: `decoder_dataset::{parse_logical_x_qubits, DecoderDatasetMode, ExportDecoderDatasetConfig, export_decoder_dataset}`.
- Produces: `rstim export_decoder_dataset` with real filesystem inputs and outputs.

- [ ] **Step 1: Write RED CLI integration tests**

Create `rstim/tests/cli_decoder_dataset.rs`:

```rust
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn rstim_cmd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_rstim"))
}

fn run_cli(args: &[String]) -> Output {
    rstim_cmd().args(args).output().expect("run rstim")
}

#[test]
fn export_decoder_dataset_cli_contract() {
    detectors_mode_writes_public_circuit_and_detector_rows();
    blinded_measurements_masks_recomputed_public_observable();
    deterministic_seed_reproduces_bundle_bytes();
    rejection_cases_fail_before_outputs_exist();
    println!("PASS decoder dataset cli detectors=1 blinded=1 deterministic=1 rejections=1");
}

fn detectors_mode_writes_public_circuit_and_detector_rows() {
    let root = tempfile::tempdir().unwrap();
    let circuit = root.path().join("circuit.stim");
    fs::write(&circuit, "R 0\nX_ERROR(1) 0\nM 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n").unwrap();
    let public_out = root.path().join("public");
    let private_out = root.path().join("private");
    let output = run_cli(&export_args(&circuit, "detectors", &public_out, &private_out, &[]));
    assert_success(&output, "detectors export");

    assert_eq!(sorted_entries(&public_out), vec!["circuit.stim", "manifest.json", "shots.b8"]);
    assert_eq!(sorted_entries(&private_out), vec!["answers.b8", "manifest.json"]);
    assert_eq!(fs::read(public_out.join("shots.b8")).unwrap(), vec![1]);
    assert_eq!(fs::read(private_out.join("answers.b8")).unwrap(), vec![1]);
    let manifest: Value = serde_json::from_slice(&fs::read(public_out.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest["mode"], "detectors");
    assert_eq!(manifest["row"]["kind"], "detectors");
}

fn blinded_measurements_masks_recomputed_public_observable() {
    let root = tempfile::tempdir().unwrap();
    let circuit = root.path().join("circuit.stim");
    fs::write(&circuit, "R 0\n# RSTIM_LOGICAL_FLIP_POINT\nX_ERROR(0.5) 0\nM 0\nOBSERVABLE_INCLUDE(0) rec[-1]\n").unwrap();
    let public_out = root.path().join("public");
    let private_out = root.path().join("private");
    let output = run_cli(&export_args(&circuit, "measurements_blinded", &public_out, &private_out, &["--logical_x_qubits", "0", "--seed", "9"]));
    assert_success(&output, "blinded export");

    let public_manifest = fs::read_to_string(public_out.join("manifest.json")).unwrap();
    assert_no_public_secret_words(&public_manifest);
    assert_eq!(fs::read(private_out.join("answers.b8")).unwrap().len(), 1);
    assert_eq!(fs::read(private_out.join("masks.b8")).unwrap().len(), 1);
}
```

Also include helper functions `export_args`, `assert_success`, `assert_failure`, `sorted_entries`, `assert_no_public_secret_words`, and rejection cases for `--shots 0`, `--mode nope`, detector mode with logical qubits, blinded mode without logical qubits, pre-existing output directory, and nested public/private paths.

- [ ] **Step 2: Run CLI tests and verify RED**

Run:

```bash
cargo test --locked -p rstim --test cli_decoder_dataset -- --nocapture
```

Expected: FAIL because clap does not know `export_decoder_dataset`.

- [ ] **Step 3: Add clap subcommand**

Add to `Commands`:

```rust
/// Export a public decoder dataset and private answer bundle
#[command(name = "export_decoder_dataset")]
ExportDecoderDataset {
    #[arg(long = "circuit")]
    circuit: String,
    #[arg(long = "shots")]
    shots: u64,
    #[arg(long = "mode")]
    mode: String,
    #[arg(long = "logical_x_qubits")]
    logical_x_qubits: Option<String>,
    #[arg(long = "public_out")]
    public_out: String,
    #[arg(long = "private_out")]
    private_out: String,
    #[arg(long = "seed")]
    seed: Option<u64>,
},
```

Add a match arm:

```rust
Some(Commands::ExportDecoderDataset {
    circuit,
    shots,
    mode,
    logical_x_qubits,
    public_out,
    private_out,
    seed,
}) => run_export_decoder_dataset(
    &circuit,
    shots,
    &mode,
    logical_x_qubits.as_deref(),
    &public_out,
    &private_out,
    seed,
),
```

- [ ] **Step 4: Add CLI wrapper**

Add:

```rust
pub fn run_export_decoder_dataset(
    circuit: &str,
    shots: u64,
    mode: &str,
    logical_x_qubits: Option<&str>,
    public_out: &str,
    private_out: &str,
    seed: Option<u64>,
) -> Result<(), String> {
    let circuit_text = std::fs::read_to_string(circuit)
        .map_err(|error| format!("failed to read circuit {circuit}: {error}"))?;
    let mode = crate::decoder_dataset::DecoderDatasetMode::parse(mode)?;
    let logical_x_qubits = match logical_x_qubits {
        Some(value) => crate::decoder_dataset::parse_logical_x_qubits(value)?,
        None => Vec::new(),
    };
    let shots = usize::try_from(shots)
        .map_err(|_| "--shots is too large for this platform".to_string())?;
    crate::decoder_dataset::export_decoder_dataset(
        crate::decoder_dataset::ExportDecoderDatasetConfig {
            circuit_text,
            shots,
            mode,
            logical_x_qubits,
            public_out: std::path::PathBuf::from(public_out),
            private_out: std::path::PathBuf::from(private_out),
            seed,
        },
    )
    .map(drop)
}
```

- [ ] **Step 5: Run CLI tests and verify GREEN**

Run:

```bash
cargo test --locked -p rstim --test cli_decoder_dataset -- --nocapture
```

Expected: PASS with exactly one line beginning `PASS decoder dataset cli`.

- [ ] **Step 6: Commit Task 5**

Run:

```bash
git add rstim/src/cli.rs rstim/tests/cli_decoder_dataset.rs
git commit -m "feat: add decoder dataset export CLI"
```

---

### Task 6: Repetition And Surface Memory Coverage

**Files:**
- Modify: `rstim/tests/cli_decoder_dataset.rs`

**Interfaces:**
- Consumes: real `rstim gen` output through `rstim::cli::generate_common_circuit_text_with_params` or literal fixture strings generated from the same helper.
- Produces: repetition-code and surface-code E2E coverage in both modes.

- [ ] **Step 1: Add fixture helpers**

Add helper functions:

```rust
fn generated_repetition_memory_with_marker() -> (String, &'static str) {
    let mut circuit = rstim::cli::generate_common_circuit_text(
        "repetition_code",
        "memory",
        3,
        3,
        0.01,
    )
    .unwrap();
    insert_marker_before_first_tick(&mut circuit);
    (circuit, "0,1,2")
}

fn generated_surface_z_memory_with_marker() -> (String, &'static str) {
    let mut circuit = rstim::cli::generate_common_circuit_text(
        "surface_code",
        "rotated_memory_z",
        3,
        3,
        0.01,
    )
    .unwrap();
    insert_marker_before_first_tick(&mut circuit);
    (circuit, "1,2,3")
}

fn insert_marker_before_first_tick(circuit: &mut String) {
    let needle = "TICK\n";
    let index = circuit.find(needle).expect("generated memory circuit has first TICK");
    circuit.insert_str(index, "# RSTIM_LOGICAL_FLIP_POINT\n");
}
```

- [ ] **Step 2: Add E2E tests for both modes**

Add:

```rust
#[test]
fn repetition_and_surface_memory_export_in_both_modes() {
    for (name, circuit_text, logical_x_qubits) in [
        {
            let (text, support) = generated_repetition_memory_with_marker();
            ("repetition", text, support)
        },
        {
            let (text, support) = generated_surface_z_memory_with_marker();
            ("surface_z", text, support)
        },
    ] {
        verify_memory_case(name, &circuit_text, logical_x_qubits, "detectors");
        verify_memory_case(name, &circuit_text, logical_x_qubits, "measurements_blinded");
    }
}

fn verify_memory_case(name: &str, circuit_text: &str, logical_x_qubits: &str, mode: &str) {
    let root = tempfile::tempdir().unwrap();
    let circuit = root.path().join(format!("{name}.stim"));
    fs::write(&circuit, circuit_text).unwrap();
    let public_out = root.path().join(format!("{name}-{mode}-public"));
    let private_out = root.path().join(format!("{name}-{mode}-private"));
    let mut extra = vec!["--seed", "20260728"];
    if mode == "measurements_blinded" {
        extra.extend(["--logical_x_qubits", logical_x_qubits]);
    }
    let output = run_cli(&export_args(&circuit, mode, &public_out, &private_out, &extra));
    assert_success(&output, &format!("{name} {mode}"));
    assert_eq!(sorted_entries(&public_out), vec!["circuit.stim", "manifest.json", "shots.b8"]);
    assert!(private_out.join("answers.b8").exists());
}
```

- [ ] **Step 3: Run focused E2E test and verify GREEN**

Run:

```bash
cargo test --locked -p rstim --test cli_decoder_dataset repetition_and_surface_memory_export_in_both_modes -- --exact --nocapture
```

Expected: PASS for repetition `--logical_x_qubits 0,1,2` and surface rotated Z `--logical_x_qubits 1,2,3`.

- [ ] **Step 4: Commit Task 6**

Run:

```bash
git add rstim/tests/cli_decoder_dataset.rs
git commit -m "test: cover decoder dataset memory circuits"
```

---

### Task 7: Documentation

**Files:**
- Create: `rstim/doc/decoder-dataset.md`
- Modify: `rstim/doc/rsmp-cli.md`

**Interfaces:**
- Consumes: implemented CLI and manifest field names.
- Produces: user-facing documentation that distinguishes private RSMP archives from public decoder dataset bundles.

- [ ] **Step 1: Create decoder dataset docs**

Create `rstim/doc/decoder-dataset.md` with this structure and exact command names:

````markdown
# Decoder Dataset Export

`rstim export_decoder_dataset` creates two directory bundles: a public bundle for decoder contestants and a private bundle for scoring.

## Detector Mode

```console
rstim export_decoder_dataset \
  --circuit memory.stim \
  --shots 100000 \
  --mode detectors \
  --public_out public-data \
  --private_out private-truth
```

The public bundle contains detector-event rows in `shots.b8` and the logical-zero `circuit.stim`. The private bundle contains `answers.b8`.

## Blinded Measurement Mode

```stim
R 0 1 2
# RSTIM_LOGICAL_FLIP_POINT
```

```console
rstim export_decoder_dataset \
  --circuit memory.stim \
  --shots 100000 \
  --mode measurements_blinded \
  --logical_x_qubits 0,2,4 \
  --public_out public-data \
  --private_out private-truth
```

For each shot, the exporter privately chooses a bit `b`, samples either the logical-zero circuit or the circuit with an ideal `X` on `--logical_x_qubits`, publishes the measurement row, and stores `answer = O_public(m) XOR b` privately.

## Files

Public files are exactly `manifest.json`, `circuit.stim`, and `shots.b8`. Private files are exactly `manifest.json`, `answers.b8`, and, in blinded measurement mode, `masks.b8`.
````

- [ ] **Step 2: Update RSMP CLI docs**

Add a paragraph near the RSMP command overview:

```markdown
RSMP v1 is a lossless circuit-bound archive and is intended for private transport or reproducible evidence. For public decoder competitions where answers must not be published, use `rstim export_decoder_dataset`; it emits an intentionally lossy public bundle plus a private answer bundle.
```

- [ ] **Step 3: Run doc-adjacent checks**

Run:

```bash
cargo test --locked -p rstim --test cli_decoder_dataset -- --nocapture
```

Expected: PASS.

- [ ] **Step 4: Commit Task 7**

Run:

```bash
git add rstim/doc/decoder-dataset.md rstim/doc/rsmp-cli.md
git commit -m "docs: document decoder dataset export"
```

---

### Task 8: Final Verification

**Files:**
- Modify: files from prior tasks if verification exposes defects.

**Interfaces:**
- Consumes: complete implementation and docs.
- Produces: final verified branch state.

- [ ] **Step 1: Run focused CLI contract**

Run:

```bash
cargo test --locked -p rstim --test cli_decoder_dataset -- --nocapture
```

Expected: PASS with exactly one line beginning `PASS decoder dataset cli`.

- [ ] **Step 2: Run core module tests**

Run:

```bash
cargo test --locked -p rstim decoder_dataset
```

Expected: PASS.

- [ ] **Step 3: Run existing RSMP tests to prove no semantic drift**

Run:

```bash
cargo test --locked -p rstim --test cli_rsmp_b8 -- --nocapture
cargo test --locked -p rstim --test cli_rsmp_publication -- --nocapture
cargo test --locked -p rstim --test rsmp_v1_compatibility
```

Expected: PASS. Existing RSMP public/private semantics remain unchanged.

- [ ] **Step 4: Run full crate verification**

Run:

```bash
cargo test --locked -p rstim
```

Expected: PASS.

- [ ] **Step 5: Inspect public bundle leakage manually**

Run one blinded export into a temporary directory and inspect public files:

```bash
RSTIM_DATASET_TMP=$(mktemp -d)
cargo run --locked --quiet -p rstim --bin rstim -- gen --code repetition_code --task memory --distance 3 --rounds 3 --after_clifford_depolarization 0.01 > "$RSTIM_DATASET_TMP/memory.stim"
python3 - "$RSTIM_DATASET_TMP/memory.stim" <<'PY'
from pathlib import Path
path = Path(__import__("sys").argv[1])
text = path.read_text()
path.write_text(text.replace("TICK\n", "# RSTIM_LOGICAL_FLIP_POINT\nTICK\n", 1))
PY
cargo run --locked --quiet -p rstim --bin rstim -- export_decoder_dataset --circuit "$RSTIM_DATASET_TMP/memory.stim" --shots 16 --mode measurements_blinded --logical_x_qubits 0,1,2 --public_out "$RSTIM_DATASET_TMP/public" --private_out "$RSTIM_DATASET_TMP/private" --seed 5
find "$RSTIM_DATASET_TMP/public" -maxdepth 1 -type f -print | sort
grep -R -i -E 'seed|mask|answer|private|producer|permutation' "$RSTIM_DATASET_TMP/public/manifest.json" && exit 1 || true
```

Expected: the public files are only `circuit.stim`, `manifest.json`, and `shots.b8`; grep finds no forbidden fields.

- [ ] **Step 6: Commit verification fixes**

If Step 1 through Step 5 required any fix commits, create a final focused commit:

```bash
git add rstim/src/decoder_dataset.rs rstim/src/cli.rs rstim/tests/cli_decoder_dataset.rs rstim/doc/decoder-dataset.md rstim/doc/rsmp-cli.md
git commit -m "fix: complete decoder dataset export verification"
```

Expected: skip this commit when the prior task commits already contain the verified final state.
