# Issue 279 BB90 Hard Replay Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a cheap hard-syndrome replay tier that compares Rust `rbposd` in `ldpc`-compatible OSD-CS mode against Python `ldpc.BpOsdDecoder` on the checked-in BB90 fixture.

**Architecture:** Reuse the existing `rsinter bb-circuit-bposd-memory --json-compare-case` model export as the shared source of effective decoder data. Extend that export with per-trial Rust logical predictions and per-basis decode stats, then add a Python `hard-replay` runner tier and a replay-specific CSV verifier.

**Tech Stack:** Rust 2024 workspace; `rsinter` and `rbposd`; Python 3 standard library tests using `unittest`; optional local `ldpc`, `bposd`, and `numpy`; `cargo test`.

## Global Constraints

- Keep this as a replay diagnostic, not a Monte Carlo campaign.
- The hard replay must use case id `bb90-p006-c10-seed12345-order7-hard-syndrome`.
- The hard replay must use basis `Z`.
- The hard replay must use `bp_method=ms`, `max_iter=10000`, `osd_method=osd_cs`, and `osd_order=7`.
- The Rust decoder must be configured through `OsdVariant::LdpcCombinationSweep` for `osd_method=osd_cs`.
- Missing Python dependencies must produce skipped Python rows and a nonzero verifier result unless an explicit allow-missing mode is used.
- Rust hard-replay rows must include OSD/GF(2) counters from the #212 profile surface.
- Do not add full 50,000-trial campaign runs.
- Do not add plotting.
- Do not optimize the Rust hot path.

---

## File Structure

- Modify `rsinter/src/bb_circuit_memory.rs`: allow compare exports to run with an explicit OSD variant and include per-trial logical predictions plus per-basis decode profiles.
- Modify `rsinter/src/bin/rsinter.rs`: add optional `--osd-method` to `bb-circuit-bposd-memory`.
- Modify `rsinter/tests/bench_cli.rs`: add failing CLI coverage for `--osd-method osd_cs` and per-trial replay fields.
- Modify `benchmarks/bb_circuit_bposd_compare/cases.py`: add hard-replay case metadata and replay CSV columns.
- Modify `benchmarks/bb_circuit_bposd_compare/run_compare.py`: add `--tier hard-replay`, `--rust-binary`, hard-replay row generation, and Rust command selection.
- Modify `benchmarks/bb_circuit_bposd_compare/tests/test_run_compare.py`: add hard-replay runner tests.
- Create `benchmarks/bb_circuit_bposd_compare/verify_replay.py`: validate hard-replay CSV rows.
- Create `benchmarks/bb_circuit_bposd_compare/tests/test_verify_replay.py`: test replay verifier positive and negative controls.
- Modify `benchmarks/bb_circuit_bposd_compare/README.md`: document the hard-replay commands.

### Task 1: Rust Export Carries ldpc OSD Selection And Trial Replay Fields

**Files:**
- Modify: `rsinter/src/bin/rsinter.rs`
- Modify: `rsinter/src/bb_circuit_memory.rs`
- Modify: `rsinter/tests/bench_cli.rs`

**Interfaces:**
- Consumes: `rbposd::OsdVariant::from_method_name`, existing `SimulationConfig`, existing `ComparisonTrialExport`.
- Produces:
  - `export_comparison_case_for_code_with_osd_variant(code_id: &str, config: SimulationConfig, osd_variant: rbposd::OsdVariant) -> Result<BbCircuitBposdComparisonExport, String>`
  - optional CLI argument `--osd-method <method>`
  - JSON fields `z_logical_prediction`, `x_logical_prediction`, `z_profile`, and `x_profile` on each collected trial.

- [ ] **Step 1: Write the failing CLI/export test**

Add this test after `rsinter_bb_circuit_bposd_memory_json_compare_case_prints_profile_bundle` in `rsinter/tests/bench_cli.rs`:

```rust
#[test]
fn rsinter_json_compare_case_accepts_ldpc_osd_method_and_exports_trial_predictions() {
    let output = Command::new(env!("CARGO_BIN_EXE_rsinter"))
        .args([
            "bb-circuit-bposd-memory",
            "--code-id",
            "bb72",
            "--physical-error-rate",
            "0.000000000001",
            "--num-cycles",
            "1",
            "--num-trials",
            "1",
            "--seed",
            "12345",
            "--max-bp-iterations",
            "10",
            "--osd-order",
            "0",
            "--osd-method",
            "osd_cs",
            "--json-compare-case",
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "{output:?}");
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let trial = &json["trials"][0];
    assert_eq!(trial["z_logical_prediction"].as_array().unwrap().len(), 12);
    assert_eq!(trial["z_profile"]["decode_call_count"], 1);
    assert!(trial["z_profile"]["decode_seconds"].as_f64().unwrap() >= 0.0);
    assert!(trial["x_logical_prediction"].as_array().is_some());
    assert_eq!(trial["x_profile"]["decode_call_count"], 1);
}
```

- [ ] **Step 2: Run the failing test**

Run:

```bash
cargo test -p rsinter rsinter_json_compare_case_accepts_ldpc_osd_method_and_exports_trial_predictions -- --nocapture
```

Expected: FAIL because the CLI does not yet accept `--osd-method` and the export does not yet include per-trial prediction/profile fields.

- [ ] **Step 3: Add explicit OSD variant export plumbing**

In `rsinter/src/bb_circuit_memory.rs`, change the import to include `OsdVariant`:

```rust
use rbposd::{
    BpOsdDecoder, ChannelModel, Correction, DecodeResult, DecodeStats, DecoderConfig, OsdVariant,
    ParityCheckMatrix, Syndrome,
};
```

Add these fields to `ComparisonTrialExport`:

```rust
    pub z_logical_prediction: Option<Vec<bool>>,
    pub x_logical_prediction: Option<Vec<bool>>,
    pub z_profile: Option<BbCircuitBposdProfile>,
    pub x_profile: Option<BbCircuitBposdProfile>,
```

Change `comparison_trial_export` so the new fields initialize to `None`:

```rust
fn comparison_trial_export(sample: &SampledTrial) -> ComparisonTrialExport {
    ComparisonTrialExport {
        z_syndrome: sample.z_syndrome.clone(),
        x_syndrome: sample.x_syndrome.clone(),
        z_logical: sample.z_logical.clone(),
        x_logical: sample.x_logical.clone(),
        z_logical_prediction: None,
        x_logical_prediction: None,
        z_profile: None,
        x_profile: None,
    }
}
```

Replace `export_comparison_case_for_code` with a wrapper plus explicit variant function:

```rust
pub fn export_comparison_case_for_code(
    code_id: &str,
    config: SimulationConfig,
) -> Result<BbCircuitBposdComparisonExport, String> {
    export_comparison_case_for_code_with_osd_variant(code_id, config, OsdVariant::Osd0)
}

pub fn export_comparison_case_for_code_with_osd_variant(
    code_id: &str,
    config: SimulationConfig,
    osd_variant: OsdVariant,
) -> Result<BbCircuitBposdComparisonExport, String> {
    let run = run_simulation_case_for_code_with_osd_variant(
        code_id,
        config.clone(),
        osd_variant,
        true,
    )?;

    Ok(BbCircuitBposdComparisonExport {
        code_id: code_id.to_owned(),
        physical_error_rate: config.physical_error_rate,
        num_cycles: config.num_cycles,
        num_trials: config.num_trials,
        seed: config.seed,
        max_bp_iterations: config.max_bp_iterations,
        osd_order: config.osd_order,
        rust_result: run.result,
        z_model: comparison_model_export(&run.models.z_faults),
        x_model: comparison_model_export(&run.models.x_faults),
        trials: run.trials.unwrap_or_default(),
    })
}
```

Change `run_simulation_for_code` to call a new internal helper:

```rust
pub fn run_simulation_for_code(
    code_id: &str,
    config: SimulationConfig,
) -> Result<SimulationResult, String> {
    Ok(run_simulation_case_for_code_with_osd_variant(
        code_id,
        config,
        OsdVariant::Osd0,
        false,
    )?
    .result)
}

fn run_simulation_case_for_code_with_osd_variant(
    code_id: &str,
    config: SimulationConfig,
    osd_variant: OsdVariant,
    collect_trials: bool,
) -> Result<SimulationCaseRun, String> {
```

Remove the old `fn run_simulation_case_for_code(...)` wrapper or leave it only if existing callers still use it; no caller should duplicate the decode loop.

Inside the decoder config construction, set the variant:

```rust
    let decoder_config = DecoderConfig {
        max_bp_iterations: config.max_bp_iterations,
        osd_variant,
        osd_order: config.osd_order,
        ..DecoderConfig::default()
    };
```

Add this helper near `comparison_trial_export`:

```rust
fn profile_from_decode_stats(
    basis: ProfileReplayBasis,
    decode_seconds: f64,
    stats: &DecodeStats,
) -> BbCircuitBposdProfile {
    let mut profile = BbCircuitBposdProfile {
        decode_seconds,
        ..BbCircuitBposdProfile::default()
    };
    profile.add_basis_stats(basis, stats);
    profile
}
```

- [ ] **Step 4: Populate per-trial fields in the decode loop**

In the `for _ in 0..config.num_trials` loop, replace the early trial push with a mutable collected trial:

```rust
        let mut trial_export = collect_trials.then(|| comparison_trial_export(&sample));
```

After the Z decode call, compute and store the Z prediction/profile before checking logical failure:

```rust
        let z_decode_seconds = decode_started.elapsed().as_secs_f64();
        profile.decode_seconds += z_decode_seconds;
        profile.add_z_stats(&z_result.stats);
        let predicted_z = correction_to_logicals(&z_result.correction, &models.z_faults, code.k());
        if let Some(trial) = trial_export.as_mut() {
            trial.z_logical_prediction = Some(predicted_z.clone());
            trial.z_profile = Some(profile_from_decode_stats(
                ProfileReplayBasis::Z,
                z_decode_seconds,
                &z_result.stats,
            ));
        }
        if predicted_z != sample.z_logical {
            if let (Some(trials), Some(trial)) = (trials.as_mut(), trial_export.take()) {
                trials.push(trial);
            }
            num_failed_trials += 1;
            continue;
        }
```

After the X decode call, compute and store the X prediction/profile:

```rust
        let x_decode_seconds = decode_started.elapsed().as_secs_f64();
        profile.decode_seconds += x_decode_seconds;
        profile.add_x_stats(&x_result.stats);
        let predicted_x = correction_to_logicals(&x_result.correction, &models.x_faults, code.k());
        if let Some(trial) = trial_export.as_mut() {
            trial.x_logical_prediction = Some(predicted_x.clone());
            trial.x_profile = Some(profile_from_decode_stats(
                ProfileReplayBasis::X,
                x_decode_seconds,
                &x_result.stats,
            ));
        }
        if predicted_x != sample.x_logical {
            num_failed_trials += 1;
        }
        if let (Some(trials), Some(trial)) = (trials.as_mut(), trial_export.take()) {
            trials.push(trial);
        }
```

- [ ] **Step 5: Add CLI parsing for `--osd-method`**

In `rsinter/src/bin/rsinter.rs`, import `OsdVariant`:

```rust
use rbposd::OsdVariant;
```

Add the optional argument to `Commands::BbCircuitBposdMemory`:

```rust
        #[arg(long)]
        osd_method: Option<String>,
```

Thread it through the match arm and choose the export function:

```rust
            let osd_variant = match osd_method.as_deref() {
                Some(method) => Some(OsdVariant::from_method_name(method).map_err(|e| e.to_string())?),
                None => None,
            };
            if json_compare_case {
                let export = match osd_variant {
                    Some(osd_variant) => export_comparison_case_for_code_with_osd_variant(
                        &code_id,
                        config,
                        osd_variant,
                    )?,
                    None => export_comparison_case_for_code(&code_id, config)?,
                };
```

For the non-JSON four-column path, keep existing behavior when `--osd-method` is omitted and route the explicit method through a new public simulation wrapper:

```rust
                let result = match osd_variant {
                    Some(osd_variant) => run_simulation_for_code_with_osd_variant(
                        &code_id,
                        config,
                        osd_variant,
                    )?,
                    None => run_simulation_for_code(&code_id, config)?,
                };
```

Add `run_simulation_for_code_with_osd_variant` beside `run_simulation_for_code` in `bb_circuit_memory.rs`:

```rust
pub fn run_simulation_for_code_with_osd_variant(
    code_id: &str,
    config: SimulationConfig,
    osd_variant: OsdVariant,
) -> Result<SimulationResult, String> {
    Ok(run_simulation_case_for_code_with_osd_variant(code_id, config, osd_variant, false)?.result)
}
```

- [ ] **Step 6: Run the focused Rust test**

Run:

```bash
cargo test -p rsinter rsinter_json_compare_case_accepts_ldpc_osd_method_and_exports_trial_predictions -- --nocapture
```

Expected: PASS.

- [ ] **Step 7: Commit Task 1**

Run:

```bash
git add rsinter/src/bin/rsinter.rs rsinter/src/bb_circuit_memory.rs rsinter/tests/bench_cli.rs
git commit -m "feat: export BB compare replay predictions"
```

### Task 2: Python hard-replay Runner And Expanded CSV Rows

**Files:**
- Modify: `benchmarks/bb_circuit_bposd_compare/cases.py`
- Modify: `benchmarks/bb_circuit_bposd_compare/run_compare.py`
- Modify: `benchmarks/bb_circuit_bposd_compare/tests/test_run_compare.py`

**Interfaces:**
- Consumes: Rust JSON fields from Task 1.
- Produces:
  - `HARD_REPLAY_CASES`
  - `run_hard_replay_suite(output_dir: Path, allow_missing_python: bool = False, rust_binary: Path | None = None, rust_exporter: Callable[[CompareCase], dict[str, Any]] | None = None) -> int`
  - CLI `--tier hard-replay`
  - CLI `--rust-binary`

- [ ] **Step 1: Write failing Python runner tests**

In `benchmarks/bb_circuit_bposd_compare/tests/test_run_compare.py`, add `import json` at the top and import `HARD_REPLAY_CASES`:

```python
from benchmarks.bb_circuit_bposd_compare.cases import HARD_REPLAY_CASES, SMOKE_CASES
```

Add this helper after `fake_export`:

```python
FAKE_HARD_LOGICAL = [False, True, False, True, False, False, False, True]


def fake_hard_fixture():
    return {
        "case_id": HARD_REPLAY_CASES[0].case_id,
        "basis": "Z",
        "syndrome_support": [0, 2, 3],
        "expected_sampled_logical": FAKE_HARD_LOGICAL,
    }


def fake_hard_export(case):
    return {
        "code_id": "bb90",
        "physical_error_rate": 0.006,
        "num_cycles": 10,
        "num_trials": 1,
        "seed": 12345,
        "max_bp_iterations": 10000,
        "osd_order": 7,
        "rust_result": {
            "num_failed_trials": 0,
            "profile": {"setup_seconds": 0.11, "decode_seconds": 0.22},
        },
        "z_model": {
            "num_checks": 4,
            "num_bits": 4,
            "sparse_rows": [[0], [1], [2], [3]],
            "augmented_columns": [[4], [], [5], [6, 7]],
            "channel_probs": [0.1, 0.1, 0.1, 0.1],
            "first_logical_row": 4,
        },
        "x_model": {
            "num_checks": 1,
            "num_bits": 1,
            "sparse_rows": [[]],
            "augmented_columns": [[]],
            "channel_probs": [0.1],
            "first_logical_row": 1,
        },
        "trials": [
            {
                "z_syndrome": [True, False, True, True],
                "x_syndrome": [False],
                "z_logical": FAKE_HARD_LOGICAL,
                "x_logical": [False],
                "z_logical_prediction": FAKE_HARD_LOGICAL,
                "x_logical_prediction": [False],
                "z_profile": {
                    "setup_seconds": 0.0,
                    "sample_seconds": 0.0,
                    "decode_seconds": 0.22,
                    "bp_seconds": 0.12,
                    "osd_seconds": 0.10,
                    "decode_call_count": 1,
                    "z_decode_call_count": 1,
                    "x_decode_call_count": 0,
                    "bp_iteration_count": 10000,
                    "osd_use_count": 1,
                    "osd_candidate_count": 4100,
                    "gf2_solve_count": 4101,
                    "gf2_full_elimination_count": 1,
                },
                "x_profile": None,
            }
        ],
    }
```

Add a fake decoder that returns the same logical prediction through the model's augmented columns:

```python
class FakeHardMatrix:
    def __init__(self, shape):
        rows, cols = shape
        self.rows = [[0 for _ in range(cols)] for _ in range(rows)]

    def __setitem__(self, key, value):
        row_index, column_index = key
        self.rows[row_index][column_index] = value


class FakeHardNumpy(ModuleType):
    uint8 = "uint8"

    def __init__(self):
        super().__init__("numpy")

    def zeros(self, shape, dtype=None):
        return FakeHardMatrix(shape)

    def asarray(self, values, dtype=None):
        return list(values)


class FakeHardVector:
    def __init__(self, values):
        self._values = list(values)

    def tolist(self):
        return list(self._values)


class FakeHardDecoder:
    def __init__(self, matrix, **kwargs):
        self.kwargs = kwargs

    def decode(self, syndrome):
        return FakeHardVector([True, False, True, True])
```

Add this test method to `RunCompareTest`:

```python
    def test_hard_replay_suite_writes_paired_prediction_rows(self) -> None:
        fake_ldpc = ModuleType("ldpc")
        fake_ldpc.BpOsdDecoder = FakeHardDecoder

        with tempfile.TemporaryDirectory() as tmpdir:
            with mock.patch.dict("sys.modules", {"numpy": FakeHardNumpy(), "ldpc": fake_ldpc}):
                with mock.patch(
                    "benchmarks.bb_circuit_bposd_compare.run_compare._load_hard_replay_fixture",
                    side_effect=fake_hard_fixture,
                ):
                    status = run_hard_replay_suite(
                        Path(tmpdir),
                        rust_exporter=fake_hard_export,
                    )
            with (Path(tmpdir) / "results.csv").open(newline="") as handle:
                rows = list(csv.DictReader(handle))

        self.assertEqual(status, 0)
        self.assertEqual([row["decoder_impl"] for row in rows], ["rbposd", "ldpc_bposd"])
        self.assertEqual(rows[0]["case_id"], HARD_REPLAY_CASES[0].case_id)
        self.assertEqual(rows[0]["basis"], "Z")
        self.assertEqual(rows[0]["osd_method"], "osd_cs")
        self.assertEqual(rows[0]["logical_prediction"], rows[1]["logical_prediction"])
        self.assertEqual(rows[0]["syndrome_support"], "[0,2,3]")
        self.assertEqual(rows[0]["osd_candidate_count"], "4100")
        self.assertEqual(rows[0]["gf2_solve_count"], "4101")
```

Add this missing-dependency test:

```python
    def test_hard_replay_suite_records_skipped_python_dependency_row(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            with mock.patch(
                "benchmarks.bb_circuit_bposd_compare.run_compare._python_hard_replay_row",
                side_effect=ModuleNotFoundError("No module named 'ldpc'"),
            ):
                with mock.patch(
                    "benchmarks.bb_circuit_bposd_compare.run_compare._load_hard_replay_fixture",
                    side_effect=fake_hard_fixture,
                ):
                    status = run_hard_replay_suite(
                        Path(tmpdir),
                        rust_exporter=fake_hard_export,
                    )
            with (Path(tmpdir) / "results.csv").open(newline="") as handle:
                rows = list(csv.DictReader(handle))

        self.assertNotEqual(status, 0)
        self.assertEqual(rows[1]["decoder_impl"], "ldpc_bposd")
        self.assertEqual(rows[1]["status"], "skipped")
        self.assertIn("No module named 'ldpc'", rows[1]["error"])
```

If an earlier draft of this test still calls `run_hard_replay_suite` without
patching `_load_hard_replay_fixture`, remove that draft. The fake export uses a
tiny syndrome, so the test must pair it with `fake_hard_fixture()`.

- [ ] **Step 2: Run the failing Python runner tests**

Run:

```bash
python3 -m unittest benchmarks.bb_circuit_bposd_compare.tests.test_run_compare -v
```

Expected: FAIL because `HARD_REPLAY_CASES`, `run_hard_replay_suite`, and hard-replay row fields do not yet exist.

- [ ] **Step 3: Expand the CSV schema and hard case metadata**

In `cases.py`, extend `CSV_HEADER` after `osd_order`:

```python
    "basis",
    "syndrome_weight",
    "syndrome_support",
    "logical_prediction",
    "expected_logical",
```

Extend `CSV_HEADER` after `logical_error_rate`:

```python
    "bp_seconds",
    "osd_seconds",
    "decode_call_count",
    "bp_iteration_count",
    "osd_use_count",
    "osd_candidate_count",
    "gf2_solve_count",
    "gf2_full_elimination_count",
```

Add hard replay constants after `SMOKE_CASES`:

```python
HARD_REPLAY_CASES = (
    CompareCase(
        "bb90-p006-c10-seed12345-order7-hard-syndrome",
        "bb90",
        0.006,
        10,
        1,
        seed=12345,
        bp_method="ms",
        max_iter=10000,
        osd_method="osd_cs",
        osd_order=7,
    ),
)
```

Update `__init__.py` to import/export `HARD_REPLAY_CASES`.

- [ ] **Step 4: Add Rust command selection and CLI args**

In `run_compare.py`, import `HARD_REPLAY_CASES` from `cases.py`.

Change `_run_rust_export` to accept optional binary and method:

```python
def _run_rust_export(
    case: CompareCase,
    rust_binary: Path | None = None,
    osd_method: str | None = None,
) -> dict[str, Any]:
    if rust_binary is None:
        command = [
            "cargo",
            "run",
            "-q",
            "-p",
            "rsinter",
            "--bin",
            "rsinter",
            "--",
        ]
    else:
        command = [str(rust_binary)]
    command.extend(
        [
            "bb-circuit-bposd-memory",
            "--code-id",
            case.code_id,
            "--physical-error-rate",
            str(case.p),
            "--num-cycles",
            str(case.num_cycles),
            "--num-trials",
            str(case.num_trials),
            "--seed",
            str(case.seed),
            "--max-bp-iterations",
            str(case.max_iter),
            "--osd-order",
            str(case.osd_order),
        ]
    )
    if osd_method is not None:
        command.extend(["--osd-method", osd_method])
    command.append("--json-compare-case")
```

Thread `rust_binary` through `run_suite`, and call `exporter(case, rust_binary, case.osd_method)` when the default exporter is used. For injected test exporters, keep accepting one-argument callables by wrapping them in a small helper:

```python
def _call_exporter(
    exporter: Callable[..., dict[str, Any]],
    case: CompareCase,
    rust_binary: Path | None,
) -> dict[str, Any]:
    if exporter is _run_rust_export:
        return exporter(case, rust_binary=rust_binary, osd_method=case.osd_method)
    return exporter(case)
```

- [ ] **Step 5: Add hard-replay row helpers**

Add these constants near `REPO_ROOT`:

```python
HARD_REPLAY_FIXTURE_PATH = (
    REPO_ROOT / "rsinter" / "tests" / "fixtures" / "bb_circuit_bposd" / "bb90_hard_syndrome.json"
)
```

Add compact JSON list formatting:

```python
def _format_json_list(values: Sequence[Any]) -> str:
    return json.dumps(list(values), separators=(",", ":"))
```

Add a fixture loader:

```python
def _load_hard_replay_fixture() -> dict[str, Any]:
    return json.loads(HARD_REPLAY_FIXTURE_PATH.read_text())
```

Add a bundle helper:

```python
def _hard_replay_bundle(
    case: CompareCase,
    export: dict[str, Any],
    fixture: dict[str, Any],
) -> dict[str, Any]:
    if fixture["case_id"] != case.case_id:
        raise RuntimeError(f"hard replay fixture case_id mismatch: {fixture['case_id']}")
    basis = fixture["basis"]
    if basis != "Z":
        raise RuntimeError(f"unsupported hard replay basis: {basis}")
    trial = export["trials"][0]
    syndrome = list(trial["z_syndrome"])
    support = [index for index, bit in enumerate(syndrome) if bit]
    if support != list(fixture["syndrome_support"]):
        raise RuntimeError("hard replay syndrome support does not match fixture")
    expected_logical = list(trial["z_logical"])
    if expected_logical != list(fixture["expected_sampled_logical"]):
        raise RuntimeError("hard replay sampled logical does not match fixture")
    rust_prediction = trial.get("z_logical_prediction")
    if rust_prediction is None:
        raise RuntimeError("Rust hard replay export is missing z_logical_prediction")
    z_profile = trial.get("z_profile")
    if not isinstance(z_profile, dict):
        raise RuntimeError("Rust hard replay export is missing z_profile")
    return {
        "basis": basis,
        "model": export["z_model"],
        "syndrome": syndrome,
        "syndrome_support": support,
        "expected_logical": expected_logical,
        "rust_prediction": list(rust_prediction),
        "rust_profile": z_profile,
    }
```

Add a metadata updater:

```python
def _update_replay_metadata(
    row: dict[str, str],
    bundle: dict[str, Any],
    logical_prediction: Sequence[bool] | None,
) -> None:
    row.update(
        {
            "basis": bundle["basis"],
            "syndrome_weight": _format_value(len(bundle["syndrome_support"])),
            "syndrome_support": _format_json_list(bundle["syndrome_support"]),
            "logical_prediction": ""
            if logical_prediction is None
            else _format_json_list(logical_prediction),
            "expected_logical": _format_json_list(bundle["expected_logical"]),
        }
    )
```

Add Rust and Python hard row builders:

```python
def _rust_hard_replay_row(
    case: CompareCase,
    export: dict[str, Any],
    fixture: dict[str, Any],
) -> dict[str, str]:
    bundle = _hard_replay_bundle(case, export, fixture)
    row = _base_row(case, "rbposd")
    setup_seconds = float(export["rust_result"]["profile"]["setup_seconds"])
    decode_seconds = float(bundle["rust_profile"]["decode_seconds"])
    logical_prediction = bundle["rust_prediction"]
    _update_replay_metadata(row, bundle, logical_prediction)
    row.update(
        {
            "setup_seconds": _format_value(setup_seconds),
            "decode_seconds": _format_value(decode_seconds),
            "run_seconds": _format_value(setup_seconds + decode_seconds),
            "logical_error_rate": _format_value(
                0.0 if logical_prediction == bundle["expected_logical"] else 1.0
            ),
            "bp_seconds": _format_value(bundle["rust_profile"]["bp_seconds"]),
            "osd_seconds": _format_value(bundle["rust_profile"]["osd_seconds"]),
            "decode_call_count": _format_value(bundle["rust_profile"]["decode_call_count"]),
            "bp_iteration_count": _format_value(bundle["rust_profile"]["bp_iteration_count"]),
            "osd_use_count": _format_value(bundle["rust_profile"]["osd_use_count"]),
            "osd_candidate_count": _format_value(bundle["rust_profile"]["osd_candidate_count"]),
            "gf2_solve_count": _format_value(bundle["rust_profile"]["gf2_solve_count"]),
            "gf2_full_elimination_count": _format_value(
                bundle["rust_profile"]["gf2_full_elimination_count"]
            ),
            "status": "ok",
        }
    )
    return row
```

```python
def _python_hard_replay_row(
    case: CompareCase,
    export: dict[str, Any],
    fixture: dict[str, Any],
) -> dict[str, str]:
    import numpy as np
    from ldpc import BpOsdDecoder

    bundle = _hard_replay_bundle(case, export, fixture)
    setup_started = time.perf_counter()
    decoder = BpOsdDecoder(
        _dense_matrix(bundle["model"], np),
        error_channel=bundle["model"]["channel_probs"],
        max_iter=PYTHON_UPSTREAM_MAX_ITER,
        bp_method=PYTHON_UPSTREAM_BP_METHOD,
        osd_method=PYTHON_UPSTREAM_OSD_METHOD,
        osd_order=PYTHON_UPSTREAM_OSD_ORDER,
        input_vector_type="syndrome",
    )
    setup_seconds = time.perf_counter() - setup_started

    decode_started = time.perf_counter()
    correction = decoder.decode(np.asarray(bundle["syndrome"], dtype=np.uint8))
    logical_prediction = _predicted_logicals(
        correction,
        bundle["model"],
        len(bundle["expected_logical"]),
    )
    decode_seconds = time.perf_counter() - decode_started

    row = _base_row(case, "ldpc_bposd")
    row.update(_python_upstream_settings())
    _update_replay_metadata(row, bundle, logical_prediction)
    row.update(
        {
            "setup_seconds": _format_value(setup_seconds),
            "decode_seconds": _format_value(decode_seconds),
            "run_seconds": _format_value(setup_seconds + decode_seconds),
            "logical_error_rate": _format_value(
                0.0 if logical_prediction == bundle["expected_logical"] else 1.0
            ),
            "status": "ok",
        }
    )
    return row
```

- [ ] **Step 6: Add the hard-replay suite and CLI tier**

Add:

```python
def run_hard_replay_suite(
    output_dir: Path,
    allow_missing_python: bool = False,
    rust_binary: Path | None = None,
    rust_exporter: Callable[..., dict[str, Any]] | None = None,
) -> int:
    exporter = rust_exporter or _run_rust_export
    output_dir.mkdir(parents=True, exist_ok=True)
    fixture = _load_hard_replay_fixture()
    rows: list[dict[str, str]] = []
    saw_rust_error = False
    saw_skipped_python = False

    for case in HARD_REPLAY_CASES:
        try:
            export = _call_exporter(exporter, case, rust_binary)
            rows.append(_rust_hard_replay_row(case, export, fixture))
        except Exception as error:
            saw_rust_error = True
            rows.append(_rust_error_row(case, error))
            continue

        try:
            rows.append(_python_hard_replay_row(case, export, fixture))
        except ImportError as error:
            if not _is_missing_python_dependency(error):
                raise
            saw_skipped_python = True
            skipped = _skipped_python_row(case, error)
            try:
                _update_replay_metadata(skipped, _hard_replay_bundle(case, export, fixture), None)
            except Exception:
                pass
            rows.append(skipped)

    _write_rows(rows, output_dir / "results.csv")
    write_summary(rows, output_dir / "summary.md")
    if saw_rust_error:
        return 1
    if saw_skipped_python and not allow_missing_python:
        return 1
    return 0
```

Change CLI parser choices:

```python
    parser.add_argument(
        "--tier",
        choices=("smoke", "small_ldpc_catalog", "hard-replay"),
        required=True,
    )
    parser.add_argument("--rust-binary", type=Path)
```

Dispatch:

```python
    if args.tier == "hard-replay":
        status = run_hard_replay_suite(
            output_dir=args.output_dir,
            allow_missing_python=args.allow_missing_python,
            rust_binary=args.rust_binary,
        )
        if status != 0 and not args.allow_missing_python:
            for message in _missing_python_dependency_messages(
                _read_rows(args.output_dir / "results.csv")
            ):
                print(message, file=sys.stderr)
        return status
```

- [ ] **Step 7: Run the Python runner tests**

Run:

```bash
python3 -m unittest benchmarks.bb_circuit_bposd_compare.tests.test_run_compare -v
```

Expected: PASS.

- [ ] **Step 8: Commit Task 2**

Run:

```bash
git add benchmarks/bb_circuit_bposd_compare/__init__.py \
  benchmarks/bb_circuit_bposd_compare/cases.py \
  benchmarks/bb_circuit_bposd_compare/run_compare.py \
  benchmarks/bb_circuit_bposd_compare/tests/test_run_compare.py
git commit -m "feat: add BB90 hard replay runner"
```

### Task 3: Replay Verifier

**Files:**
- Create: `benchmarks/bb_circuit_bposd_compare/verify_replay.py`
- Create: `benchmarks/bb_circuit_bposd_compare/tests/test_verify_replay.py`

**Interfaces:**
- Consumes: expanded `CSV_HEADER`, `HARD_REPLAY_CASES`.
- Produces:
  - `verify_rows(rows: list[dict[str, str]], allow_missing_python: bool = False) -> list[str]`
  - CLI `python3 -m benchmarks.bb_circuit_bposd_compare.verify_replay [--allow-missing-python] <csv_path>`

- [ ] **Step 1: Write failing verifier tests**

Create `benchmarks/bb_circuit_bposd_compare/tests/test_verify_replay.py`:

```python
import unittest

from benchmarks.bb_circuit_bposd_compare.verify_replay import verify_rows


CASE_ID = "bb90-p006-c10-seed12345-order7-hard-syndrome"
PREDICTION = "[false,true,false,true,false,false,false,true]"
SUPPORT = "[5,8,14]"


def make_row(decoder_impl: str, **overrides: str) -> dict[str, str]:
    row = {
        "case_id": CASE_ID,
        "runner": "compare",
        "decoder_impl": decoder_impl,
        "code_id": "bb90",
        "p": "0.006",
        "num_cycles": "10",
        "num_trials": "1",
        "seed": "12345",
        "bp_method": "ms",
        "max_iter": "10000",
        "osd_method": "osd_cs",
        "osd_order": "7",
        "basis": "Z",
        "syndrome_weight": "3",
        "syndrome_support": SUPPORT,
        "logical_prediction": PREDICTION,
        "expected_logical": PREDICTION,
        "setup_seconds": "0.1",
        "decode_seconds": "0.2",
        "run_seconds": "0.3",
        "logical_error_rate": "0.0",
        "bp_seconds": "0.1" if decoder_impl == "rbposd" else "",
        "osd_seconds": "0.1" if decoder_impl == "rbposd" else "",
        "decode_call_count": "1" if decoder_impl == "rbposd" else "",
        "bp_iteration_count": "10000" if decoder_impl == "rbposd" else "",
        "osd_use_count": "1" if decoder_impl == "rbposd" else "",
        "osd_candidate_count": "4100" if decoder_impl == "rbposd" else "",
        "gf2_solve_count": "4101" if decoder_impl == "rbposd" else "",
        "gf2_full_elimination_count": "1" if decoder_impl == "rbposd" else "",
        "status": "ok",
        "error": "",
    }
    row.update(overrides)
    return row


class VerifyReplayTest(unittest.TestCase):
    def test_verify_rows_accepts_paired_hard_replay(self) -> None:
        self.assertEqual(verify_rows([make_row("rbposd"), make_row("ldpc_bposd")]), [])

    def test_verify_rows_rejects_unpaired_syndrome_metadata(self) -> None:
        errors = verify_rows(
            [
                make_row("rbposd"),
                make_row("ldpc_bposd", syndrome_support="[5,8,15]"),
            ]
        )
        self.assertIn("Rust/Python replay is no longer paired", "\n".join(errors))

    def test_verify_rows_rejects_logical_prediction_mismatch(self) -> None:
        errors = verify_rows(
            [
                make_row("rbposd"),
                make_row("ldpc_bposd", logical_prediction="[true,true,false,true,false,false,false,true]"),
            ]
        )
        self.assertIn("Rust/Python logical predictions do not match", "\n".join(errors))

    def test_verify_rows_rejects_skipped_python_without_allow_missing(self) -> None:
        errors = verify_rows(
            [
                make_row("rbposd"),
                make_row(
                    "ldpc_bposd",
                    status="skipped",
                    setup_seconds="",
                    decode_seconds="",
                    run_seconds="",
                    logical_prediction="",
                    error="python dependency unavailable for ldpc_bposd replay: No module named 'ldpc'",
                ),
            ]
        )
        self.assertIn("Python ldpc_bposd replay row is skipped", "\n".join(errors))

    def test_verify_rows_allows_skipped_python_with_allow_missing(self) -> None:
        rows = [
            make_row("rbposd"),
            make_row(
                "ldpc_bposd",
                status="skipped",
                setup_seconds="",
                decode_seconds="",
                run_seconds="",
                logical_prediction="",
                error="python dependency unavailable for ldpc_bposd replay: No module named 'ldpc'",
            ),
        ]
        self.assertEqual(verify_rows(rows, allow_missing_python=True), [])

    def test_verify_rows_rejects_missing_rust_counters(self) -> None:
        errors = verify_rows([make_row("rbposd", gf2_solve_count=""), make_row("ldpc_bposd")])
        self.assertIn("Rust rbposd replay row is missing OSD/GF(2) counter fields", "\n".join(errors))


if __name__ == "__main__":
    unittest.main()
```

- [ ] **Step 2: Run the failing verifier tests**

Run:

```bash
python3 -m unittest benchmarks.bb_circuit_bposd_compare.tests.test_verify_replay -v
```

Expected: FAIL because `verify_replay.py` does not exist.

- [ ] **Step 3: Implement `verify_replay.py`**

Create `benchmarks/bb_circuit_bposd_compare/verify_replay.py`:

```python
from __future__ import annotations

import argparse
import csv
import json
import sys
from pathlib import Path

from benchmarks.bb_circuit_bposd_compare.cases import CSV_HEADER, HARD_REPLAY_CASES

REQUIRED_OK_FIELDS = (
    "basis",
    "syndrome_weight",
    "syndrome_support",
    "logical_prediction",
    "expected_logical",
    "setup_seconds",
    "decode_seconds",
    "run_seconds",
    "logical_error_rate",
    "status",
)
PINNED_REPLAY_SETTINGS = {
    "bp_method": "ms",
    "max_iter": "10000",
    "osd_order": "7",
    "seed": "12345",
    "basis": "Z",
}
ACCEPTED_OSD_METHODS = {"osd_cs", "ldpc_cs", "ldpc_osd_cs"}
RUST_COUNTER_FIELDS = (
    "bp_seconds",
    "osd_seconds",
    "decode_call_count",
    "bp_iteration_count",
    "osd_use_count",
    "osd_candidate_count",
    "gf2_solve_count",
    "gf2_full_elimination_count",
)


def verify_rows(
    rows: list[dict[str, str]],
    allow_missing_python: bool = False,
) -> list[str]:
    errors: list[str] = []
    if not rows:
        return ["CSV has no data rows"]

    missing_columns = [column for column in CSV_HEADER if not all(column in row for row in rows)]
    if missing_columns:
        errors.append("row is missing required CSV column(s): " + ", ".join(missing_columns))

    case_id = HARD_REPLAY_CASES[0].case_id
    case_rows = [row for row in rows if row.get("case_id") == case_id]
    rust_rows = [row for row in case_rows if row.get("decoder_impl") == "rbposd"]
    python_rows = [row for row in case_rows if row.get("decoder_impl") == "ldpc_bposd"]
    if len(rust_rows) != 1:
        errors.append("expected exactly one Rust rbposd hard replay row")
    if len(python_rows) != 1:
        errors.append("expected exactly one Python ldpc_bposd hard replay row")
    if len(rust_rows) != 1 or len(python_rows) != 1:
        return errors

    rust = rust_rows[0]
    python = python_rows[0]
    for row in (rust, python):
        for field, expected_value in PINNED_REPLAY_SETTINGS.items():
            if row.get(field) != expected_value:
                errors.append(
                    f"hard replay row has mismatched {field}: expected {expected_value}, got {row.get(field, '')}"
                )
        if row.get("osd_method") not in ACCEPTED_OSD_METHODS:
            errors.append(
                "hard replay row has mismatched osd_method: expected osd_cs/ldpc_cs equivalent"
            )

    python_status = python.get("status")
    if python_status == "skipped":
        if not python.get("error"):
            errors.append("Python ldpc_bposd replay row is skipped without an explicit error")
        if not allow_missing_python:
            errors.append("Python ldpc_bposd replay row is skipped")
        return errors

    ok_rows = [rust, python]
    for row in ok_rows:
        if row.get("status") != "ok":
            errors.append("hard replay row is not completed: " + row.get("decoder_impl", ""))
            continue
        if any(not row.get(field) for field in REQUIRED_OK_FIELDS):
            errors.append("completed hard replay row missing required timing/logical/status field")
            break

    pair_fields = ("case_id", "basis", "syndrome_weight", "syndrome_support", "expected_logical")
    if any(rust.get(field) != python.get(field) for field in pair_fields):
        errors.append("Rust/Python replay is no longer paired")

    if _json_list(rust.get("logical_prediction", "")) != _json_list(
        python.get("logical_prediction", "")
    ):
        errors.append("Rust/Python logical predictions do not match")

    if any(not rust.get(field) for field in RUST_COUNTER_FIELDS):
        errors.append("Rust rbposd replay row is missing OSD/GF(2) counter fields")
    else:
        for field in RUST_COUNTER_FIELDS:
            _require_nonnegative_number(rust, field, errors)
        for field in (
            "decode_call_count",
            "bp_iteration_count",
            "osd_use_count",
            "osd_candidate_count",
            "gf2_solve_count",
            "gf2_full_elimination_count",
        ):
            _require_integer(rust, field, errors)
        if _as_int(rust, "osd_use_count") <= 0:
            errors.append("Rust rbposd replay row did not record OSD use")
        if _as_int(rust, "osd_candidate_count") <= 0:
            errors.append("Rust rbposd replay row did not record OSD candidates")
        if _as_int(rust, "gf2_solve_count") <= 0:
            errors.append("Rust rbposd replay row did not record GF(2) solves")

    return errors


def _json_list(value: str) -> list[object]:
    try:
        parsed = json.loads(value)
    except json.JSONDecodeError:
        return ["<invalid-json-list>", value]
    return parsed if isinstance(parsed, list) else ["<not-a-list>", parsed]


def _as_int(row: dict[str, str], field: str) -> int:
    try:
        return int(row.get(field, "0"))
    except ValueError:
        return -1


def _require_nonnegative_number(row: dict[str, str], field: str, errors: list[str]) -> None:
    try:
        value = float(row[field])
    except ValueError:
        errors.append(f"Rust rbposd replay counter/timing field is not numeric: {field}")
        return
    if value < 0.0:
        errors.append(f"Rust rbposd replay counter/timing field is negative: {field}")


def _require_integer(row: dict[str, str], field: str, errors: list[str]) -> None:
    try:
        value = float(row[field])
    except ValueError:
        return
    if value.is_integer():
        return
    errors.append(f"Rust rbposd replay counter field is not an integer: {field}")


def _load_rows(csv_path: Path) -> list[dict[str, str]]:
    with csv_path.open(newline="") as handle:
        return list(csv.DictReader(handle))


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--allow-missing-python", action="store_true")
    parser.add_argument("csv_path", type=Path)
    args = parser.parse_args(argv)

    errors = verify_rows(_load_rows(args.csv_path), allow_missing_python=args.allow_missing_python)
    if errors:
        for error in errors:
            print(error, file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
```

- [ ] **Step 4: Run verifier tests**

Run:

```bash
python3 -m unittest benchmarks.bb_circuit_bposd_compare.tests.test_verify_replay -v
```

Expected: PASS.

- [ ] **Step 5: Commit Task 3**

Run:

```bash
git add benchmarks/bb_circuit_bposd_compare/verify_replay.py \
  benchmarks/bb_circuit_bposd_compare/tests/test_verify_replay.py
git commit -m "feat: verify BB90 hard replay rows"
```

### Task 4: Docs, Integration Checks, And Cleanup

**Files:**
- Modify: `benchmarks/bb_circuit_bposd_compare/README.md`
- Modify touched files only if integration tests expose small fixes.

**Interfaces:**
- Consumes: completed Tasks 1-3.
- Produces: documented hard replay command and a verified branch.

- [ ] **Step 1: Document the hard replay command**

Append this section to `benchmarks/bb_circuit_bposd_compare/README.md`:

````markdown
## BB90 Hard-Syndrome Replay

After building `rsinter`, replay the checked-in BB90 hard-syndrome fixture with:

```bash
python3 -m benchmarks.bb_circuit_bposd_compare.run_compare \
  --tier hard-replay \
  --output-dir /tmp/rstim-bb90-hard-replay \
  --rust-binary target/release/rsinter

python3 -m benchmarks.bb_circuit_bposd_compare.verify_replay \
  /tmp/rstim-bb90-hard-replay/results.csv
```

The replay writes one Rust `rbposd` row and one Python `ldpc_bposd` row for
`bb90-p006-c10-seed12345-order7-hard-syndrome`. Both rows use
`bp_method=ms`, `max_iter=10000`, `osd_method=osd_cs`, and `osd_order=7`.
The verifier checks that the rows are paired on the fixture basis/syndrome and
that Rust and Python logical predictions match. Rust rows also carry the OSD
and GF(2) counters from the replay decode.

Missing Python decoder dependencies remain explicit: `run_compare` records a
skipped Python row and exits nonzero unless `--allow-missing-python` is passed.
`verify_replay` also rejects skipped Python rows unless its own
`--allow-missing-python` diagnostic flag is used.
````

- [ ] **Step 2: Run focused Python tests**

Run:

```bash
python3 -m unittest benchmarks.bb_circuit_bposd_compare.tests.test_run_compare \
  benchmarks.bb_circuit_bposd_compare.tests.test_verify_replay \
  benchmarks.bb_circuit_bposd_compare.tests.test_verify_smoke -v
```

Expected: PASS.

- [ ] **Step 3: Run focused Rust tests**

Run:

```bash
cargo test -p rsinter rsinter_json_compare_case_accepts_ldpc_osd_method_and_exports_trial_predictions -- --nocapture
cargo test -p rsinter bb90_hard_syndrome_reports_osd_profile_counters -- --nocapture
```

Expected: PASS.

- [ ] **Step 4: Build release binary for issue verification**

Run:

```bash
cargo build --release -p rsinter --bin rsinter
```

Expected: PASS and `target/release/rsinter` exists.

- [ ] **Step 5: Run issue verification command**

Run:

```bash
.venv-surface-decoder/bin/python -m benchmarks.bb_circuit_bposd_compare.run_compare \
  --tier hard-replay \
  --output-dir /tmp/rstim-bb90-hard-replay \
  --rust-binary target/release/rsinter
```

Expected: PASS if Python `ldpc` dependencies are installed. If Python dependencies are missing, rerun with `--allow-missing-python` only to inspect artifacts, and record that the required verifier is expected to reject skipped Python rows.

- [ ] **Step 6: Run issue replay verifier**

Run:

```bash
.venv-surface-decoder/bin/python -m benchmarks.bb_circuit_bposd_compare.verify_replay \
  /tmp/rstim-bb90-hard-replay/results.csv
```

Expected: PASS when the previous command produced completed Rust and Python rows.

- [ ] **Step 7: Run the required workspace verification**

Run:

```bash
cargo test
```

Expected: PASS.

- [ ] **Step 8: Check formatting and diff hygiene**

Run:

```bash
rustfmt --edition 2024 rsinter/src/bin/rsinter.rs rsinter/src/bb_circuit_memory.rs --check
python3 -m unittest discover benchmarks/bb_circuit_bposd_compare/tests -v
git diff --check
git status --short
```

Expected: formatting passes, Python tests pass, no whitespace errors, and only intended files are modified.

- [ ] **Step 9: Commit Task 4**

Run:

```bash
git add benchmarks/bb_circuit_bposd_compare/README.md
git commit -m "docs: document BB90 hard replay"
```
