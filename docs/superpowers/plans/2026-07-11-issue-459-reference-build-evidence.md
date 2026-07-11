# Issue 459 Reference Build Evidence Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Publish a checked packed reference-build evidence bundle with symmetric long-lived Stim and rstim workers.

**Architecture:** Add JSONL `reference-build-v1` workers that parse once and time only reference construction plus b8 byte materialization. Add a runner that drives both workers for two warmups and seven measured builds, writes raw/summary/report/environment/hash artifacts, and add an independent checker that derives every claim from raw bytes before validating provenance and hashes.

**Tech Stack:** Python 3 standard library plus installed `stim==1.15.0` and NumPy through Stim, Rust 2024 with existing `rstim` crate APIs, existing Cargo workspace, existing benchmark fixture and manifest.

## Global Constraints

- Add `benchmarks/rstim_vs_stim_simulator/run_reference_build_benchmark.py`.
- Add `benchmarks/rstim_vs_stim_simulator/workers/stim_reference_build.py`.
- Add `rstim/src/bin/rstim_reference_build_worker.rs`.
- Add `tools/check_rstim_vs_stim_reference_build_evidence.py`.
- Publish exactly `raw.jsonl`, `summary.json`, `report.md`, `environment.json`, and `artifact-sha256.json` under `benchmarks/rstim_vs_stim_simulator/results/reference-build-release/`.
- Canonical workers are `python3 -m benchmarks.rstim_vs_stim_simulator.workers.stim_reference_build --protocol reference-build-v1` and `target/release/rstim_reference_build_worker --protocol reference-build-v1`.
- Variants are exactly `stim-reference-b8` and `rstim-packed-reference-b8`.
- Protocol is exactly `reference-build-v1`.
- One `load` request parses the fixture exactly once; nine `build_reference` requests follow.
- Timer scope is exactly `reference_build_only`: starts immediately before reference construction and stops after the final packed byte is materialized; excludes startup, parsing, IPC, base64, hashing, and JSON serialization.
- Every build result contains 12,121 bits, 1,516 bytes, and SHA-256 `d95f3eacd05c1ca0d3a90e4a48e1d68b7ef5f2d817da11121ba4b77454b24d3d`.
- rstim build responses must report `backend="packed_inverse"`.
- Raw contains exactly 18 rows with variant, phase, round, elapsed time, base64 packed bytes, bytes, digest, backend, timer scope, `parse_count`, and `reference_build_count`.
- Summary is recomputed from seven measured rows per variant and records count, min/median/max, digest, backend, and counters.
- Report values must come from summary.
- `artifact-sha256.json` maps the other four files to lowercase SHA-256 digests.
- Environment records release profile, exact runner/worker argv, deterministic no-seed policy, git commit/dirty state, fixture and manifest paths/hashes, Stim `1.15.0`, executable paths/hashes, rustc/cargo/Python versions, OS/CPU, rounds, protocol, and timer scope.
- Manifest SHA-256 is `9fc35393f362f709e90bfd64ab08eda5140844974a7e685fd1e5614f67e0c921`.
- Checker decodes bytes, recomputes digests/statistics, cross-checks report/environment, validates semantic fields, and only then validates mandatory artifact hashes.
- Checker tests independently reject changed decoded byte, mismatched digest, legacy rstim backend, timer scope including parsing, `parse_count != 1`, final `reference_build_count != 9`, summary not recomputable from raw rows, and missing hash manifest.
- Semantic failures must be reported before hash mismatches.
- Do not time parsing, frame construction, IPC, shot sampling, base64, hashing, or JSON serialization.
- Do not update site metadata or require a speed ratio.

---

### Task 1: Failing Tests For Workers, Runner, And Checker

**Files:**
- Create: `benchmarks/rstim_vs_stim_simulator/tests/test_run_reference_build_benchmark.py`
- Create: `tools/test_check_rstim_vs_stim_reference_build_evidence.py`
- Create: `rstim/tests/rstim_reference_build_worker.rs`

**Interfaces:**
- Consumes: planned JSONL protocol, fixture `benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim`, manifest `benchmarks/rstim_vs_stim_simulator/cases.full.toml`.
- Produces: failing tests that define the runner output schema, checker semantics, and Rust worker protocol.

- [ ] **Step 1: Write Python runner tests**

Create `benchmarks/rstim_vs_stim_simulator/tests/test_run_reference_build_benchmark.py` with fake JSONL workers. Include:

```python
class RunReferenceBuildBenchmarkTest(unittest.TestCase):
    def test_fake_workers_emit_required_artifacts_and_hash_manifest(self) -> None: ...
    def test_default_worker_argvs_match_reference_build_protocol(self) -> None: ...
    def test_runner_rejects_wrong_manifest_hash_before_launching_workers(self) -> None: ...
```

The fake worker must accept `--protocol reference-build-v1`, parse one load line, reply with `parse_count=1`, then reply to nine build requests with base64 for `b"\x00" * 1516`, digest `d95f3eacd05c1ca0d3a90e4a48e1d68b7ef5f2d817da11121ba4b77454b24d3d`, `measurement_bits=12121`, `packed_bytes=1516`, `timer_scope="reference_build_only"`, and incrementing `reference_build_count`.

- [ ] **Step 2: Write checker tests**

Create `tools/test_check_rstim_vs_stim_reference_build_evidence.py` with helpers:

```python
def write_valid_bundle(path: Path, *, rstim_worker: Path) -> None: ...
def rewrite_artifact_hashes(bundle: Path) -> None: ...
def load_raw(path: Path) -> list[dict[str, Any]]: ...
```

Add tests:

```python
def test_accepts_valid_bundle(self) -> None: ...
def test_rejects_changed_decoded_byte_before_hash_mismatch(self) -> None: ...
def test_rejects_mismatched_digest(self) -> None: ...
def test_rejects_legacy_rstim_backend(self) -> None: ...
def test_rejects_timer_scope_including_parsing(self) -> None: ...
def test_rejects_parse_count_not_one(self) -> None: ...
def test_rejects_missing_final_reference_build_count_nine(self) -> None: ...
def test_rejects_rehashed_summary_not_derived_from_raw(self) -> None: ...
def test_rejects_missing_hash_manifest(self) -> None: ...
```

The valid synthetic bundle uses repository-relative fixture and manifest paths, the canonical manifest digest, and artifact hashes for the other four files.

- [ ] **Step 3: Write Rust worker integration test**

Create `rstim/tests/rstim_reference_build_worker.rs`:

```rust
use serde_json::json;
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

#[test]
fn rstim_reference_build_worker_parses_once_and_builds_references() {
    let worker = env!("CARGO_BIN_EXE_rstim_reference_build_worker");
    let mut child = Command::new(worker)
        .arg("--protocol")
        .arg("reference-build-v1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("worker starts");
    let mut stdin = child.stdin.take().expect("stdin");
    let stdout = child.stdout.take().expect("stdout");
    let mut reader = BufReader::new(stdout);
    let fixture = tempfile::NamedTempFile::new().expect("fixture");
    std::fs::write(fixture.path(), "X 0\nM 0\n").expect("write fixture");
    writeln!(
        stdin,
        "{}",
        json!({"protocol":"reference-build-v1","type":"load","fixture_path":fixture.path()})
    )
    .expect("send load");
    let mut line = String::new();
    reader.read_line(&mut line).expect("read load");
    let loaded: serde_json::Value = serde_json::from_str(&line).expect("load json");
    assert_eq!(loaded["type"], "loaded");
    assert_eq!(loaded["parse_count"], 1);
    for request_id in 0..2 {
        writeln!(
            stdin,
            "{}",
            json!({"protocol":"reference-build-v1","type":"build_reference","request_id":request_id})
        )
        .expect("send build");
        line.clear();
        reader.read_line(&mut line).expect("read build");
        let built: serde_json::Value = serde_json::from_str(&line).expect("build json");
        assert_eq!(built["type"], "reference_built");
        assert_eq!(built["backend"], "packed_inverse");
        assert_eq!(built["parse_count"], 1);
        assert_eq!(built["reference_build_count"], request_id + 1);
        assert_eq!(built["measurement_bits"], 1);
        assert_eq!(built["packed_bytes"], 1);
        assert_eq!(built["packed_base64"], "AQ==");
    }
    drop(stdin);
    assert!(child.wait().expect("wait").success());
}
```

- [ ] **Step 4: Run tests to verify RED**

Run:

```sh
python3 -m unittest \
  benchmarks.rstim_vs_stim_simulator.tests.test_run_reference_build_benchmark \
  tools.test_check_rstim_vs_stim_reference_build_evidence -q
cargo test -p rstim --test rstim_reference_build_worker
```

Expected: Python fails because the runner/checker modules do not exist, and Cargo fails because the new binary target does not exist.

- [ ] **Step 5: Commit tests**

```sh
git add benchmarks/rstim_vs_stim_simulator/tests/test_run_reference_build_benchmark.py \
  tools/test_check_rstim_vs_stim_reference_build_evidence.py \
  rstim/tests/rstim_reference_build_worker.rs
git commit -m "test: specify reference-build evidence"
```

### Task 2: JSONL Workers

**Files:**
- Create: `benchmarks/rstim_vs_stim_simulator/workers/stim_reference_build.py`
- Create: `rstim/src/bin/rstim_reference_build_worker.rs`

**Interfaces:**
- Consumes: JSONL `load` and `build_reference` requests.
- Produces: canonical worker commands from the issue with `reference-build-v1` responses.

- [ ] **Step 1: Implement Stim worker**

Implement `stim_reference_build.py` with:

```python
PROTOCOL = "reference-build-v1"
EXPECTED_STIM_VERSION = "1.15.0"
TIMER_SCOPE = "reference_build_only"
BACKEND = "stim_reference"
```

`load` reads the fixture, constructs `stim.Circuit`, increments `parse_count`, and reports measurement count. `build_reference` times only:

```python
started_ns = time.perf_counter_ns()
bits = circuit.reference_sample()
packed = numpy.packbits(bits, bitorder="little").tobytes()
elapsed_ns = time.perf_counter_ns() - started_ns
```

Then hash/base64 after the timer stops.

- [ ] **Step 2: Implement rstim worker**

Implement `rstim_reference_build_worker.rs` with `clap`, `serde`, `serde_json`, and `sha2`. Parse one fixture on `load`, store `Vec<StimInstr>`, and for each build time only:

```rust
let started = Instant::now();
let reference = build_reference_sample_with_decision(&instructions)?;
let packed = pack_b8(&reference.bits);
let elapsed_ns = started.elapsed().as_nanos() as u64;
```

Reject any decision other than `ReferenceSampleDecision::PackedInverse`. Encode base64 with a local standard-base64 helper to avoid adding a dependency.

- [ ] **Step 3: Run worker tests to verify GREEN**

Run:

```sh
cargo test -p rstim --test rstim_reference_build_worker
python3 -m unittest \
  benchmarks.rstim_vs_stim_simulator.tests.test_run_reference_build_benchmark.RunReferenceBuildBenchmarkTest.test_default_worker_argvs_match_reference_build_protocol -q
```

Expected: worker protocol test passes and default argv test passes after Task 3 exposes the default helpers; if the default helper test is still red, leave it for Task 3.

- [ ] **Step 4: Commit workers**

```sh
git add benchmarks/rstim_vs_stim_simulator/workers/stim_reference_build.py \
  rstim/src/bin/rstim_reference_build_worker.rs
git commit -m "feat: add reference-build workers"
```

### Task 3: Runner And Artifact Writer

**Files:**
- Create: `benchmarks/rstim_vs_stim_simulator/run_reference_build_benchmark.py`
- Modify: `benchmarks/rstim_vs_stim_simulator/tests/test_run_reference_build_benchmark.py`

**Interfaces:**
- Consumes: canonical worker commands and fixture/manifest CLI.
- Produces: runner CLI from the issue and the five-file evidence bundle.

- [ ] **Step 1: Implement runner constants and helpers**

Implement:

```python
PROTOCOL = "reference-build-v1"
TIMER_SCOPE = "reference_build_only"
EXPECTED_MANIFEST_SHA256 = "9fc35393f362f709e90bfd64ab08eda5140844974a7e685fd1e5614f67e0c921"
EXPECTED_REFERENCE_SHA256 = "d95f3eacd05c1ca0d3a90e4a48e1d68b7ef5f2d817da11121ba4b77454b24d3d"
EXPECTED_MEASUREMENT_BITS = 12121
EXPECTED_PACKED_BYTES = 1516
STIM_VARIANT = "stim-reference-b8"
RSTIM_VARIANT = "rstim-packed-reference-b8"
```

Add `default_stim_worker_argv(stim_python: str) -> list[str]`,
`default_rstim_worker_argv(rstim_worker: str) -> list[str]`, `sha256_file`,
`write_artifact_hashes`, and JSONL `WorkerSession`.

- [ ] **Step 2: Implement benchmark execution**

For each variant:

1. Start the worker.
2. Send `load`.
3. Validate `parse_count == 1`.
4. Send request IDs 0 through 8.
5. Validate bytes, digest, timer scope, backend, and counters.
6. Append raw rows with phase `warmup` for rounds 0 and 1 and `measured` for rounds 2 through 8.

- [ ] **Step 3: Implement summary/report/environment**

Summary shape:

```json
{
  "measured_records": 14,
  "protocol": "reference-build-v1",
  "timer_scope": "reference_build_only",
  "variants": [
    {
      "variant": "stim-reference-b8",
      "count": 7,
      "min_elapsed_ns": 1,
      "median_elapsed_ns": 2,
      "max_elapsed_ns": 3,
      "measurement_bits": 12121,
      "packed_bytes": 1516,
      "byte_sha256": "d95f3e...",
      "backend": "stim_reference",
      "parse_count": 1,
      "final_reference_build_count": 9
    }
  ]
}
```

Report table columns: `variant`, `count`, `min_elapsed_ns`,
`median_elapsed_ns`, `max_elapsed_ns`, `backend`, `parse_count`,
`final_reference_build_count`, `byte_sha256`.

Environment records the fields listed in Global Constraints and captures git
commit/dirty state before writing artifacts.

- [ ] **Step 4: Run runner tests to verify GREEN**

Run:

```sh
python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_run_reference_build_benchmark -q
```

Expected: all runner tests pass.

- [ ] **Step 5: Commit runner**

```sh
git add benchmarks/rstim_vs_stim_simulator/run_reference_build_benchmark.py \
  benchmarks/rstim_vs_stim_simulator/tests/test_run_reference_build_benchmark.py
git commit -m "feat: add reference-build benchmark runner"
```

### Task 4: Checker

**Files:**
- Create: `tools/check_rstim_vs_stim_reference_build_evidence.py`
- Modify: `tools/test_check_rstim_vs_stim_reference_build_evidence.py`

**Interfaces:**
- Consumes: the runner artifact schema.
- Produces: CLI `tools/check_rstim_vs_stim_reference_build_evidence.py --dir <path>` with success text `PASS packed reference-build evidence`.

- [ ] **Step 1: Implement checker functions**

Implement:

```python
sha256_file(path: Path) -> str
load_json_object(path: Path, label: str) -> dict[str, Any]
load_raw_records(path: Path) -> list[dict[str, Any]]
validate_required_files(results_dir: Path) -> None
validate_raw_semantics(records: list[dict[str, Any]]) -> dict[str, Any]
derive_summary(records: list[dict[str, Any]]) -> dict[str, Any]
render_report(summary: dict[str, Any]) -> str
validate_environment(environment: dict[str, Any], derived: dict[str, Any], records: list[dict[str, Any]]) -> None
validate_artifact_hashes(results_dir: Path) -> None
validate_bundle(results_dir: Path) -> None
main(argv: list[str] | None = None) -> int
```

Import runner constants and render helpers from
`benchmarks.rstim_vs_stim_simulator.run_reference_build_benchmark` to avoid
summary/report drift.

- [ ] **Step 2: Enforce semantic-before-hash failures**

In `validate_bundle`, call in this order:

```python
validate_required_files(results_dir)
records = load_raw_records(results_dir / "raw.jsonl")
derived = validate_raw_semantics(records)
summary = derive_summary(records)
# compare summary and report
environment = load_json_object(results_dir / "environment.json", "environment.json")
validate_environment(environment, derived, records)
validate_artifact_hashes(results_dir)
```

Do not call `validate_artifact_hashes` before semantic validation succeeds.

- [ ] **Step 3: Run checker tests to verify GREEN**

Run:

```sh
python3 -m unittest tools.test_check_rstim_vs_stim_reference_build_evidence -q
```

Expected: all checker tests pass.

- [ ] **Step 4: Commit checker**

```sh
git add tools/check_rstim_vs_stim_reference_build_evidence.py \
  tools/test_check_rstim_vs_stim_reference_build_evidence.py
git commit -m "test: check reference-build evidence semantics"
```

### Task 5: Release Bundle And Final Verification

**Files:**
- Create: `benchmarks/rstim_vs_stim_simulator/results/reference-build-release/raw.jsonl`
- Create: `benchmarks/rstim_vs_stim_simulator/results/reference-build-release/summary.json`
- Create: `benchmarks/rstim_vs_stim_simulator/results/reference-build-release/report.md`
- Create: `benchmarks/rstim_vs_stim_simulator/results/reference-build-release/environment.json`
- Create: `benchmarks/rstim_vs_stim_simulator/results/reference-build-release/artifact-sha256.json`

**Interfaces:**
- Consumes: release rstim worker binary, runner, checker.
- Produces: checked release evidence bundle and PR-ready branch.

- [ ] **Step 1: Build release worker**

Run:

```sh
cargo build --release -p rstim --bin rstim_reference_build_worker
```

Expected: exit code 0.

- [ ] **Step 2: Generate temp evidence and check it**

Run:

```sh
rm -rf /tmp/rstim-reference-build
python3 -m benchmarks.rstim_vs_stim_simulator.run_reference_build_benchmark \
  --fixture benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim \
  --manifest benchmarks/rstim_vs_stim_simulator/cases.full.toml \
  --stim-python "$(command -v python3)" \
  --rstim-worker target/release/rstim_reference_build_worker \
  --warmup-rounds 2 --measure-rounds 7 \
  --out-dir /tmp/rstim-reference-build
python3 tools/check_rstim_vs_stim_reference_build_evidence.py --dir /tmp/rstim-reference-build
```

Expected checker stdout:

```text
PASS packed reference-build evidence
```

- [ ] **Step 3: Generate release bundle and check it**

Run:

```sh
rm -rf benchmarks/rstim_vs_stim_simulator/results/reference-build-release
python3 -m benchmarks.rstim_vs_stim_simulator.run_reference_build_benchmark \
  --fixture benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim \
  --manifest benchmarks/rstim_vs_stim_simulator/cases.full.toml \
  --stim-python "$(command -v python3)" \
  --rstim-worker target/release/rstim_reference_build_worker \
  --warmup-rounds 2 --measure-rounds 7 \
  --out-dir benchmarks/rstim_vs_stim_simulator/results/reference-build-release
python3 tools/check_rstim_vs_stim_reference_build_evidence.py \
  --dir benchmarks/rstim_vs_stim_simulator/results/reference-build-release
```

Expected checker stdout:

```text
PASS packed reference-build evidence
```

- [ ] **Step 4: Run required unit tests**

Run:

```sh
python3 -m unittest \
  benchmarks.rstim_vs_stim_simulator.tests.test_run_reference_build_benchmark \
  tools.test_check_rstim_vs_stim_reference_build_evidence -q
```

Expected: all tests pass.

- [ ] **Step 5: Run required Cargo verification**

Run:

```sh
cargo test
```

Expected: all Rust tests pass.

- [ ] **Step 6: Commit release bundle and any remaining docs**

Run:

```sh
git add benchmarks/rstim_vs_stim_simulator/results/reference-build-release \
  docs/superpowers/plans/2026-07-11-issue-459-reference-build-evidence.md
git commit -m "data: publish reference-build evidence"
```

- [ ] **Step 7: Push and create PR**

Run:

```sh
git push -u origin agent/issue-459-publish-packed-reference-sampling-phase-evidence-run-1
gh pr create --base master \
  --head agent/issue-459-publish-packed-reference-sampling-phase-evidence-run-1 \
  --title "Publish packed reference-build evidence" \
  --body-file /tmp/issue-459-pr-body.md
```

Expected: PR URL is printed.
