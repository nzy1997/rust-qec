# Paired Reference-Build Evidence Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Refresh the `reference-build-release` evidence slot with same-run Stim, benchmark-only canonical rstim, and production direct-repeat rstim evidence.

**Architecture:** Keep production sampler routing unchanged. Add a benchmark-only strategy switch to `rstim_reference_build_worker`, then expand the Python runner/checker and committed evidence bundle to the three-variant shape required by issue #490.

**Tech Stack:** Rust 2024, clap, serde JSON, Python 3 unittest, existing benchmark artifact format.

## Global Constraints

- Existing `benchmarks/rstim_vs_stim_simulator/results/reference-build-release/summary.json` must be preserved as `baseline-summary.json` with SHA-256 `614658cf8213b486752f1fe53b7d864561abbe41c2eefd799fc8fa34883270a5`.
- Variants are exactly `stim-reference-b8`, `rstim-canonical-reference-b8`, and `rstim-direct-repeat-reference-b8`.
- All variants parse once, run two warmups and seven measured builds, and produce identical 1,516-byte packed output with byte SHA-256 `d95f3eacd05c1ca0d3a90e4a48e1d68b7ef5f2d817da11121ba4b77454b24d3d`.
- The canonical rstim strategy is benchmark-only and must not be used by production sampler routing.
- The production variant backend is exactly `direct_inverse_repeat_folded`.
- The production variant requires `canonical_materializations == 0`, `executed_repeat_iterations == 1`, and `skipped_repeat_iterations == 98`.
- The checker must assert same-run `direct_speedup >= 2.0`, computed as canonical median divided by direct median.
- No absolute timing threshold applies.
- The checker pass line begins `PASS packed reference-build evidence variants=3 direct_speedup=`.
- Semantic digest validation must run before artifact hash validation.

---

### Task 1: Worker Strategy Switch

**Files:**
- Modify: `rstim/src/bin/rstim_reference_build_worker.rs`
- Modify: `rstim/tests/rstim_reference_build_worker.rs`

**Interfaces:**
- Consumes: existing `build_reference_sample_with_decision`, existing `rstim::executor::reference_sample`, existing `ReferenceBuildPhaseCounters`.
- Produces: `--strategy direct` default backend `direct_inverse_repeat_folded`; `--strategy canonical` backend `canonical_roundtrip`; both include phase counters when `include_phase_counters` is true.

- [ ] **Step 1: Write the failing Rust tests**

Add a helper in `rstim/tests/rstim_reference_build_worker.rs`:

```rust
fn spawn_worker_with_args(args: &[&str]) -> (Child, ChildStdin, BufReader<ChildStdout>) {
    let worker = env!("CARGO_BIN_EXE_rstim_reference_build_worker");
    let mut child = Command::new(worker)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("worker starts");
    let stdin = child.stdin.take().expect("stdin");
    let stdout = child.stdout.take().expect("stdout");
    (child, stdin, BufReader::new(stdout))
}
```

Then update `spawn_worker` to call:

```rust
fn spawn_worker(protocol: &str) -> (Child, ChildStdin, BufReader<ChildStdout>) {
    spawn_worker_with_args(&["--protocol", protocol])
}
```

Add this test:

```rust
#[test]
fn rstim_reference_build_worker_defaults_to_direct_repeat_strategy() {
    let (mut child, mut stdin, mut reader) = spawn_worker(PROTOCOL);
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim");
    writeln!(
        stdin,
        "{}",
        json!({"protocol":PROTOCOL,"type":"load","fixture_path":fixture})
    )
    .expect("send load");
    let mut line = String::new();
    assert_eq!(read_response(&mut reader, &mut line)["type"], "loaded");

    writeln!(
        stdin,
        "{}",
        json!({"protocol":PROTOCOL,"type":"build_reference","request_id":0,"include_phase_counters":true})
    )
    .expect("send build");
    let built = read_response(&mut reader, &mut line);
    assert_eq!(built["backend"], "direct_inverse_repeat_folded");
    let counters = built["phase_counters"].as_object().expect("phase counters object");
    assert_eq!(counters["canonical_materializations"], json!(0));
    assert_eq!(counters["executed_repeat_iterations"], json!(1));
    assert_eq!(counters["skipped_repeat_iterations"], json!(98));

    drop(stdin);
    assert!(child.wait().expect("wait").success());
}
```

Add this test:

```rust
#[test]
fn rstim_reference_build_worker_canonical_strategy_is_benchmark_only() {
    let (mut child, mut stdin, mut reader) =
        spawn_worker_with_args(&["--protocol", PROTOCOL, "--strategy", "canonical"]);
    let fixture = tempfile::NamedTempFile::new().expect("fixture");
    std::fs::write(fixture.path(), "H 0\nM 0\n").expect("write fixture");
    writeln!(
        stdin,
        "{}",
        json!({"protocol":PROTOCOL,"type":"load","fixture_path":fixture.path()})
    )
    .expect("send load");
    let mut line = String::new();
    assert_eq!(read_response(&mut reader, &mut line)["type"], "loaded");

    writeln!(
        stdin,
        "{}",
        json!({"protocol":PROTOCOL,"type":"build_reference","request_id":0,"include_phase_counters":true})
    )
    .expect("send build");
    let built = read_response(&mut reader, &mut line);
    assert_eq!(built["backend"], "canonical_roundtrip");
    assert_eq!(built["packed_bytes"], json!(1));
    assert_eq!(built["packed_base64"], "AA==");
    let counters = built["phase_counters"].as_object().expect("phase counters object");
    assert_eq!(counters["measurement_bits"], json!(1));
    assert!(counters["canonical_materializations"].as_u64().unwrap() > 0);
    assert_eq!(counters["skipped_repeat_iterations"], json!(0));

    drop(stdin);
    assert!(child.wait().expect("wait").success());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```sh
cargo test -p rstim --test rstim_reference_build_worker
```

Expected: FAIL because `--strategy` is unknown and the default backend is still `packed_inverse`.

- [ ] **Step 3: Implement the worker strategy**

In `rstim/src/bin/rstim_reference_build_worker.rs`, replace the single backend constant with:

```rust
const DIRECT_BACKEND: &str = "direct_inverse_repeat_folded";
const CANONICAL_BACKEND: &str = "canonical_roundtrip";
```

Add:

```rust
#[derive(Clone, Copy, Debug, clap::ValueEnum)]
enum Strategy {
    Direct,
    Canonical,
}
```

Update `Args`:

```rust
#[derive(Parser)]
#[command(name = "rstim_reference_build_worker", version)]
struct Args {
    #[arg(long)]
    protocol: String,
    #[arg(long, value_enum, default_value_t = Strategy::Direct)]
    strategy: Strategy,
}
```

Add `strategy: Strategy` to `WorkerState` and initialize it from `args.strategy`.

Add this helper near `pack_b8`:

```rust
fn canonical_phase_counters(instructions: &[StimInstr], measurement_bits: usize) -> ReferenceBuildPhaseCounters {
    ReferenceBuildPhaseCounters {
        measurement_reset_batches: measurement_bits,
        canonical_materializations: measurement_bits.max(1),
        canonical_writebacks: measurement_bits,
        expanded_repeat_iterations: count_repeat_iterations(instructions),
        executed_repeat_iterations: count_repeat_iterations(instructions),
        skipped_repeat_iterations: 0,
        measurement_bits,
        ..ReferenceBuildPhaseCounters::default()
    }
}

fn count_repeat_iterations(instructions: &[StimInstr]) -> usize {
    instructions.iter().fold(0_usize, |total, instr| match instr {
        StimInstr::Op { .. } => total,
        StimInstr::Repeat { count, body } => {
            let count = usize::try_from(*count).unwrap_or(usize::MAX);
            total.saturating_add(count).saturating_add(count.saturating_mul(count_repeat_iterations(body)))
        }
    })
}
```

In `handle_build_reference`, branch on `state.strategy`:

```rust
let (backend, bits, phase_counters) = match state.strategy {
    Strategy::Direct => {
        let reference = build_reference_sample_with_decision(instructions)?;
        let bits = match reference.decision {
            ReferenceSampleDecision::PackedInverse => reference.bits,
            other => return Err(format!("unsupported reference sample decision: {other:?}")),
        };
        (DIRECT_BACKEND, bits, reference.phase_counters)
    }
    Strategy::Canonical => {
        let bits = rstim::executor::reference_sample(instructions)?;
        let counters = canonical_phase_counters(instructions, state.measurement_bits);
        (CANONICAL_BACKEND, bits, counters)
    }
};
let packed = pack_b8(&bits);
```

Use `backend` in the response instead of the old backend constant.

- [ ] **Step 4: Run tests to verify they pass**

Run:

```sh
cargo test -p rstim --test rstim_reference_build_worker
```

Expected: PASS.

- [ ] **Step 5: Commit**

```sh
git add rstim/src/bin/rstim_reference_build_worker.rs rstim/tests/rstim_reference_build_worker.rs
git commit -m "feat: add reference build worker strategies"
```

---

### Task 2: Three-Variant Runner

**Files:**
- Modify: `benchmarks/rstim_vs_stim_simulator/run_reference_build_benchmark.py`
- Modify: `benchmarks/rstim_vs_stim_simulator/tests/test_run_reference_build_benchmark.py`

**Interfaces:**
- Consumes: Task 1 worker `--strategy canonical`.
- Produces: runner artifacts with 27 raw records, three summary variants, `direct_speedup`, phase counters for rstim variants, and `baseline-summary.json` in the hash manifest.

- [ ] **Step 1: Write failing runner tests**

In `benchmarks/rstim_vs_stim_simulator/tests/test_run_reference_build_benchmark.py`, replace the rstim constants with:

```python
RSTIM_CANONICAL_VARIANT = "rstim-canonical-reference-b8"
RSTIM_DIRECT_VARIANT = "rstim-direct-repeat-reference-b8"
```

Update the fake worker to accept strategy:

```python
parser.add_argument("--strategy", choices=["direct", "canonical"], default="direct")
backend = "canonical_roundtrip" if args.strategy == "canonical" else {backend!r}
phase_counters = {
    "measurement_reset_batches": 5,
    "canonical_materializations": 12121 if args.strategy == "canonical" else 0,
    "canonical_writebacks": 12121 if args.strategy == "canonical" else 0,
    "direct_inverse_batches": 0 if args.strategy == "canonical" else 5,
    "transposed_collapse_batches": 0 if args.strategy == "canonical" else 2,
    "collapse_pivots": 120,
    "expanded_repeat_iterations": 99,
    "executed_repeat_iterations": 99 if args.strategy == "canonical" else 1,
    "skipped_repeat_iterations": 0 if args.strategy == "canonical" else 98,
    "measurement_bits": 12121,
}
```

Add `"phase_counters": phase_counters` to fake `reference_built` responses.

Update `test_fake_workers_emit_required_artifacts_and_hash_manifest` expectations:

```python
expected_files = {
    "raw.jsonl",
    "summary.json",
    "baseline-summary.json",
    "report.md",
    "environment.json",
    "artifact-sha256.json",
}
self.assertEqual(len(raw), 27)
self.assertEqual({record["variant"] for record in raw}, {STIM_VARIANT, RSTIM_CANONICAL_VARIANT, RSTIM_DIRECT_VARIANT})
self.assertEqual(summary["measured_records"], 21)
self.assertGreaterEqual(summary["direct_speedup"], 2.0)
self.assertEqual(hash_manifest["baseline-summary.json"], "614658cf8213b486752f1fe53b7d864561abbe41c2eefd799fc8fa34883270a5")
```

Update expected worker argv:

```python
expected_worker_argv = {
    STIM_VARIANT: ["tool://stim-python", "-m", "benchmarks.rstim_vs_stim_simulator.workers.stim_reference_build", "--protocol", PROTOCOL],
    RSTIM_CANONICAL_VARIANT: ["tool://rstim-reference-worker", "--protocol", PROTOCOL, "--strategy", "canonical"],
    RSTIM_DIRECT_VARIANT: ["tool://rstim-reference-worker", "--protocol", PROTOCOL],
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```sh
python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_run_reference_build_benchmark
```

Expected: FAIL because the runner still emits two variants and no `baseline-summary.json`.

- [ ] **Step 3: Implement runner variant metadata**

In `run_reference_build_benchmark.py`, replace the rstim constants with:

```python
RSTIM_CANONICAL_VARIANT = "rstim-canonical-reference-b8"
RSTIM_DIRECT_VARIANT = "rstim-direct-repeat-reference-b8"
RSTIM_CANONICAL_BACKEND = "canonical_roundtrip"
RSTIM_DIRECT_BACKEND = "direct_inverse_repeat_folded"
BASELINE_SUMMARY_SHA256 = "614658cf8213b486752f1fe53b7d864561abbe41c2eefd799fc8fa34883270a5"
```

Update worker argv helpers:

```python
def default_rstim_worker_argv(rstim_worker: str, *, strategy: str = "direct") -> list[str]:
    argv = [rstim_worker, "--protocol", PROTOCOL]
    if strategy != "direct":
        argv.extend(["--strategy", strategy])
    return argv

def logical_rstim_worker_argv(*, strategy: str = "direct") -> list[str]:
    return default_rstim_worker_argv(RSTIM_WORKER_ROLE, strategy=strategy)
```

Add:

```python
def _rstim_worker_has_phase_counters(variant: str) -> bool:
    return variant in {RSTIM_CANONICAL_VARIANT, RSTIM_DIRECT_VARIANT}
```

Send `"include_phase_counters": True` in `_run_variant` requests for rstim variants and include `phase_counters` in raw records when present.

- [ ] **Step 4: Implement summary, report, environment, and baseline preservation**

Update `derive_summary` to iterate:

```python
for variant, backend in (
    (STIM_VARIANT, STIM_BACKEND),
    (RSTIM_CANONICAL_VARIANT, RSTIM_CANONICAL_BACKEND),
    (RSTIM_DIRECT_VARIANT, RSTIM_DIRECT_BACKEND),
):
```

Compute:

```python
canonical_median = next(item["median_elapsed_ns"] for item in variants if item["variant"] == RSTIM_CANONICAL_VARIANT)
direct_median = next(item["median_elapsed_ns"] for item in variants if item["variant"] == RSTIM_DIRECT_VARIANT)
direct_speedup = canonical_median / direct_median
```

Return `measured_records: 21` and `direct_speedup: round(direct_speedup, 6)`.

Update `render_report` to append:

```python
lines.extend(["", f"direct_speedup={summary['direct_speedup']:.6f}"])
```

Update `collect_environment` worker argv maps to all three variants, with canonical strategy only on the canonical variant.

Add:

```python
def preserve_baseline_summary(out_dir: Path) -> None:
    source = out_dir / "summary.json"
    target = out_dir / "baseline-summary.json"
    if target.exists():
        if sha256_file(target) != BASELINE_SUMMARY_SHA256:
            raise RunnerError("baseline-summary.json SHA-256 mismatch")
        return
    if not source.is_file():
        raise RunnerError("cannot preserve baseline summary before summary.json exists")
    if sha256_file(source) != BASELINE_SUMMARY_SHA256:
        raise RunnerError("existing summary.json SHA-256 does not match required baseline")
    target.write_bytes(source.read_bytes())
```

Call `preserve_baseline_summary(out_dir)` before writing the new `summary.json`.

Update `write_artifact_hashes` filenames:

```python
filenames = ("raw.jsonl", "summary.json", "baseline-summary.json", "report.md", "environment.json")
```

- [ ] **Step 5: Run tests to verify they pass**

Run:

```sh
python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_run_reference_build_benchmark
```

Expected: PASS.

- [ ] **Step 6: Commit**

```sh
git add benchmarks/rstim_vs_stim_simulator/run_reference_build_benchmark.py benchmarks/rstim_vs_stim_simulator/tests/test_run_reference_build_benchmark.py
git commit -m "feat: run paired reference build variants"
```

---

### Task 3: Checker Contract and Negative Controls

**Files:**
- Modify: `tools/check_rstim_vs_stim_reference_build_evidence.py`
- Modify: `tools/test_check_rstim_vs_stim_reference_build_evidence.py`
- Modify: `tools/check_all_portable_evidence.py` if its pass-line adapter needs `direct_speedup`.

**Interfaces:**
- Consumes: Task 2 artifact schema.
- Produces: checker that accepts only the three-variant evidence bundle and rejects the issue's negative controls before artifact hashes.

- [ ] **Step 1: Write failing checker tests**

Update `tools/test_check_rstim_vs_stim_reference_build_evidence.py` constants:

```python
RSTIM_CANONICAL_VARIANT = "rstim-canonical-reference-b8"
RSTIM_DIRECT_VARIANT = "rstim-direct-repeat-reference-b8"
```

Make `write_valid_bundle` emit 27 records. Canonical records use backend
`canonical_roundtrip`, direct records use backend `direct_inverse_repeat_folded`.
Every rstim record includes phase counters:

```python
def phase_counters(*, canonical: bool) -> dict[str, int]:
    return {
        "measurement_reset_batches": 5,
        "canonical_materializations": 12121 if canonical else 0,
        "canonical_writebacks": 12121 if canonical else 0,
        "direct_inverse_batches": 0 if canonical else 5,
        "transposed_collapse_batches": 0 if canonical else 2,
        "collapse_pivots": 120,
        "expanded_repeat_iterations": 99,
        "executed_repeat_iterations": 99 if canonical else 1,
        "skipped_repeat_iterations": 0 if canonical else 98,
        "measurement_bits": MEASUREMENT_BITS,
    }
```

Write `baseline-summary.json` with content whose digest is
`614658cf8213b486752f1fe53b7d864561abbe41c2eefd799fc8fa34883270a5`; for unit
tests, copy the repository's committed `baseline-summary.json` when present,
otherwise copy the current release `summary.json` before mutation.

Add tests:

```python
def test_rejects_direct_variant_with_canonical_materializations(self) -> None:
    records = load_raw(self.bundle / "raw.jsonl")
    next(record for record in records if record["variant"] == RSTIM_DIRECT_VARIANT)["phase_counters"]["canonical_materializations"] = 1
    rewrite_raw(self.bundle / "raw.jsonl", records)
    rewrite_artifact_hashes(self.bundle)
    result = self.run_checker()
    self.assertNotEqual(result.returncode, 0, result.stdout)
    self.assertIn("direct reference canonical_materializations must be integer 0", result.stderr)
    self.assertNotIn("artifact-sha256.json", result.stderr)

def test_rejects_direct_speedup_below_two(self) -> None:
    records = load_raw(self.bundle / "raw.jsonl")
    for record in records:
        if record["variant"] == RSTIM_DIRECT_VARIANT and record["phase"] == "measured":
            record["elapsed_ns"] = 900
        if record["variant"] == RSTIM_CANONICAL_VARIANT and record["phase"] == "measured":
            record["elapsed_ns"] = 1000
    rewrite_raw(self.bundle / "raw.jsonl", records)
    summary = derive_summary(records)
    (self.bundle / "summary.json").write_text(json.dumps(summary, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    (self.bundle / "report.md").write_text(render_report(summary), encoding="utf-8")
    rewrite_artifact_hashes(self.bundle)
    result = self.run_checker()
    self.assertNotEqual(result.returncode, 0, result.stdout)
    self.assertIn("direct reference speedup must be at least 2.0x", result.stderr)
    self.assertNotIn("artifact-sha256.json", result.stderr)
```

Update `test_accepts_valid_bundle` expected stdout to:

```python
self.assertRegex(result.stdout, r"^PASS packed reference-build evidence variants=3 direct_speedup=\\d+\\.\\d{6}\\n$")
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```sh
python3 -m unittest tools.test_check_rstim_vs_stim_reference_build_evidence
```

Expected: FAIL because the checker still expects two variants.

- [ ] **Step 3: Implement checker schema**

Update checker constants:

```python
REQUIRED_FILES = ("raw.jsonl", "summary.json", "baseline-summary.json", "report.md", "environment.json", "artifact-sha256.json")
ARTIFACT_FILES = REQUIRED_FILES[:-1]
VARIANTS = (runner.STIM_VARIANT, runner.RSTIM_CANONICAL_VARIANT, runner.RSTIM_DIRECT_VARIANT)
BACKENDS = {
    runner.STIM_VARIANT: runner.STIM_BACKEND,
    runner.RSTIM_CANONICAL_VARIANT: runner.RSTIM_CANONICAL_BACKEND,
    runner.RSTIM_DIRECT_VARIANT: runner.RSTIM_DIRECT_BACKEND,
}
BASELINE_SUMMARY_SHA256 = runner.BASELINE_SUMMARY_SHA256
```

Update `validate_raw_semantics` to expect 27 records, three variants, and nine
records per variant. Decode bytes and byte digests before artifact hashes as it
does today.

Add:

```python
PHASE_COUNTER_KEYS = (
    "measurement_reset_batches",
    "canonical_materializations",
    "canonical_writebacks",
    "direct_inverse_batches",
    "transposed_collapse_batches",
    "collapse_pivots",
    "expanded_repeat_iterations",
    "executed_repeat_iterations",
    "skipped_repeat_iterations",
    "measurement_bits",
)

def _validate_phase_counters(record: dict[str, Any], variant: str) -> dict[str, int]:
    counters = record.get("phase_counters")
    if not isinstance(counters, dict):
        raise ValueError(f"{variant} phase_counters must be a JSON object")
    if set(counters) != set(PHASE_COUNTER_KEYS):
        raise ValueError(f"{variant} phase_counters must contain the canonical key set")
    validated = {}
    for key in PHASE_COUNTER_KEYS:
        value = counters[key]
        if not isinstance(value, int) or isinstance(value, bool) or value < 0:
            raise ValueError(f"{variant} phase_counters {key} must be a nonnegative integer")
        validated[key] = value
    return validated
```

For `RSTIM_DIRECT_VARIANT`, require:

```python
_require_int_equal(counters["canonical_materializations"], 0, "direct reference canonical_materializations")
_require_int_equal(counters["executed_repeat_iterations"], 1, "direct reference executed_repeat_iterations")
_require_int_equal(counters["skipped_repeat_iterations"], 98, "direct reference skipped_repeat_iterations")
_require_int_equal(counters["measurement_bits"], runner.EXPECTED_MEASUREMENT_BITS, "direct reference phase counter measurement_bits")
```

- [ ] **Step 4: Implement speedup, baseline, environment, and pass line validation**

After summary derivation, validate speedup:

```python
def validate_direct_speedup(summary: dict[str, Any]) -> float:
    by_variant = {item["variant"]: item for item in summary["variants"]}
    canonical = by_variant[runner.RSTIM_CANONICAL_VARIANT]["median_elapsed_ns"]
    direct = by_variant[runner.RSTIM_DIRECT_VARIANT]["median_elapsed_ns"]
    speedup = canonical / direct
    if speedup < 2.0:
        raise ValueError("direct reference speedup must be at least 2.0x")
    if summary.get("direct_speedup") != round(speedup, 6):
        raise ValueError("summary.json direct_speedup does not match variant medians")
    return speedup
```

Validate `baseline-summary.json` digest before environment and artifact hashes:

```python
if sha256_file(results_dir / "baseline-summary.json") != BASELINE_SUMMARY_SHA256:
    raise ValueError("baseline-summary.json SHA-256 must match preserved pre-optimization summary")
```

Update `_validate_worker_argv` expected canonical map to include all three variants.
The direct rstim argv must be `[RSTIM_WORKER_ROLE, "--protocol", runner.PROTOCOL]`.
The canonical rstim argv must be `[RSTIM_WORKER_ROLE, "--protocol", runner.PROTOCOL, "--strategy", "canonical"]`.

Update `_validate_runner_argv` length/tail only if runner args changed. The runner
command remains one `--rstim-worker`, so the length stays 17.

Update `main`:

```python
result = validate_bundle(args.results_dir, args.verify_runtime_binary)
print(f"PASS packed reference-build evidence variants=3 direct_speedup={result['direct_speedup']:.6f}")
```

Update `tools/check_all_portable_evidence.py` if it formats the reference-build
pass line itself; it must preserve `variants=3 direct_speedup=...`.

- [ ] **Step 5: Run tests to verify they pass**

Run:

```sh
python3 -m unittest tools.test_check_rstim_vs_stim_reference_build_evidence
```

Expected: PASS.

- [ ] **Step 6: Commit**

```sh
git add tools/check_rstim_vs_stim_reference_build_evidence.py tools/test_check_rstim_vs_stim_reference_build_evidence.py tools/check_all_portable_evidence.py
git commit -m "test: enforce paired reference build evidence"
```

---

### Task 4: Refresh Reference-Build Evidence Bundle

**Files:**
- Modify: `benchmarks/rstim_vs_stim_simulator/results/reference-build-release/raw.jsonl`
- Modify: `benchmarks/rstim_vs_stim_simulator/results/reference-build-release/summary.json`
- Create: `benchmarks/rstim_vs_stim_simulator/results/reference-build-release/baseline-summary.json`
- Modify: `benchmarks/rstim_vs_stim_simulator/results/reference-build-release/report.md`
- Modify: `benchmarks/rstim_vs_stim_simulator/results/reference-build-release/environment.json`
- Modify: `benchmarks/rstim_vs_stim_simulator/results/reference-build-release/artifact-sha256.json`

**Interfaces:**
- Consumes: Tasks 1 through 3.
- Produces: committed release evidence that passes the checker command from issue #490.

- [ ] **Step 1: Build the release worker**

Run:

```sh
cargo build --release -p rstim --bin rstim_reference_build_worker
```

Expected: PASS.

- [ ] **Step 2: Run the benchmark to refresh the catalog slot**

Run:

```sh
python3 -m benchmarks.rstim_vs_stim_simulator.run_reference_build_benchmark \
  --fixture benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim \
  --manifest benchmarks/rstim_vs_stim_simulator/cases.full.toml \
  --stim-python python3 \
  --rstim-worker target/release/rstim_reference_build_worker \
  --warmup-rounds 2 \
  --measure-rounds 7 \
  --out-dir benchmarks/rstim_vs_stim_simulator/results/reference-build-release
```

Expected stdout starts:

```text
PASS packed reference-build benchmark variants=3 measured=21 direct_speedup=
```

- [ ] **Step 3: Verify the refreshed bundle**

Run:

```sh
python3 tools/check_rstim_vs_stim_reference_build_evidence.py \
  --dir benchmarks/rstim_vs_stim_simulator/results/reference-build-release
```

Expected stdout starts:

```text
PASS packed reference-build evidence variants=3 direct_speedup=
```

- [ ] **Step 4: Check baseline summary digest**

Run:

```sh
python3 - <<'PY'
from pathlib import Path
import hashlib
path = Path("benchmarks/rstim_vs_stim_simulator/results/reference-build-release/baseline-summary.json")
print(hashlib.sha256(path.read_bytes()).hexdigest())
PY
```

Expected:

```text
614658cf8213b486752f1fe53b7d864561abbe41c2eefd799fc8fa34883270a5
```

- [ ] **Step 5: Commit**

```sh
git add benchmarks/rstim_vs_stim_simulator/results/reference-build-release
git commit -m "data: publish paired reference build evidence"
```

---

### Task 5: Full Verification Sweep

**Files:**
- No source files expected.

**Interfaces:**
- Consumes: completed implementation and refreshed evidence.
- Produces: final verification evidence for the PR.

- [ ] **Step 1: Run focused Python tests**

Run:

```sh
python3 -m unittest tools.test_check_rstim_vs_stim_reference_build_evidence benchmarks.rstim_vs_stim_simulator.tests.test_run_reference_build_benchmark
```

Expected: PASS.

- [ ] **Step 2: Run the required checker**

Run:

```sh
python3 tools/check_rstim_vs_stim_reference_build_evidence.py \
  --dir benchmarks/rstim_vs_stim_simulator/results/reference-build-release
```

Expected stdout starts:

```text
PASS packed reference-build evidence variants=3 direct_speedup=
```

- [ ] **Step 3: Run the required Cargo test command**

Run:

```sh
cargo test
```

Expected: PASS.

- [ ] **Step 4: Commit any verification-only fixes**

If verification uncovered a fix, commit the smallest targeted change:

```sh
git status --short
git add rstim/src/bin/rstim_reference_build_worker.rs \
  rstim/tests/rstim_reference_build_worker.rs \
  benchmarks/rstim_vs_stim_simulator/run_reference_build_benchmark.py \
  benchmarks/rstim_vs_stim_simulator/tests/test_run_reference_build_benchmark.py \
  tools/check_rstim_vs_stim_reference_build_evidence.py \
  tools/test_check_rstim_vs_stim_reference_build_evidence.py \
  tools/check_all_portable_evidence.py \
  benchmarks/rstim_vs_stim_simulator/results/reference-build-release
git commit -m "fix: complete paired reference build verification"
```

If no files changed, do not create an empty commit.
