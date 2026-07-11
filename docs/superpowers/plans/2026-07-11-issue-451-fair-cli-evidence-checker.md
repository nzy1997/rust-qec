# Issue 451 Fair CLI Evidence Checker Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Publish the checked fair CLI release evidence bundle and add a checker that validates it from raw records.

**Architecture:** Add one Python checker under `tools/` that validates the committed bundle in the required order: raw semantics, raw-derived summary, raw-derived report, #450 provenance and hashes, old #406 summary hash separation, and artifact hashes last. Reuse #450's `run_fair_cli._summary` and `_render_report` as the canonical derivation functions, and keep tests in a focused unittest module with temporary bundles for negative controls.

**Tech Stack:** Python standard library `argparse`, `copy`, `hashlib`, `json`, `shutil`, `subprocess`, `tempfile`, `unittest`, `pathlib`; existing `benchmarks.rstim_vs_stim_simulator.fair_cli_contract` and `benchmarks.rstim_vs_stim_simulator.run_fair_cli`.

## Global Constraints

- Checker CLI path is exactly `tools/check_rstim_vs_stim_fair_cli_evidence.py`.
- Checker interface is exactly `--dir <path>`.
- Publish the bundle under `benchmarks/rstim_vs_stim_simulator/results/fair-cli-release/`.
- Bundle files are exactly `raw.jsonl`, `summary.json`, `report.md`, `environment.json`, and `artifact-sha256.json`.
- `artifact-sha256.json` is mandatory and must map the other four bundle-relative filenames to lowercase SHA-256 digests.
- Validate semantics before artifact hashes.
- Require two warmups and seven measured records for each canonical variant.
- Require `b8`, `1024` shots, `12121` measurements, `1516` bytes per shot, `1552384` bytes per run, and `cli_end_to_end`.
- Verify exit status, record indexes, seeds, argv, and the known-answer preflight.
- Recompute the canonical summary from measured raw records only.
- Regenerate the canonical report from that recomputed summary.
- Require `summary.json` and `report.md` to equal the regenerated forms.
- Require all #450 provenance fields and verify fixture, source-manifest, fair-manifest, Stim-binary, and `rstim`-binary hashes.
- Verify `benchmarks/rstim_vs_stim_simulator/results/full/speed-summary.json` has SHA-256 `97ae397e598fe447d206c6b07a26ceaa0a3336d1883a7f77bc194f7b4c491805` and is not used as the new result.
- Only after those checks, verify `artifact-sha256.json`.
- Do not overwrite `results/full/` or `results/release/`.
- Do not update `site/benchmark-site.json`.
- Do not claim broad cross-machine performance parity.
- Required focused verification is `python3 -m unittest tools.test_check_rstim_vs_stim_fair_cli_evidence -q`.
- Required issue verification includes the checker command and `cargo test`.

---

## File Structure

- Create `tools/check_rstim_vs_stim_fair_cli_evidence.py`: standalone checker CLI and reusable validation helpers.
- Create `tools/test_check_rstim_vs_stim_fair_cli_evidence.py`: positive committed-bundle test plus temporary-bundle negative controls.
- Create directory `benchmarks/rstim_vs_stim_simulator/results/fair-cli-release/` with `raw.jsonl`, `summary.json`, `report.md`, `environment.json`, and `artifact-sha256.json`.
- Modify `benchmarks/rstim_vs_stim_simulator/run_fair_cli.py` only if tests need public aliases for the existing canonical summary/report helpers.

### Task 1: Test Checker Failure Ordering And Raw-Derived Regeneration

**Files:**
- Create: `tools/test_check_rstim_vs_stim_fair_cli_evidence.py`
- Create later: `tools/check_rstim_vs_stim_fair_cli_evidence.py`

**Interfaces:**
- Consumes after Task 2: checker CLI `python3 tools/check_rstim_vs_stim_fair_cli_evidence.py --dir <path>`.
- Produces test helpers `write_valid_bundle(path: Path) -> None`, `rewrite_json(path: Path, mutate: Callable[[dict[str, Any]], None]) -> None`, and `rewrite_artifact_hashes(bundle: Path) -> None`.

- [ ] **Step 1: Write the valid temporary bundle helper**

Create a unittest module that imports:

```python
from benchmarks.rstim_vs_stim_simulator import fair_cli_contract, run_fair_cli
```

Build 18 raw records for variants `stim-cli-b8` and `rstim-cli-b8`. For each
variant, write two warmup rows with indexes `0` and `1`, then seven measured
rows with indexes `0` through `6`. Use seeds `0` through `8`, elapsed samples
`1000 + seed` for Stim and `2000 + seed` for `rstim`, stdout hashes of 64
lowercase hex characters, and the exact constants from
`fair_cli_contract.EXPECTED_CASE`. Build `argv` with the expected executable
path, `sample`, `--shots`, `1024`, `--seed`, seed, `--out_format`, `b8`, `--in`,
and the canonical fixture path.

Call `run_fair_cli._summary(records, case=fair_cli_contract.EXPECTED_CASE)` and
`run_fair_cli._render_report(summary)` to write canonical `summary.json` and
`report.md`. Write `environment.json` with all #450 fields, absolute temporary
Stim and `rstim` binary paths, binary hashes, manifest/source/fixture paths and
hashes, `round_argv` mirroring raw records, and a passed preflight detail for
each variant. Write `artifact-sha256.json` for the other four bundle files.

- [ ] **Step 2: Write acceptance and required negative tests**

Add tests that invoke the checker as a subprocess and assert:

```python
self.assertEqual(result.returncode, 0, result.stderr)
self.assertEqual(
    "PASS fair CLI sampling evidence variants=2 measured=14\n",
    result.stdout,
)
```

Add negative tests:

- mutate a raw `stim-cli-b8` row to `output_format = "01"` without updating
  artifact hashes and assert stderr contains exactly
  `stim-cli-b8 output_format must be b8`;
- mutate `summary.json`, update artifact hashes, and assert stderr contains
  `summary.json does not match summary derived from raw.jsonl`;
- mutate `report.md`, update artifact hashes, and assert stderr contains
  `report.md does not match report derived from raw.jsonl`;
- remove `artifact-sha256.json` and assert stderr contains
  `missing required bundle file: artifact-sha256.json`.

- [ ] **Step 3: Run RED**

Run:

```sh
python3 -m unittest tools.test_check_rstim_vs_stim_fair_cli_evidence -q
```

Expected before Task 2: fail because `tools/check_rstim_vs_stim_fair_cli_evidence.py` does not exist.

### Task 2: Implement The Fair CLI Evidence Checker

**Files:**
- Create: `tools/check_rstim_vs_stim_fair_cli_evidence.py`
- Modify: `tools/test_check_rstim_vs_stim_fair_cli_evidence.py` only to correct assertions that still match #451.

**Interfaces:**
- Produces: `main(argv: list[str] | None = None) -> int`.
- Produces: `validate_bundle(results_dir: Path) -> tuple[int, int]`.
- Produces: `derive_summary(records: list[dict[str, Any]]) -> dict[str, Any]`.
- Produces: `render_report(summary: dict[str, Any]) -> str`.

- [ ] **Step 1: Implement file loading and required-file checks**

Create the checker with `REPO_ROOT = Path(__file__).resolve().parents[1]`.
Implement `load_json_object(path: Path, label: str) -> dict[str, Any]`,
`load_raw_records(path: Path) -> list[dict[str, Any]]`,
`sha256_file(path: Path) -> str`, and `validate_required_files(results_dir:
Path) -> None`. Missing files must raise `ValueError(f"missing required bundle
file: {filename}")`.

- [ ] **Step 2: Implement raw semantic validation**

Validate exactly two canonical variants and 18 total records. For each variant,
check phases, indexes, seeds, `case_id`, `shots`, `measurement_count`,
`output_format`, `bytes_per_shot` derived from measurement count, output bytes,
`timer_scope`, `exit_code`, `stdout_sha256`, and exact argv. Error messages
for per-variant fields use the form `{variant} output_format must be b8` so the
negative control observes the semantic error before any artifact hash error.

- [ ] **Step 3: Implement summary and report regeneration**

Call `run_fair_cli._summary(records, case=fair_cli_contract.EXPECTED_CASE)` and
`run_fair_cli._render_report(summary)`. Compare loaded `summary.json` with the
derived summary object, and compare `report.md` text with the derived report
text. Raise the exact messages required by #451 for mismatches.

- [ ] **Step 4: Implement environment provenance validation**

Require nonempty `git_commit`, `os`, `cpu_model`, `rstim_version`,
`rustc_version`, `profile == "release"`, `timer_scope == "cli_end_to_end"`,
`seed_policy == "round_index_0_through_8"`, `stim_version == "1.15.0"`,
`warmup_rounds == 2`, `measure_rounds == 7`, `known_answer_preflight ==
"passed"`, a preflight detail for each canonical variant, and `round_argv`
equal to raw records. Verify `fair_manifest_path`, `fair_manifest_sha256`,
`source_manifest_path`, `source_manifest_sha256`, `fixture_path`,
`fixture_sha256`, `stim_binary`, `stim_binary_sha256`, `rstim_binary`, and
`rstim_binary_sha256` against current file bytes. Accept the #450 alias fields
`manifest`/`manifest_sha256`, `source_manifest`/`source_manifest_sha256`, and
`fixture`/`fixture_sha256` only when the required path/hash fields are also
present and consistent.

- [ ] **Step 5: Implement historical and artifact hash checks**

Verify the old full speed summary hash at
`benchmarks/rstim_vs_stim_simulator/results/full/speed-summary.json`. Also
reject if the new `summary.json` bytes have that old digest. Then load
`artifact-sha256.json`, require exactly `raw.jsonl`, `summary.json`,
`report.md`, and `environment.json`, require lowercase 64-character hex
digests, and compare each digest to file bytes.

- [ ] **Step 6: Run GREEN**

Run:

```sh
python3 -m unittest tools.test_check_rstim_vs_stim_fair_cli_evidence -q
```

Expected after implementation: all tests pass.

### Task 3: Generate And Commit The Checked Fair CLI Bundle

**Files:**
- Create: `benchmarks/rstim_vs_stim_simulator/results/fair-cli-release/raw.jsonl`
- Create: `benchmarks/rstim_vs_stim_simulator/results/fair-cli-release/summary.json`
- Create: `benchmarks/rstim_vs_stim_simulator/results/fair-cli-release/report.md`
- Create: `benchmarks/rstim_vs_stim_simulator/results/fair-cli-release/environment.json`
- Create: `benchmarks/rstim_vs_stim_simulator/results/fair-cli-release/artifact-sha256.json`

**Interfaces:**
- Consumes: `benchmarks.rstim_vs_stim_simulator.run_fair_cli`.
- Produces: committed checked evidence bundle accepted by the checker.

- [ ] **Step 1: Run the release-profile fair CLI runner**

Run:

```sh
rm -rf /tmp/rstim-fair-cli
python3 -m benchmarks.rstim_vs_stim_simulator.run_fair_cli \
  --manifest benchmarks/rstim_vs_stim_simulator/fair_cli_cases.toml \
  --case stim_surface_d11_r100 \
  --profile release --warmup-rounds 2 --measure-rounds 7 \
  --out-dir /tmp/rstim-fair-cli
```

Expected runner output:

```text
PASS symmetric fair CLI runner variants=2 warmups=4 measured=14 bytes_per_run=1552384
```

- [ ] **Step 2: Copy only the requested artifacts**

Create `benchmarks/rstim_vs_stim_simulator/results/fair-cli-release/` and copy
only `raw.jsonl`, `summary.json`, `report.md`, and `environment.json` from
`/tmp/rstim-fair-cli`.

- [ ] **Step 3: Write artifact hashes**

Compute SHA-256 for the four copied files and write sorted JSON:

```json
{
  "environment.json": "<lowercase sha256>",
  "raw.jsonl": "<lowercase sha256>",
  "report.md": "<lowercase sha256>",
  "summary.json": "<lowercase sha256>"
}
```

- [ ] **Step 4: Check the committed bundle**

Run:

```sh
python3 tools/check_rstim_vs_stim_fair_cli_evidence.py \
  --dir benchmarks/rstim_vs_stim_simulator/results/fair-cli-release
```

Expected:

```text
PASS fair CLI sampling evidence variants=2 measured=14
```

### Task 4: Final Verification, Review, Commit, And PR

**Files:**
- Modify only files required by failed verification.

**Interfaces:**
- Consumes: all previous tasks.
- Produces: pushed branch and PR targeting `master`.

- [ ] **Step 1: Run focused verification**

Run:

```sh
python3 tools/check_rstim_vs_stim_fair_cli_evidence.py \
  --dir benchmarks/rstim_vs_stim_simulator/results/fair-cli-release
python3 -m unittest tools.test_check_rstim_vs_stim_fair_cli_evidence -q
```

Expected checker output:

```text
PASS fair CLI sampling evidence variants=2 measured=14
```

- [ ] **Step 2: Run required repository verification**

Run:

```sh
cargo test
```

Expected: exit code `0`.

- [ ] **Step 3: Review the diff**

Run:

```sh
git status -sb
git diff --stat
git diff --check
```

Expected: only issue #451 files are changed, and `git diff --check` exits `0`.

- [ ] **Step 4: Commit, push, and open PR**

Stage only the #451 design, plan, checker, tests, and fair CLI release bundle.
Commit with:

```sh
git commit -m "feat: add checked fair CLI evidence"
```

Push the current branch and create a pull request to `master` with a body that
summarizes the checker, bundle, and verification commands, and includes
`Closes #451`.

## Self Review

- Spec coverage: tasks cover bundle publication, raw semantic validation,
  summary/report regeneration, #450 provenance fields and hashes, old #406
  summary separation, artifact hash validation last, and required negative
  controls.
- Red-flag scan: no placeholder implementation steps remain.
- Type consistency: task interfaces match the checker and test module names
  required by #451.
