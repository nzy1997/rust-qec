# Issue 450 Fair CLI Runner Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the symmetric bit-packed CLI sampling runner for `stim_surface_d11_r100`.

**Architecture:** Implement one Python runner that imports the existing #449 fair CLI contract, validates the manifest before process execution, builds `rstim` once, resolves both binaries, performs a known-answer preflight, then times full subprocess completion for each warmup and measured round. Keep tests in one unittest module using fake `stim` and `rstim` executables so process timing, provenance, artifact shape, and failure behavior are verified without relying on local benchmark runtime.

**Tech Stack:** Python standard library `argparse`, `hashlib`, `json`, `os`, `platform`, `re`, `shutil`, `statistics`, `subprocess`, `sys`, `tempfile`, `time`, `unittest`; existing `benchmarks.rstim_vs_stim_simulator.fair_cli_contract` and `run_speed_case`.

## Global Constraints

- CLI path is exactly `benchmarks/rstim_vs_stim_simulator/run_fair_cli.py`.
- Required CLI flags are `--manifest <toml> --case <id> --profile release --warmup-rounds 2 --measure-rounds 7 --out-dir <dir>`.
- The runner must build `rstim` once before timed samples and must not include build time in benchmark samples.
- The runner must validate the fair manifest before benchmark process execution.
- Before timing, both CLI templates must run on a temporary `X 0\nM 0\n` circuit with one shot and stdout exactly `0x01`.
- A known-answer preflight failure must occur before `raw.jsonl` is created.
- Stim version must be `1.15.0`.
- Each elapsed time must cover process spawn, complete stdout drain, stderr drain, and successful process exit.
- Each successful benchmark run must exit `0` and produce `1552384` bytes.
- `raw.jsonl` must contain exactly 18 records: two warmups and seven measured records for each of two variants.
- Every raw record must contain `case_id`, `variant`, `phase`, `round_index`, `seed`, `argv`, `shots`, `measurement_count`, `output_format`, `timer_scope`, `elapsed_ns`, `actual_output_bytes`, `stdout_sha256`, and `exit_code`.
- Seeds are `0` through `8` in execution order for each variant.
- `summary.json` must derive only from the 14 measured records.
- `report.md` must render from the derived summary.
- `environment.json` must record git commit; OS; CPU model; profile; timer scope; seed policy; Stim version; `rstim` version; Rust version; fair manifest path and SHA-256; source manifest path and SHA-256; fixture path and SHA-256; resolved Stim and `rstim` binary paths and SHA-256; exact expanded argv for all rounds; warmup and measure round counts; and known-answer preflight result.
- Do not cache samplers between CLI invocations.
- Do not publish checked timing evidence.
- Do not set a speed-ratio gate.
- Required focused verification is `python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_run_fair_cli -q`.
- Required issue verification includes the canonical runner command and `cargo test`.

---

## File Structure

- Create `benchmarks/rstim_vs_stim_simulator/run_fair_cli.py`: runner CLI, manifest validation glue, binary/version provenance, subprocess timing, artifact writers.
- Create `benchmarks/rstim_vs_stim_simulator/tests/test_run_fair_cli.py`: fake CLI helpers, success workflow tests, timing negative control, preflight negative control.

### Task 1: Test The Runner Contract

**Files:**
- Create: `benchmarks/rstim_vs_stim_simulator/tests/test_run_fair_cli.py`
- Create later: `benchmarks/rstim_vs_stim_simulator/run_fair_cli.py`

**Interfaces:**
- Consumes after Task 2: `run_fair_cli.main(argv: list[str] | None = None) -> int`.
- Consumes after Task 2: `run_fair_cli.run_fair_cli(args: argparse.Namespace, repo_root: Path = REPO_ROOT, command_line: list[str] | None = None) -> None`.
- Produces test fixtures that fake `stim` on `PATH` and fake the built `rstim` binary returned by `build_rstim`.

- [ ] **Step 1: Write the failing success-path tests**

Create `benchmarks/rstim_vs_stim_simulator/tests/test_run_fair_cli.py` with helpers that write executable Python fake CLIs. The success fake should emit `b"\x01"` when the input file contains `X 0\nM 0\n`, otherwise emit exactly `1552384` deterministic bytes and exit `0`.

The main success test should:

```python
with tempfile.TemporaryDirectory() as temp_dir:
    root = Path(temp_dir)
    fake_bin = root / "bin"
    fake_bin.mkdir()
    stim = write_fake_cli(fake_bin / "stim", mode="success")
    rstim = write_fake_cli(root / "target" / "release" / "rstim", mode="success")
    out_dir = root / "out"
    args = argparse.Namespace(
        manifest=FAIR_MANIFEST,
        case="stim_surface_d11_r100",
        profile="release",
        warmup_rounds=2,
        measure_rounds=7,
        out_dir=out_dir,
    )
    with mock.patch.dict(os.environ, {"PATH": f"{fake_bin}{os.pathsep}{os.environ.get('PATH', '')}"}), \
         mock.patch("benchmarks.rstim_vs_stim_simulator.run_fair_cli.build_rstim", return_value=rstim):
        run_fair_cli.run_fair_cli(args, repo_root=ROOT, command_line=["run-fair-cli"])
```

It should parse `raw.jsonl` independently and assert 18 records, required keys
for every record, variants `stim-cli-b8` and `rstim-cli-b8`, phases
`warmup` and `measured`, round indexes `0..1` and `0..6`, per-variant seeds
`0..8`, `exit_code == 0`, `actual_output_bytes == 1552384`, and `argv` first
elements resolving to the fake executables.

It should parse `summary.json` independently and recompute per-variant measured
elapsed samples from `raw.jsonl`:

```python
measured = [record for record in records if record["phase"] == "measured"]
self.assertEqual(len(measured), 14)
for variant in ("stim-cli-b8", "rstim-cli-b8"):
    samples = [record["elapsed_ns"] for record in measured if record["variant"] == variant]
    summary_variant = next(item for item in summary["variants"] if item["variant"] == variant)
    self.assertEqual(summary_variant["sample_count"], 7)
    self.assertEqual(summary_variant["elapsed_ns"]["median"], statistics.median(samples))
```

It should parse `environment.json` and assert the manifest, source manifest,
fixture, binary path/hash, exact argv, round counts, profile, timer scope,
seed policy, `stim_version == "1.15.0"`, and known-answer preflight success.

- [ ] **Step 2: Write the timing negative-control test**

Add a fake CLI mode that:

```python
payload = expected_output_bytes()
sys.stdout.buffer.write(payload[:-1])
sys.stdout.buffer.flush()
time.sleep(0.15)
sys.stdout.buffer.write(payload[-1:])
sys.stdout.buffer.flush()
sys.stdout.close()
time.sleep(0.15)
sys.exit(0)
```

Patch `time.perf_counter_ns` with deterministic values around one measured
process call so the accepted `elapsed_ns` proves the implementation waits until
process exit, not just first read or stdout close. Assert the delayed record's
elapsed time is at least `300_000_000`.

- [ ] **Step 3: Write the preflight negative-control test**

Add a fake CLI mode that returns `b"\x00"` for the known-answer circuit and
would otherwise emit valid benchmark bytes. Run `run_fair_cli.run_fair_cli`
and assert it raises `RuntimeError` containing `known-answer preflight`.
Assert `(out_dir / "raw.jsonl").exists()` is false.

- [ ] **Step 4: Run RED**

Run:

```sh
python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_run_fair_cli -q
```

Expected before Task 2: fail because `benchmarks.rstim_vs_stim_simulator.run_fair_cli` does not exist.

### Task 2: Implement The Fair CLI Runner

**Files:**
- Create: `benchmarks/rstim_vs_stim_simulator/run_fair_cli.py`
- Modify: `benchmarks/rstim_vs_stim_simulator/tests/test_run_fair_cli.py` only if a test needs a corrected assertion that still matches the issue.

**Interfaces:**
- Produces: `build_parser() -> argparse.ArgumentParser`.
- Produces: `main(argv: list[str] | None = None) -> int`.
- Produces: `run_fair_cli(args: argparse.Namespace, repo_root: Path = REPO_ROOT, command_line: list[str] | None = None) -> None`.
- Produces: `time_cli(argv: list[str], *, cwd: Path) -> ProcessResult`.
- Produces: `summarize_records(records: list[dict[str, object]], case: dict[str, Any]) -> dict[str, object]`.
- Produces: `render_report(summary: dict[str, object]) -> str`.

- [ ] **Step 1: Implement manifest loading and binary resolution**

`run_fair_cli.py` should import:

```python
from benchmarks.rstim_vs_stim_simulator import fair_cli_contract, run_speed_case

build_rstim = run_speed_case.build_rstim
```

Add `sha256_file(path: Path) -> str`, `repo_relative_path(path: Path, *, repo_root: Path) -> str`, `load_validated_case(manifest_path: Path, case_id: str, *, repo_root: Path) -> dict[str, Any]`, and `resolve_executable(name: str) -> Path`. `load_validated_case` must call `fair_cli_contract.validate_case` and raise `ValueError` with joined diagnostics before any process execution.

- [ ] **Step 2: Implement subprocess timing**

Add:

```python
@dataclass(frozen=True, slots=True)
class ProcessResult:
    exit_code: int
    stdout: bytes
    stderr: bytes
    elapsed_ns: int
```

`time_cli` should use `subprocess.Popen(..., stdout=subprocess.PIPE, stderr=subprocess.PIPE)`, start the timer immediately before `Popen`, call `communicate()`, stop the timer after `communicate()` returns, and return the captured bytes and return code.

- [ ] **Step 3: Implement argv expansion and preflight**

Expand each case `argv` template through `fair_cli_contract.expand_argv`, passing the resolved `rstim` binary path. Replace argv element `0` with the resolved Stim or `rstim` executable path used for execution. For preflight, copy the case with `shots = 1` and `canonical_input_path = str(temp_circuit_path)`, run both variants with seed `0`, and require `stdout == b"\x01"` and exit code `0`.

- [ ] **Step 4: Implement raw records and artifact writers**

Create rounds in variant-major order so each variant gets seeds `0..8`. For each process result, reject nonzero exit or wrong byte count immediately. Write raw records with the exact required keys. Build `summary.json` only from `phase == "measured"` records, and render `report.md` from that summary.

- [ ] **Step 5: Implement environment provenance**

Record `git_commit` from `git rev-parse HEAD`, `os` from `platform.platform()`,
`cpu_model` from `platform.processor()` or `platform.machine()`, parsed Stim
version `1.15.0`, `rstim` version from running the resolved binary with no
subcommand, Rust version from `rustc --version`, file paths plus SHA-256 for
fair manifest, source manifest, fixture, and both resolved binaries, exact
argv for all rounds, warmup and measure counts, and the preflight result.

- [ ] **Step 6: Run GREEN**

Run:

```sh
python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_run_fair_cli -q
```

Expected after implementation: all tests pass.

### Task 3: Canonical Verification And Final Checks

**Files:**
- Modify: only if verification exposes an issue in Task 2 behavior.

**Interfaces:**
- Consumes: `benchmarks.rstim_vs_stim_simulator.run_fair_cli` CLI.
- Produces: verified PR-ready branch.

- [ ] **Step 1: Run the issue verification command**

Run:

```sh
rm -rf /tmp/rstim-fair-cli
python3 -m benchmarks.rstim_vs_stim_simulator.run_fair_cli \
  --manifest benchmarks/rstim_vs_stim_simulator/fair_cli_cases.toml \
  --case stim_surface_d11_r100 \
  --profile release --warmup-rounds 2 --measure-rounds 7 \
  --out-dir /tmp/rstim-fair-cli
python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_run_fair_cli -q
```

Expected runner output:

```text
PASS symmetric fair CLI runner variants=2 warmups=4 measured=14 bytes_per_run=1552384
```

- [ ] **Step 2: Run repository verification**

Run:

```sh
cargo test
```

Expected: exit code `0`.

- [ ] **Step 3: Commit implementation**

Run:

```sh
git add docs/superpowers/plans/2026-07-10-issue-450-fair-cli-runner.md \
  benchmarks/rstim_vs_stim_simulator/run_fair_cli.py \
  benchmarks/rstim_vs_stim_simulator/tests/test_run_fair_cli.py
git commit -m "feat: add fair CLI sampling runner"
```

## Self Review

- Spec coverage: tasks cover validation before timing, known-answer preflight, subprocess timing scope, raw/summary/report/environment artifacts, Stim version enforcement, exact byte counts, fake CLI negative controls, and required verification.
- Red-flag scan: no vague implementation steps remain.
- Type consistency: function names in Task 2 match the test-facing interfaces in Task 1.
