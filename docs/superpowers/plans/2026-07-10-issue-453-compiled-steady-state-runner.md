# Compiled Steady-State Runner Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a fair compiled steady-state benchmark runner with long-lived Stim and rstim workers that compile once and serve two warmup plus seven measured `sample(1024)` requests.

**Architecture:** The Python runner validates the #449 manifest, owns lifecycle/timing/provenance, and speaks a compact binary frame protocol to workers over stdin/stdout. The Stim worker and Rust worker each own variant-specific compile-once state and return raw shot-major `b8` bytes per `SAMPLE` request. Tests use fake workers for protocol, timing-window, and failure negative controls before the canonical worker path is exercised.

**Tech Stack:** Python 3 standard library, `stim==1.15.0`, Rust 2024, `rstim::CompiledMeasurementSampler`, `serde_json`, `rand::rngs::StdRng`, `unittest`.

## Global Constraints

- Runner CLI: `--manifest <toml> --case <id> --profile release --warmup-rounds 2 --measure-rounds 7 --seed 0 --out-dir <dir>`.
- Canonical Stim worker argv: `python3 -m benchmarks.rstim_vs_stim_simulator.workers.stim_compiled_steady --input <fixture> --seed 0`.
- Canonical rstim worker argv: `target/release/rstim_compiled_steady_worker --input <fixture> --seed 0`.
- Require `stim==1.15.0`.
- Protocol frames are one-byte type, unsigned 64-bit little-endian payload length, then exact payload bytes.
- Define `READY`, `SAMPLE`, `RESULT`, `STOP`, `FINAL`, and `ERROR`.
- `READY` and `FINAL` carry variant, compile count, reference-build count, sample-call count, fixture SHA, measurement count, and bytes/shot.
- `SAMPLE` carries request ID and shot count.
- `RESULT` carries request ID, cumulative sample-call count, and raw shot-major `b8`.
- `raw.jsonl` contains one ready, nine sample, and one final record per variant.
- Timing starts immediately before writing the complete `SAMPLE` frame and ends only after reading the complete `RESULT` frame.
- Each canonical response is 1,552,384 data bytes.
- Summary uses only seven measured records per variant.
- Record #450 provenance fields plus exact worker argv, Python executable and hash, loaded Stim extension and hash, rstim worker binary and hash, protocol version, and `seed_policy="seed_once_then_advance_across_9_calls"`.
- Known-answer preflight runs both workers on `X 0\nM 0\n`; a one-shot result must be `0x01`.
- Do not publish checked artifacts or require a speed ratio.

---

### Task 1: Runner Protocol and Fake-Worker Contract Tests

**Files:**
- Create: `benchmarks/rstim_vs_stim_simulator/run_compiled_steady.py`
- Create: `benchmarks/rstim_vs_stim_simulator/tests/test_run_compiled_steady.py`

**Interfaces:**
- Produces: frame constants `READY`, `SAMPLE`, `RESULT`, `STOP`, `FINAL`, `ERROR`, `PROTOCOL_VERSION`.
- Produces: `write_frame(stream, frame_type: bytes, payload: bytes) -> None`.
- Produces: `read_frame(stream) -> tuple[bytes, bytes]`.
- Produces: `main(argv: list[str] | None = None) -> int`.

- [ ] **Step 1: Write failing protocol lifecycle tests**

Add a test file that creates temporary fake worker scripts. The primary fake worker imports `benchmarks.rstim_vs_stim_simulator.run_compiled_steady`, sends valid `READY`, returns `0x01` for one-shot preflight fixtures, returns `expected_output_bytes` bytes for canonical fixtures, sends valid `FINAL`, and exits zero.

```python
def test_fake_workers_emit_required_lifecycle_and_summary(self) -> None:
    with tempfile.TemporaryDirectory() as temp_dir:
        paths = self._write_fake_workers(Path(temp_dir), mode="ok")
        out_dir = Path(temp_dir) / "out"
        result = self._run_runner(
            out_dir,
            stim_worker=[sys.executable, str(paths["stim"])],
            rstim_worker=[sys.executable, str(paths["rstim"])],
        )

    self.assertEqual(result.returncode, 0, result.stderr)
    self.assertIn(
        "PASS compiled steady-state lifecycle variants=2 compile=1 reference=1 calls=9 measured=14",
        result.stdout,
    )
    raw = [json.loads(line) for line in (out_dir / "raw.jsonl").read_text().splitlines()]
    self.assertEqual(sum(1 for record in raw if record["record_type"] == "ready"), 2)
    self.assertEqual(sum(1 for record in raw if record["record_type"] == "sample"), 18)
    self.assertEqual(sum(1 for record in raw if record["record_type"] == "final"), 2)
    summary = json.loads((out_dir / "summary.json").read_text())
    self.assertEqual(summary["measured_records"], 14)
    self.assertEqual({variant["sample_count"] for variant in summary["variants"]}, {7})
```

- [ ] **Step 2: Write failing negative-control tests**

Add tests for delayed final result bytes, nonzero exit after valid `FINAL`, and known-answer `0x00`.

```python
def test_sample_timing_includes_delayed_final_result_byte(self) -> None:
    result, out_dir = self._run_fake_mode("delay-last-byte")
    self.assertEqual(result.returncode, 0, result.stderr)
    raw = [json.loads(line) for line in (out_dir / "raw.jsonl").read_text().splitlines()]
    elapsed = [record["elapsed_ns"] for record in raw if record["record_type"] == "sample"]
    self.assertGreaterEqual(max(elapsed), 140_000_000)

def test_nonzero_exit_after_final_rejects_before_summary(self) -> None:
    result, out_dir = self._run_fake_mode("final-then-nonzero")
    self.assertNotEqual(result.returncode, 0)
    self.assertFalse((out_dir / "summary.json").exists())

def test_known_answer_zero_fails_before_canonical_timing(self) -> None:
    result, out_dir = self._run_fake_mode("bad-known-answer")
    self.assertNotEqual(result.returncode, 0)
    self.assertFalse((out_dir / "raw.jsonl").exists())
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_run_compiled_steady -q`

Expected: failure because `run_compiled_steady.py` and protocol helpers do not exist yet.

- [ ] **Step 4: Implement protocol helpers and fake-worker runner path**

Implement frame helpers, manifest loading via `fair_cli_contract`, worker session management, raw record writing, measured-only summary generation, environment writing, and test-only worker override args `--stim-worker-command` and `--rstim-worker-command`. The override args are hidden from the public issue interface but keep tests deterministic.

- [ ] **Step 5: Run tests to verify they pass**

Run: `python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_run_compiled_steady -q`

Expected: PASS for fake-worker lifecycle and negative controls.

- [ ] **Step 6: Commit**

```bash
git add benchmarks/rstim_vs_stim_simulator/run_compiled_steady.py benchmarks/rstim_vs_stim_simulator/tests/test_run_compiled_steady.py
git commit -m "test: cover compiled steady-state runner protocol"
```

### Task 2: Stim Compiled Steady-State Worker

**Files:**
- Create: `benchmarks/rstim_vs_stim_simulator/workers/__init__.py`
- Create: `benchmarks/rstim_vs_stim_simulator/workers/stim_compiled_steady.py`
- Modify: `benchmarks/rstim_vs_stim_simulator/tests/test_run_compiled_steady.py`

**Interfaces:**
- Consumes: `read_frame`, `write_frame`, frame constants, and telemetry schema from `run_compiled_steady.py`.
- Produces: module entrypoint `python3 -m benchmarks.rstim_vs_stim_simulator.workers.stim_compiled_steady --input <fixture> --seed <seed>`.

- [ ] **Step 1: Write worker-specific tests**

Add tests that run the Stim worker when `stim==1.15.0` is importable. The known-answer test writes `X 0\nM 0\n` to a temporary fixture, reads `READY`, sends one `SAMPLE` with one shot, and asserts the `RESULT` data byte is `0x01`.

- [ ] **Step 2: Run worker tests to verify they fail**

Run: `python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_run_compiled_steady -q`

Expected: failure because the Stim worker module does not exist.

- [ ] **Step 3: Implement Stim worker**

Implement the worker to import `stim`, require `stim.__version__ == "1.15.0"`, compute fixture SHA-256, instantiate `stim.Circuit(input_text)`, call `compile_sampler(seed=args.seed)` exactly once, and respond to `SAMPLE` frames with `sampler.sample(shots=shots, bit_packed=True).tobytes(order="C")`.

- [ ] **Step 4: Run worker tests**

Run: `python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_run_compiled_steady -q`

Expected: PASS or SKIP for the Stim worker-specific test if the exact Stim version is unavailable; fake-worker contract tests still pass.

- [ ] **Step 5: Commit**

```bash
git add benchmarks/rstim_vs_stim_simulator/workers benchmarks/rstim_vs_stim_simulator/tests/test_run_compiled_steady.py
git commit -m "feat: add compiled steady-state Stim worker"
```

### Task 3: rstim Compiled Steady-State Worker

**Files:**
- Create: `rstim/src/bin/rstim_compiled_steady_worker.rs`
- Modify: `benchmarks/rstim_vs_stim_simulator/run_compiled_steady.py`
- Modify: `benchmarks/rstim_vs_stim_simulator/tests/test_run_compiled_steady.py`

**Interfaces:**
- Consumes: `rstim::parser::parse_lines`, `rstim::CompiledMeasurementSampler`, `rstim::data_path::ReferenceSampleMode`, `rstim::sampler::SampleOutputMode`, and `rstim::output::write_shots_b8`.
- Produces: binary `target/{profile}/rstim_compiled_steady_worker`.

- [ ] **Step 1: Write Rust-worker integration test**

Add a test that builds `cargo build -p rstim --bin rstim_compiled_steady_worker`, runs the worker on a temporary `X 0\nM 0\n` fixture, sends a one-shot `SAMPLE`, and asserts the returned data is `b"\x01"` and telemetry reports `compile_count == 1`, `reference_build_count == 1`, `sample_call_count == 1`.

- [ ] **Step 2: Run test to verify it fails**

Run: `python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_run_compiled_steady -q`

Expected: failure because the Rust worker binary does not exist.

- [ ] **Step 3: Implement Rust worker**

Implement argument parsing for `--input` and `--seed`, binary frame read/write helpers, JSON telemetry structs, JSON `SAMPLE` parsing, `RESULT` payload serialization as request ID plus cumulative sample-call count plus `write_shots_b8` bytes, `ERROR` frame emission on recoverable errors, and `FINAL` after `STOP`.

- [ ] **Step 4: Update runner build path**

Update `run_compiled_steady.py` to build `cargo build --release -p rstim --bin rstim_compiled_steady_worker` for `--profile release` and `cargo build -p rstim --bin rstim_compiled_steady_worker` for `--profile debug`, then use the built binary path in canonical argv and provenance.

- [ ] **Step 5: Run Rust worker test**

Run: `python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_run_compiled_steady -q`

Expected: PASS for fake-worker tests and Rust worker known-answer test.

- [ ] **Step 6: Commit**

```bash
git add rstim/src/bin/rstim_compiled_steady_worker.rs benchmarks/rstim_vs_stim_simulator/run_compiled_steady.py benchmarks/rstim_vs_stim_simulator/tests/test_run_compiled_steady.py
git commit -m "feat: add compiled steady-state rstim worker"
```

### Task 4: Canonical Runner Verification and Provenance Tightening

**Files:**
- Modify: `benchmarks/rstim_vs_stim_simulator/run_compiled_steady.py`
- Modify: `benchmarks/rstim_vs_stim_simulator/tests/test_run_compiled_steady.py`

**Interfaces:**
- Consumes: #449 `fair_cli_cases.toml` case fields.
- Produces: `raw.jsonl`, `summary.json`, `report.md`, `environment.json`, and acceptance print.

- [ ] **Step 1: Add assertions for canonical provenance fields**

Extend tests to assert `environment.json` contains `git_commit`, `os`, `cpu_model`, `profile`, `timer_scope`, `seed_policy`, `stim_version`, `rstim_version`, `rustc_version`, fair/source manifest paths and SHA-256 values, fixture path and SHA-256, worker argv, Python executable/path hash, Stim extension path/hash fields, rstim worker binary/hash fields, protocol version, warmup/measure rounds, and known-answer preflight results.

- [ ] **Step 2: Run tests to identify missing fields**

Run: `python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_run_compiled_steady -q`

Expected: failure naming any missing provenance fields.

- [ ] **Step 3: Fill provenance and report rendering**

Implement missing provenance collection with safe fallbacks for unavailable CPU model or Stim extension path. Render `report.md` from `summary.json` with one row per variant and measured median elapsed nanoseconds.

- [ ] **Step 4: Run canonical issue command**

Run:

```bash
rm -rf /tmp/rstim-compiled-steady
python3 -m benchmarks.rstim_vs_stim_simulator.run_compiled_steady --manifest benchmarks/rstim_vs_stim_simulator/fair_cli_cases.toml --case stim_surface_d11_r100 --profile release --warmup-rounds 2 --measure-rounds 7 --seed 0 --out-dir /tmp/rstim-compiled-steady
python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_run_compiled_steady -q
```

Expected: runner stdout contains `PASS compiled steady-state lifecycle variants=2 compile=1 reference=1 calls=9 measured=14`; unit tests pass.

- [ ] **Step 5: Run full required verification**

Run: `cargo test`

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add benchmarks/rstim_vs_stim_simulator/run_compiled_steady.py benchmarks/rstim_vs_stim_simulator/tests/test_run_compiled_steady.py
git commit -m "feat: add compiled steady-state benchmark runner"
```

## Self-Review

- Spec coverage: tasks cover the runner CLI, both canonical worker argv forms, binary frames, lifecycle records, summary scoping, #450-style provenance, known-answer preflight, negative controls, canonical issue command, and `cargo test`.
- Placeholder scan: no placeholder task remains; every step has concrete files, functions, commands, and expected outcomes.
- Type consistency: frame helper signatures are defined in Task 1 and consumed unchanged by later Python worker/tests; Rust worker payloads match the same frame schema.
