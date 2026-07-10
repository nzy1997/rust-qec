# Issue 449 Fair CLI Contract Design

Issue: #449

## Context

Issue #449 adds a preflight contract for the symmetric
`stim_surface_d11_r100` CLI benchmark. The related issue #406 records why this
matters: historical benchmark evidence compared paths with different output and
timing semantics. The new contract must make the selected CLI case
machine-checkable before any benchmark process starts.

No repository-specific `AGENTS.md`, `CLAUDE.md`, or `CONVENTIONS.md` files were
present in this worktree. The live issue has no comments, and no pull request
exists yet for this worker branch.

## Approaches Considered

1. Add a standalone Python contract validator and canonical TOML manifest.
   This keeps the work scoped to the requested interface, validates the
   fixture hash and source manifest relationship up front, expands argv
   templates as data, and never invokes Stim or `rstim`.

2. Fold the contract into an existing speed runner. This would make future
   runners use the contract directly, but it risks changing timing behavior and
   is broader than the issue objective.

3. Add only tests around existing manifests. This would catch some drift, but
   it would not provide the requested reusable CLI entry point or manifest.

The selected approach is option 1. It satisfies the issue interface exactly and
leaves timing runners, historical artifacts, site manifests, and performance
thresholds untouched.

## Manifest

Create `benchmarks/rstim_vs_stim_simulator/fair_cli_cases.toml` with one case:
`stim_surface_d11_r100`. The case declares these exact scalar fields:

- `source_manifest_path =
  "benchmarks/rstim_vs_stim_simulator/cases.full.toml"`
- `source_manifest_case_id = "stim_surface_d11_r100"`
- `canonical_input_path =
  "benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim"`
- `canonical_input_sha256 =
  "a49acb5edf3de447d47e401b012d043730b8b45077d5118a615066c2b5e8b229"`
- `stim_version = "1.15.0"`
- `shots = 1024`
- `measurement_count = 12121`
- `output_format = "b8"`
- `bytes_per_shot = 1516`
- `expected_output_bytes = 1552384`
- `timer_scope = "cli_end_to_end"`
- `seed_policy = "round_index_0_through_8"`

The manifest also contains exactly two argv templates:

- `stim-cli-b8 = ["stim", "sample", "--shots", "{shots}", "--seed",
  "{seed}", "--out_format", "b8", "--in", "{canonical_input_path}"]`
- `rstim-cli-b8 = ["{rstim_binary}", "sample", "--shots", "{shots}",
  "--seed", "{seed}", "--out_format", "b8", "--in",
  "{canonical_input_path}"]`

## Validator

Create `benchmarks.rstim_vs_stim_simulator.fair_cli_contract` with a CLI:

```sh
python3 -m benchmarks.rstim_vs_stim_simulator.fair_cli_contract \
  --manifest benchmarks/rstim_vs_stim_simulator/fair_cli_cases.toml \
  --case stim_surface_d11_r100
```

The validator loads TOML with `tomllib`, resolves paths relative to the
repository root, and performs only preflight checks. It does not run either
argv. On success it prints exactly:

```text
PASS fair CLI contract case=stim_surface_d11_r100 shots=1024 measurements=12121 format=b8 bytes_per_shot=1516 bytes=1552384 timer=cli_end_to_end
```

Validation checks:

- the selected case exists and no duplicate case ID is accepted;
- the fixture SHA-256 matches `canonical_input_sha256`;
- `cases.full.toml` contains `source_manifest_case_id`;
- source manifest `shots`, `expected_measurements`, `stim_version`, and
  canonical input path match the fair CLI case;
- `timer_scope` is exactly `cli_end_to_end`;
- `output_format` is exactly `b8`;
- `bytes_per_shot` is recomputed as `(measurement_count + 7) // 8`;
- `expected_output_bytes` is recomputed as `bytes_per_shot * shots`;
- both argv templates expand placeholders without shell evaluation;
- both expanded argv arrays use the same resolved `--in` path;
- both expanded argv arrays request `--out_format b8`;
- the `rstim` template keeps the binary path as a placeholder before
  expansion and uses the caller-provided `rstim_binary` replacement only as an
  argv element.

Failures return exit code `1` and print direct diagnostics to stderr. The
diagnostics include the mismatched field name. The asymmetric output format
negative control must include:

```text
asymmetric output_format: expected b8
```

## Tests

Add `benchmarks/rstim_vs_stim_simulator/tests/test_fair_cli_contract.py`.
Tests use temporary manifest copies and do not run benchmark processes.

Coverage:

- the CLI succeeds on the canonical manifest and prints the expected PASS line;
- tests independently recompute `ceil(12121 / 8) = 1516` and
  `1516 * 1024 = 1552384`;
- tests inspect fully expanded argv for `stim-cli-b8` and `rstim-cli-b8`;
- negative controls independently mutate one field per copy:
  `--out_format 01`, `timer_scope = "sample_only"`, one variant input path,
  and the fixture SHA-256;
- each negative control exits nonzero before any execution and identifies the
  mismatched field.

## Scope Limits

- Do not run or publish timing evidence.
- Do not modify historical #406 artifacts.
- Do not update the site manifest.
- Do not add a wall-clock performance threshold.
- Do not change the existing speed runners unless a test proves this contract
  requires it.

## Verification

Run:

```sh
python3 -m benchmarks.rstim_vs_stim_simulator.fair_cli_contract \
  --manifest benchmarks/rstim_vs_stim_simulator/fair_cli_cases.toml \
  --case stim_surface_d11_r100
python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_fair_cli_contract -q
cargo test
```

## Self Review

- No placeholders, TODOs, or incomplete requirements remain.
- The design implements the requested manifest and validator only.
- The byte-count contract is recomputed rather than trusted.
- The argv templates are validated as argv arrays, not shell strings.
- The negative controls match the issue text exactly.
