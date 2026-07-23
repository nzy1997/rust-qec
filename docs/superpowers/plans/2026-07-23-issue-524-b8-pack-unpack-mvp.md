# Issue 524 b8 Pack/Unpack MVP Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add exact, bounded, transactionally published `rstim pack_samples` and `rstim unpack_samples` commands for dense one-block `rsmp` archives and Stim-compatible `b8` streams.

**Architecture:** Keep `rstim/src/cli.rs` as the command wiring layer and call directly into `MeasurementTransform` and `sample_archive`. Add one focused strict `b8` reader for pack input and small CLI-owned publication helpers for sibling temporary files; reuse `write_shots_b8` for every unpack interchange output.

**Tech Stack:** Rust 2024, clap derive CLI, existing `BitTable`, existing `MeasurementTransform`, existing `sample_archive`, Cargo integration tests using the real `rstim` binary.

## Global Constraints

- Command names are exactly `pack_samples` and `unpack_samples`.
- Option names are exactly `--circuit`, `--shots`, `--in`, `--in_format`, `--out`, `--measurements_out`, `--measurements_out_format`, `--detectors_out`, `--detectors_out_format`, `--obs_out`, and `--obs_out_format`.
- Do not introduce `sample-pack`, `sample-unpack`, `--input`, `--output`, `--out-format`, or `--dets-out` aliases.
- For this issue, every format option defaults to `b8` and rejects any other value.
- Packing must use an exact-shot strict `b8` reader, not `read_shots_b8(data, bits)`.
- Reject shots above the one-block archive limit before consuming an unbounded stdin payload.
- At most one input may consume stdin and at most one output may target stdout.
- At least one unpack output is required.
- Duplicate final file output paths are rejected before any destination is opened.
- File outputs use collision-safe sibling temporary files and are published only after writer or reader `finish()` succeeds.
- Corrupt or incomplete archives must not create, truncate, or replace any unpack destination.
- The required focused command is `cargo test --locked -p rstim --test cli_rsmp_b8 -- --nocapture`.
- The required PASS line is exactly `PASS rsmp b8 cli valid_cases=7 negative_cases=10`.

---

### Task 1: Real CLI Contract Test

**Files:**
- Create: `rstim/tests/cli_rsmp_b8.rs`

**Interfaces:**
- Consumes: the existing `CARGO_BIN_EXE_rstim` test binary pattern, `MeasurementTransform`, `measurements_to_detections`, `write_shots_b8`, and the shared rsmp fixture catalog.
- Produces: a failing integration test that proves the exact command spelling, seven positive semantic roles, ten negative controls, sentinel preservation, and no leaked sibling temporary files.

- [ ] **Step 1: Write the failing test**

Create `rstim/tests/cli_rsmp_b8.rs` with helpers for:

```rust
fn rstim_cmd() -> std::process::Command {
    std::process::Command::new(env!("CARGO_BIN_EXE_rstim"))
}

fn run_cli(args: &[String], stdin: Option<&[u8]>) -> std::process::Output {
    use std::io::Write;
    use std::process::Stdio;
    let mut command = rstim_cmd();
    command.args(args).stdin(if stdin.is_some() { Stdio::piped() } else { Stdio::null() });
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command.spawn().expect("spawn rstim");
    if let Some(bytes) = stdin {
        child.stdin.take().expect("stdin").write_all(bytes).expect("write stdin");
    }
    child.wait_with_output().expect("wait rstim")
}
```

The positive cases must call the real binary with the documented underscore command names and options. They must cover measurements-only, detectors-only, observables-only, all-three-output unpack, one stdin/stdout pipeline, and `M = 0` with nonzero shots. For each nonzero case, pack the measurement bytes, unpack, compare measurements byte-for-byte, and compare detector and observable bytes to `measurements_to_detections` serialized through `write_shots_b8`.

The negative cases must initialize every relevant destination with distinct sentinel bytes, run the failing command, assert a nonzero status and the required underlying code when applicable, assert every sentinel remains byte-for-byte unchanged, and assert no sibling temporary files remain.

- [ ] **Step 2: Verify RED**

Run:

```bash
cargo test --locked -p rstim --test cli_rsmp_b8 -- --nocapture
```

Expected: FAIL because `pack_samples` and `unpack_samples` are not implemented.

### Task 2: CLI Pack/Unpack Implementation

**Files:**
- Modify: `rstim/src/cli.rs`

**Interfaces:**
- Consumes: `MeasurementTransform::from_circuit`, `ArchiveLimits::default`, `SampleArchiveWriter`, `SampleArchiveReader`, and `write_shots_b8`.
- Produces: the public `pack_samples` and `unpack_samples` CLI commands plus internal helpers for strict `b8`, stream validation, and staged file publication.

- [ ] **Step 1: Add command variants and dispatch**

Add clap variants with exact names:

```rust
#[command(name = "pack_samples")]
PackSamples {
    #[arg(long = "circuit")]
    circuit: String,
    #[arg(long = "shots")]
    shots: u64,
    #[arg(long = "in")]
    r#in: String,
    #[arg(long = "in_format", default_value = "b8")]
    in_format: String,
    #[arg(long = "out")]
    out: String,
},
#[command(name = "unpack_samples")]
UnpackSamples {
    #[arg(long = "circuit")]
    circuit: String,
    #[arg(long = "in")]
    r#in: String,
    #[arg(long = "measurements_out")]
    measurements_out: Option<String>,
    #[arg(long = "measurements_out_format", default_value = "b8")]
    measurements_out_format: String,
    #[arg(long = "detectors_out")]
    detectors_out: Option<String>,
    #[arg(long = "detectors_out_format", default_value = "b8")]
    detectors_out_format: String,
    #[arg(long = "obs_out")]
    obs_out: Option<String>,
    #[arg(long = "obs_out_format", default_value = "b8")]
    obs_out_format: String,
},
```

Route them from `run_command` only after preflight validation.

- [ ] **Step 2: Implement strict pack input**

Implement a private helper equivalent to:

```rust
fn read_exact_b8_measurements(data: &[u8], bits: usize, shots: u64) -> Result<BitTable, String>
```

It checks `shots <= usize::MAX`, computes `bytes_per_shot = ceil(bits / 8)` and expected byte count with checked arithmetic, requires exact input length, rejects nonzero unused high bits in each shot's final byte, and allocates `BitTable::try_new(bits, shots_usize)`.

- [ ] **Step 3: Implement stream/path preflight**

Before opening any path, validate:

```text
pack: `in_format == "b8"`, input/stdin conflict count <= 1, output/stdout count <= 1, shots <= ArchiveLimits::default().transform.max_shots_per_block, and output path present.
unpack: all requested formats are `b8`, at least one output exists, stdin input count <= 1, stdout output count <= 1, and normalized duplicate file outputs are rejected.
```

Use lexical absolute path normalization for duplicate final output comparison. Treat `-` as stdin/stdout and exclude it from duplicate file checks.

- [ ] **Step 4: Implement staged publication**

Add small helpers in `cli.rs` for sibling temporary files:

```rust
struct PendingOutput { final_path: std::path::PathBuf, temp_path: std::path::PathBuf, file: std::io::BufWriter<std::fs::File>, published: bool }
```

Create temp names beside the final path with the process id and a retry counter, open with `create_new(true)`, write and flush the temp file, publish with `std::fs::rename`, and remove only unpublished temp paths on drop or error.

- [ ] **Step 5: Implement pack and unpack**

Packing reads the circuit, builds a transform, rejects over-limit shots, reads the measurement stream, writes the archive into a temp file or stdout, calls writer `finish()`, then publishes the archive file.

Unpacking reads the circuit and archive, calls `SampleArchiveReader::open`, reads at most one block with `next_block()`, calls `finish()`, then writes any requested decoded tables through `write_shots_b8` to temp files or stdout and publishes file outputs.

### Task 3: Verification, Review, and Pull Request

**Files:**
- Modify only files touched by Tasks 1 and 2 unless a focused fix requires otherwise.

**Interfaces:**
- Consumes: complete CLI implementation and test contract.
- Produces: passing required verification, broad `cargo test` evidence, code-review evidence, pushed branch, and a PR against `master` for issue #524.

- [ ] **Step 1: Run focused verification**

Run:

```bash
cargo test --locked -p rstim --test cli_rsmp_b8 -- --nocapture
```

Expected: exit 0 and exactly one line equal to `PASS rsmp b8 cli valid_cases=7 negative_cases=10`.

- [ ] **Step 2: Run required broader verification**

Run:

```bash
cargo test
```

Expected: exit 0.

- [ ] **Step 3: Review, commit, push, and open PR**

Use the Superpowers verification-before-completion, requesting-code-review, and finishing-a-development-branch workflows. When finishing presents options, choose `Push and create a Pull Request`, push `agent/issue-524-add-the-b8-pack-unpack-mvp-run-1`, and create the PR against `master` without merging.
