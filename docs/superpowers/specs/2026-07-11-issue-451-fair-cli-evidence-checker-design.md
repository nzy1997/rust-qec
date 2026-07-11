# Issue 451 Fair CLI Evidence Checker Design

Issue: #451

## Context

Issue #451 builds on #450, which added
`benchmarks/rstim_vs_stim_simulator/run_fair_cli.py`. That runner emits
`raw.jsonl`, `summary.json`, `report.md`, and `environment.json` for the
canonical `stim_surface_d11_r100` bit-packed CLI sampling case. This issue
publishes one checked release-profile bundle and adds a checker that derives
the trusted view from raw records instead of trusting summary, report, or
provenance claims inside the bundle.

Repository instructions live in `.AGENTS/AGENTS.md`. The QP101 visualization
rules are not relevant because this issue does not touch visualization export,
Typst fixtures, or rendered assets. Issue #451 has no comments. Issue #450 is
closed by PR #474, and its provenance fields define the environment fields this
checker must require and hash-check.

## Approaches Considered

1. Add a standalone checker at
   `tools/check_rstim_vs_stim_fair_cli_evidence.py` and publish the checked
   bundle under `benchmarks/rstim_vs_stim_simulator/results/fair-cli-release/`.
   The checker parses raw records, validates semantics, regenerates summary and
   report through the #450 canonical functions, validates provenance paths and
   hashes, checks the old #406 full summary hash, and only then verifies
   `artifact-sha256.json`.

2. Extend `tools/check_rstim_vs_stim_expanded_evidence.py` to include the fair
   CLI bundle. This would consolidate evidence checks, but the issue names a
   dedicated `--dir` interface and requires negative controls focused on
   semantic-before-hash ordering.

3. Treat `summary.json` as the authoritative input and only spot-check raw
   rows. This is simpler, but it directly contradicts the issue objective:
   checked evidence must be reproducible from raw records without trusting
   summary, report, or provenance claims embedded in the bundle.

The selected approach is option 1. It keeps the checker narrowly scoped, makes
validation ordering explicit, and matches the existing `tools/check_*` pattern.

## Checker Contract

Create `tools/check_rstim_vs_stim_fair_cli_evidence.py` with:

```sh
python3 tools/check_rstim_vs_stim_fair_cli_evidence.py \
  --dir benchmarks/rstim_vs_stim_simulator/results/fair-cli-release
```

The checker requires exactly these bundle files:

- `raw.jsonl`
- `summary.json`
- `report.md`
- `environment.json`
- `artifact-sha256.json`

Validation order is binding:

1. Parse `raw.jsonl` and validate semantic constraints for the canonical
   variants before checking artifact hashes.
2. Recompute summary from measured raw records only.
3. Regenerate report from the recomputed summary.
4. Require `summary.json` and `report.md` to equal the regenerated forms.
5. Require environment provenance fields and verify the fixture, source
   manifest, fair manifest, Stim binary, and `rstim` binary SHA-256 values.
6. Verify the historical `results/full/speed-summary.json` file still has
   SHA-256
   `97ae397e598fe447d206c6b07a26ceaa0a3336d1883a7f77bc194f7b4c491805` and is
   not reused as the new fair CLI summary.
7. Verify `artifact-sha256.json` exists, maps the other four relative
   filenames to lowercase SHA-256 digests, and matches current file bytes.

The success line is exactly:

```text
PASS fair CLI sampling evidence variants=2 measured=14
```

## Semantic Checks

The raw record contract is the source of truth. For each canonical variant
`stim-cli-b8` and `rstim-cli-b8`, the checker requires:

- exactly two `phase == "warmup"` rows with round indexes `0` and `1`;
- exactly seven `phase == "measured"` rows with round indexes `0` through `6`;
- seeds `0` through `8` in execution order for that variant;
- `case_id == "stim_surface_d11_r100"`;
- `shots == 1024`;
- `measurement_count == 12121`;
- `output_format == "b8"`;
- `timer_scope == "cli_end_to_end"`;
- `actual_output_bytes == 1552384`;
- `exit_code == 0`;
- `argv` equal to the #450 canonical expanded template for that variant, seed,
  shots, output format, and fixture path, while allowing the executable element
  to be the exact binary path recorded in `environment.json`.

The checker also confirms that environment `round_argv` exactly mirrors the
raw records and that `known_answer_preflight` is `passed` with one passing
detail per canonical variant.

## Published Bundle

Create `benchmarks/rstim_vs_stim_simulator/results/fair-cli-release/` from a
release-profile run of #450's runner for `stim_surface_d11_r100`, then add
`artifact-sha256.json`. The artifact hash file maps the four other
bundle-relative filenames to lowercase SHA-256 digests:

```json
{
  "environment.json": "...",
  "raw.jsonl": "...",
  "report.md": "...",
  "summary.json": "..."
}
```

The checked bundle is intentionally separate from `results/full/` and
`results/release/`. No site manifest or broad performance claim is updated.

## Tests

Add `tools/test_check_rstim_vs_stim_fair_cli_evidence.py`.

Coverage:

- accepts the committed bundle and prints the required pass line;
- constructs a temporary valid bundle from the runner's canonical summary and
  report functions so tests can mutate raw, summary, report, environment, and
  artifact hashes independently;
- proves counts and aggregates come from `raw.jsonl` by changing self-reported
  summary values and updating artifact hashes; the checker must still reject
  with `summary.json does not match summary derived from raw.jsonl`;
- applies the equivalent rejection to `report.md`;
- changes a raw `stim-cli-b8` record's `output_format` from `b8` to `01`
  without updating hashes; the checker must fail with
  `stim-cli-b8 output_format must be b8` before any hash error;
- rejects missing `artifact-sha256.json`;
- rejects incorrect artifact digests after semantic, summary, report, and
  provenance checks have passed.

## Scope Limits

- Do not overwrite `benchmarks/rstim_vs_stim_simulator/results/full/`.
- Do not overwrite `benchmarks/rstim_vs_stim_simulator/results/release/`.
- Do not update `site/benchmark-site.json`.
- Do not claim broad cross-machine performance parity.
- Do not change #450 runner semantics except for exposing helper functions
  needed for canonical checker reuse.

## Verification

Run:

```sh
python3 tools/check_rstim_vs_stim_fair_cli_evidence.py \
  --dir benchmarks/rstim_vs_stim_simulator/results/fair-cli-release
python3 -m unittest tools.test_check_rstim_vs_stim_fair_cli_evidence -q
cargo test
```

## Self Review

- No placeholders remain.
- The design validates raw semantics before artifact hashes.
- Summary and report are regenerated from measured raw records only.
- Provenance checks include all #450 fields requested by #451.
- The old #406 full summary hash is checked but not used as the new result.
