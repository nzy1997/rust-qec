# Issue 491 Post-Optimization Fair CLI Evidence Design

Issue: #491

## Context

Issue #491 refreshes the existing
`benchmarks/rstim_vs_stim_simulator/results/fair-cli-release` evidence slot
after the direct inverse repeat-folded work landed. The current slot's
`summary.json` is the pre-optimization baseline and must be preserved as
`baseline-summary.json` with SHA-256
`131ca52cce2c9108bc7bc7c638070f6c82d1a636d6554dbc9df21697e7f8ef07`.

The dependency issue #490 is closed by PR #507. The repository now contains
checked M3-3 reference-build evidence in
`benchmarks/rstim_vs_stim_simulator/results/reference-build-release/summary.json`.
The fair CLI candidate must cross-link to that checked reference summary and
record strategy `direct_inverse_repeat_folded`.

Repository instructions live in `.AGENTS/AGENTS.md`. The QP101 visualization
rules are not relevant because this issue does not touch visualization export,
Typst fixtures, or rendered assets.

## Approaches Considered

1. Refresh the existing `fair-cli-release` slot in place. Keep `raw.jsonl`,
   `summary.json`, `report.md`, and `environment.json` as the new candidate
   run. Add `baseline-summary.json` for the pinned old summary and
   `comparison.json` for the derived baseline-vs-candidate ratio. Extend the
   runner to write the comparison and reference cross-link, and extend the
   checker to validate all fields from raw and pinned evidence.

2. Store baseline and candidate in nested subdirectories. This separates old
   and new artifacts, but it would change the catalog shape and force broader
   portable-evidence updates than the issue asks for.

3. Manually add `comparison.json` without teaching the runner or checker how
   to derive it. This is fast, but the comparison would not be reproducible
   from the checked raw records and pinned baseline.

The selected approach is option 1. It preserves the current catalog slot,
keeps the historical baseline immutable, and lets the checker prove that the
published comparison is derived rather than hand-written.

## Artifact Contract

The refreshed `fair-cli-release` bundle contains exactly these checked files:

- `raw.jsonl`
- `summary.json`
- `baseline-summary.json`
- `comparison.json`
- `report.md`
- `environment.json`
- `artifact-sha256.json`

`raw.jsonl`, `summary.json`, and `report.md` describe the new symmetric
`b8`, 1024-shot, process-spawn-through-exit candidate run. The candidate still
uses the canonical `stim_surface_d11_r100` fixture, two warmups, seven
measured rounds, seeds `0` through `8`, and variants `stim-cli-b8` and
`rstim-cli-b8`.

`baseline-summary.json` is byte-for-byte the preserved pre-optimization
summary with SHA-256
`131ca52cce2c9108bc7bc7c638070f6c82d1a636d6554dbc9df21697e7f8ef07`.
The checker rejects any candidate `summary.json` with the same SHA-256 as that
baseline.

`comparison.json` is derived from `baseline-summary.json` and the candidate
`summary.json`. It records:

- `baseline_rstim_over_stim`, rounded to `3.576`;
- `candidate_rstim_over_stim`, rounded from candidate medians;
- `ratio_delta_from_baseline`, computed as candidate ratio minus `3.576`;
- the baseline and candidate median nanosecond values used for the ratio;
- `reference_strategy`, exactly `direct_inverse_repeat_folded`;
- the M3-3 reference summary path and SHA-256 used for the cross-link;
- a non-parity claim string that describes the measured ratio without making
  a threshold claim.

`artifact-sha256.json` hashes the other six artifact files. The portable
catalog's `artifacts` entries mirror those file hashes.

## Environment Cross-Link

`environment.json` keeps the current portable provenance fields and adds a
`reference_evidence` object:

```json
{
  "slot": "reference-build-release",
  "summary_path": "benchmarks/rstim_vs_stim_simulator/results/reference-build-release/summary.json",
  "summary_sha256": "<current M3-3 summary digest>",
  "reference_variant": "rstim-direct-repeat-reference-b8",
  "reference_strategy": "direct_inverse_repeat_folded",
  "checker": "tools/check_rstim_vs_stim_reference_build_evidence.py"
}
```

The fair CLI checker validates that the referenced path is repository-relative,
that the digest matches the checked summary file, that the referenced summary
contains the direct repeat variant, and that that variant's backend is
`direct_inverse_repeat_folded`.

## Checker Contract

The command remains:

```sh
python3 tools/check_rstim_vs_stim_fair_cli_evidence.py \
  --dir benchmarks/rstim_vs_stim_simulator/results/fair-cli-release
```

The success line remains exactly:

```text
PASS fair CLI sampling evidence variants=2 measured=14
```

Validation order is binding:

1. Required file presence.
2. Raw semantic invariants for the candidate run.
3. Candidate summary derived from raw records.
4. Candidate ratio derived from the candidate summary.
5. Pinned baseline summary hash and baseline ratio `3.576`.
6. `comparison.json` derived from baseline, candidate, and reference evidence.
7. Candidate environment provenance and M3-3 reference cross-link.
8. Report text derived from candidate summary and comparison.
9. Unsupported parity wording check for report/comparison prose.
10. Artifact hash manifest.

The checker must reject:

- baseline reused as candidate, with
  `candidate summary must differ from pinned baseline summary`;
- a mismatched reference-evidence hash;
- any parity wording in checked prose while the measured candidate ratio is
  greater than `1.0`.

The checker does not impose an absolute timing threshold and does not require
the candidate ratio to beat the baseline ratio.

## Testing

Update `tools/test_check_rstim_vs_stim_fair_cli_evidence.py` so the temporary
valid bundle includes baseline, comparison, and reference cross-link fields.

Coverage must include:

- accepts the committed refreshed bundle and prints the required pass line;
- rejects baseline reused as candidate with the required message;
- rejects a mismatched reference-evidence hash;
- rejects unsupported parity wording while candidate ratio exceeds `1.0`;
- verifies `comparison.json` is derived from raw candidate summary and pinned
  baseline, including `baseline_rstim_over_stim == 3.576` and
  `reference_strategy == "direct_inverse_repeat_folded"`;
- preserves existing raw-before-hash, summary-derivation, report-derivation,
  provenance, preflight, and artifact-hash negative controls.

Run:

```sh
python3 tools/check_rstim_vs_stim_fair_cli_evidence.py \
  --dir benchmarks/rstim_vs_stim_simulator/results/fair-cli-release
python3 -m unittest tools.test_check_rstim_vs_stim_fair_cli_evidence -q
cargo test
```

## Scope Limits

- Do not update `site/benchmark-site.json` or any site metadata.
- Do not close or rewrite #406 history.
- Do not impose a cross-machine timing threshold.
- Do not claim rstim/Stim parity unless the measured candidate ratio is at most
  `1.0`.
- Do not change the fair CLI workload, shot count, output format, fixture, or
  timer scope.

## Self Review

- No placeholders remain.
- The design preserves the pre-optimization summary under the exact required
  hash.
- The candidate comparison is derived from raw candidate evidence and the
  pinned baseline.
- The M3-3 cross-link validates both hash and
  `direct_inverse_repeat_folded` strategy.
- The checker rejects unsupported parity wording without turning timing into a
  threshold gate.
