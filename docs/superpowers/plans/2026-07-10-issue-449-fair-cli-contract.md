# Issue 449 Fair CLI Contract Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a canonical fair CLI sampling contract for `stim_surface_d11_r100` and validate it before any benchmark process starts.

**Architecture:** Keep the contract as a small benchmark-package manifest plus a pure-Python validator. The validator loads the fair manifest, cross-checks the existing full fixture manifest, hashes the canonical input, expands argv templates without shell evaluation, and reports mismatches before any CLI benchmark command can run.

**Tech Stack:** Python standard library `argparse`, `hashlib`, `math`, `sys`, `tomllib`, `tempfile`, `subprocess`, `unittest`; existing `benchmarks/rstim_vs_stim_simulator/cases.full.toml` fixture manifest.

## Global Constraints

- Canonical case ID is exactly `stim_surface_d11_r100`.
- Source manifest path is exactly `benchmarks/rstim_vs_stim_simulator/cases.full.toml`.
- Source manifest case ID is exactly `stim_surface_d11_r100`.
- Canonical input path is exactly `benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim`.
- Canonical input SHA-256 is exactly `a49acb5edf3de447d47e401b012d043730b8b45077d5118a615066c2b5e8b229`.
- Stim version is exactly `1.15.0`.
- Shot count is exactly `1024`.
- Measurement count is exactly `12121`.
- Output format is exactly `b8`.
- Bytes per shot must be recomputed as `ceil(12121 / 8) = 1516`.
- Expected output bytes must be recomputed as `1516 * 1024 = 1552384`.
- Timer scope is exactly `cli_end_to_end`.
- Seed policy is exactly `round_index_0_through_8`.
- `stim-cli-b8` argv template is exactly `["stim", "sample", "--shots", "{shots}", "--seed", "{seed}", "--out_format", "b8", "--in", "{canonical_input_path}"]`.
- `rstim-cli-b8` argv template is exactly `["{rstim_binary}", "sample", "--shots", "{shots}", "--seed", "{seed}", "--out_format", "b8", "--in", "{canonical_input_path}"]`.
- Placeholder expansion must happen per argv element without shell evaluation.
- Validation must finish before benchmark process execution and must not run Stim or `rstim`.
- Do not run or publish timing evidence.
- Do not modify historical #406 artifacts.
- Do not update the site manifest.
- Do not add a wall-clock performance threshold.
- Required focused verification commands are:
  `python3 -m benchmarks.rstim_vs_stim_simulator.fair_cli_contract --manifest benchmarks/rstim_vs_stim_simulator/fair_cli_cases.toml --case stim_surface_d11_r100`
  and `python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_fair_cli_contract -q`.
- Required final verification command from Agent Desk is `cargo test`.

---

## File Structure

- Create `benchmarks/rstim_vs_stim_simulator/fair_cli_cases.toml`: canonical fair CLI benchmark manifest.
- Create `benchmarks/rstim_vs_stim_simulator/fair_cli_contract.py`: manifest loader, validator, argv expander, CLI entry point.
- Create `benchmarks/rstim_vs_stim_simulator/tests/test_fair_cli_contract.py`: focused unit tests and negative controls.

### Task 1: Fair CLI Contract Manifest And Validator

**Files:**
- Create: `benchmarks/rstim_vs_stim_simulator/tests/test_fair_cli_contract.py`
- Create: `benchmarks/rstim_vs_stim_simulator/fair_cli_cases.toml`
- Create: `benchmarks/rstim_vs_stim_simulator/fair_cli_contract.py`

**Interfaces:**
- Produces: `load_manifest(path: Path) -> dict[str, Any]`.
- Produces: `find_case(manifest: dict[str, Any], case_id: str) -> dict[str, Any]`.
- Produces: `expand_argv(template: list[str], case: dict[str, Any], *, seed: int = 0, rstim_binary: str = "rstim") -> list[str]`.
- Produces: `validate_case(case: dict[str, Any], *, manifest_path: Path, repo_root: Path) -> list[str]`.
- Produces: CLI `main(argv: list[str] | None = None) -> int`.
- CLI success prints `PASS fair CLI contract case=stim_surface_d11_r100 shots=1024 measurements=12121 format=b8 bytes_per_shot=1516 bytes=1552384 timer=cli_end_to_end`.

- [ ] **Step 1: Write the failing tests**

Create `benchmarks/rstim_vs_stim_simulator/tests/test_fair_cli_contract.py` with tests that import `fair_cli_contract`, run its CLI, independently recompute byte counts, inspect expanded argv arrays, and mutate temporary manifest copies for each negative control.

The test helper should run the CLI with:

```python
def run_contract(path: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [
            sys.executable,
            "-m",
            "benchmarks.rstim_vs_stim_simulator.fair_cli_contract",
            "--manifest",
            str(path),
            "--case",
            "stim_surface_d11_r100",
        ],
        cwd=ROOT,
        check=False,
        capture_output=True,
        text=True,
    )
```

The canonical success test should assert:

```python
self.assertEqual(result.returncode, 0, result.stderr)
self.assertEqual(
    result.stdout,
    "PASS fair CLI contract case=stim_surface_d11_r100 shots=1024 "
    "measurements=12121 format=b8 bytes_per_shot=1516 bytes=1552384 "
    "timer=cli_end_to_end\n",
)
self.assertEqual(result.stderr, "")
```

The independent byte-count test should compute:

```python
bytes_per_shot = (12121 + 7) // 8
self.assertEqual(bytes_per_shot, 1516)
self.assertEqual(bytes_per_shot * 1024, 1552384)
```

The argv inspection test should call:

```python
manifest = fair_cli_contract.load_manifest(FAIR_MANIFEST)
case = fair_cli_contract.find_case(manifest, "stim_surface_d11_r100")
stim_argv = fair_cli_contract.expand_argv(case["argv"]["stim-cli-b8"], case, seed=0, rstim_binary="target/release/rstim")
rstim_argv = fair_cli_contract.expand_argv(case["argv"]["rstim-cli-b8"], case, seed=0, rstim_binary="target/release/rstim")
self.assertEqual(stim_argv, ["stim", "sample", "--shots", "1024", "--seed", "0", "--out_format", "b8", "--in", "benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim"])
self.assertEqual(rstim_argv, ["target/release/rstim", "sample", "--shots", "1024", "--seed", "0", "--out_format", "b8", "--in", "benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim"])
```

Each negative test should copy the canonical manifest into a temporary file, mutate exactly one item, run the CLI, assert nonzero exit, and assert the field-specific diagnostic:

```python
self.assertIn("asymmetric output_format: expected b8", result.stderr)
self.assertIn("timer_scope", result.stderr)
self.assertIn("canonical_input_path", result.stderr)
self.assertIn("canonical_input_sha256", result.stderr)
```

- [ ] **Step 2: Run the focused tests to verify RED**

Run:

```sh
python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_fair_cli_contract -q
```

Expected before implementation: fail because `benchmarks.rstim_vs_stim_simulator.fair_cli_contract` does not exist.

- [ ] **Step 3: Add the canonical manifest**

Create `benchmarks/rstim_vs_stim_simulator/fair_cli_cases.toml` with:

```toml
manifest_version = 1
suite = "rstim_vs_stim_simulator"

[[cases]]
case_id = "stim_surface_d11_r100"
source_manifest_path = "benchmarks/rstim_vs_stim_simulator/cases.full.toml"
source_manifest_case_id = "stim_surface_d11_r100"
canonical_input_path = "benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim"
canonical_input_sha256 = "a49acb5edf3de447d47e401b012d043730b8b45077d5118a615066c2b5e8b229"
stim_version = "1.15.0"
shots = 1024
measurement_count = 12121
output_format = "b8"
bytes_per_shot = 1516
expected_output_bytes = 1552384
timer_scope = "cli_end_to_end"
seed_policy = "round_index_0_through_8"

[cases.argv]
stim-cli-b8 = ["stim", "sample", "--shots", "{shots}", "--seed", "{seed}", "--out_format", "b8", "--in", "{canonical_input_path}"]
rstim-cli-b8 = ["{rstim_binary}", "sample", "--shots", "{shots}", "--seed", "{seed}", "--out_format", "b8", "--in", "{canonical_input_path}"]
```

- [ ] **Step 4: Implement the validator**

Create `fair_cli_contract.py` with these behaviors:

```python
EXPECTED_CASE = {
    "case_id": "stim_surface_d11_r100",
    "source_manifest_path": "benchmarks/rstim_vs_stim_simulator/cases.full.toml",
    "source_manifest_case_id": "stim_surface_d11_r100",
    "canonical_input_path": "benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim",
    "canonical_input_sha256": "a49acb5edf3de447d47e401b012d043730b8b45077d5118a615066c2b5e8b229",
    "stim_version": "1.15.0",
    "shots": 1024,
    "measurement_count": 12121,
    "output_format": "b8",
    "bytes_per_shot": 1516,
    "expected_output_bytes": 1552384,
    "timer_scope": "cli_end_to_end",
    "seed_policy": "round_index_0_through_8",
}
EXPECTED_ARGV = {
    "stim-cli-b8": ["stim", "sample", "--shots", "{shots}", "--seed", "{seed}", "--out_format", "b8", "--in", "{canonical_input_path}"],
    "rstim-cli-b8": ["{rstim_binary}", "sample", "--shots", "{shots}", "--seed", "{seed}", "--out_format", "b8", "--in", "{canonical_input_path}"],
}
```

Implementation requirements:

- `load_manifest` opens TOML in binary mode and rejects non-table roots.
- `find_case` finds exactly one case ID and raises `ValueError` for missing or duplicate IDs.
- `expand_argv` calls `str.format_map` with only `shots`, `seed`, `canonical_input_path`, and `rstim_binary`, returning a list of strings.
- `validate_case` accumulates all errors into a list of strings.
- Hash validation reads the canonical input path from `repo_root`.
- Source manifest validation loads `cases.full.toml`, finds the source case, and checks `shots`, `expected_measurements`, `stim_version`, and canonical input path. The source manifest input path `fixtures/...` must resolve to the same file as the fair manifest path `benchmarks/rstim_vs_stim_simulator/fixtures/...`.
- Byte counts are recomputed from `measurement_count` and `shots`.
- `_argv_option(argv, "--in")` and `_argv_option(argv, "--out_format")` extract option values and report missing options.
- If either expanded argv uses an output format other than `b8`, append `asymmetric output_format: expected b8`.
- If the two expanded `--in` paths do not resolve to the same file, append a diagnostic containing `canonical_input_path`.
- The CLI catches `OSError`, `tomllib.TOMLDecodeError`, and `ValueError`, prints errors to stderr, and returns `1`.

- [ ] **Step 5: Run focused verification**

Run:

```sh
python3 -m benchmarks.rstim_vs_stim_simulator.fair_cli_contract --manifest benchmarks/rstim_vs_stim_simulator/fair_cli_cases.toml --case stim_surface_d11_r100
python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_fair_cli_contract -q
```

Expected after implementation: both commands pass, and the CLI output includes the required PASS line.

- [ ] **Step 6: Run final repository verification**

Run:

```sh
cargo test
```

Expected after implementation: exits `0`.

- [ ] **Step 7: Commit**

Run:

```sh
git add docs/superpowers/specs/2026-07-10-issue-449-fair-cli-contract-design.md docs/superpowers/plans/2026-07-10-issue-449-fair-cli-contract.md benchmarks/rstim_vs_stim_simulator/fair_cli_cases.toml benchmarks/rstim_vs_stim_simulator/fair_cli_contract.py benchmarks/rstim_vs_stim_simulator/tests/test_fair_cli_contract.py
git commit -m "feat: add fair CLI benchmark contract"
```

## Self Review

- Spec coverage: the manifest, validator, byte recomputation, argv expansion, source manifest cross-check, fixture SHA-256, success line, and all four negative controls are covered by Task 1.
- Placeholder scan: no TODO, TBD, or deferred implementation steps remain.
- Type consistency: `load_manifest`, `find_case`, `expand_argv`, `validate_case`, and `main` names match between task text and interfaces.
