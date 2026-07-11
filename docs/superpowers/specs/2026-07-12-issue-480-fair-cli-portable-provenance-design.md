# Issue 480 Fair CLI Portable Provenance Design

Issue: #480 Migrate fair CLI evidence to portable provenance
Date: 2026-07-12

## Context

The fair CLI checked bundle was published before the schema-v2 portable
provenance catalog existed. Its `raw.jsonl` command records still contain the
publishing machine's absolute Stim, `rstim`, and fixture paths. The checker then
reconstructs expected argv using the current checkout path, so the committed
bundle fails after relocation even though its timings, output digests, summary,
and report are still the historical checked evidence.

Issue #479 added the portable schema-v2 catalog. The `fair-cli-release` catalog
entry already records the desired logical command shape with `tool://stim`,
`tool://rstim`, and the repo-relative fixture path. Issue #480 migrates the
bundle-specific runner, checker, and committed fair bundle to that same logical
shape.

This Agent Desk run is non-interactive. The Standing Answer Policy resolves the
Superpowers gates:

- Visual companion: not used because this is a backend provenance migration.
- Clarifying questions: answered from issues #480 and #479, merged PR #496, and
  the existing fair CLI runner/checker.
- Design approval: accepted automatically because issue #480 gives the exact
  interface, preserved summary/report hashes, verification commands, negative
  control, and out-of-scope limits.
- Spec review: this document is approved for planning after checking for
  placeholders, contradictions, ambiguity, and unrelated scope.

## Approaches Considered

1. Update only the checker to tolerate old absolute paths. This would make the
   current checkout pass, but it would leave raw provenance non-portable and
   contradict the required schema-v2 interface.
2. Migrate the runner, checker, and committed fair bundle to logical provenance.
   Raw argv records use `tool://stim` or `tool://rstim` as the executable and
   the catalog's repo-relative fixture path. Environment metadata records
   runtime identities by role, version, basename, and SHA-256, with no original
   absolute executable path. The checker validates this portable shape directly.
3. Add a sidecar compatibility map from old paths to current paths. This would
   avoid touching `raw.jsonl`, but it would create another provenance format and
   still fail the negative control that requires host-absolute argv rejection.

The selected approach is option 2. It is the smallest change that satisfies the
schema-v2 interface, preserves all measured values and derived report bytes, and
keeps the migration focused on fair CLI evidence only.

## Data Model

Raw records keep the existing timing and output fields unchanged. Only `argv`
changes:

- `stim-cli-b8` starts with `tool://stim`;
- `rstim-cli-b8` starts with `tool://rstim`;
- both variants use
  `benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim`
  after `--in`;
- all seed, shot, output format, phase, elapsed, output-byte, digest, and exit
  fields remain as recorded.

`environment.json` keeps the existing run identity fields that are not host
paths: commit, OS, CPU model, profile, timer scope, seed policy, versions,
round counts, preflight status, and preflight timing/output data. It replaces
absolute binary path fields with `runtime_identities`:

```json
[
  {
    "role": "tool://stim",
    "version": "1.15.0",
    "basename": "stim",
    "sha256": "e7f31b9ac1780080161b3992e70644ade97dbe97369a9464997645c437a29323"
  },
  {
    "role": "tool://rstim",
    "version": "rstim 0.1.1",
    "basename": "rstim",
    "sha256": "2db6fa113495235829ca1dc7e4f8080befe3e6336f8effb61800b9e84510182a"
  }
]
```

The same logical argv shape is used in `environment.round_argv` and
`known_answer_preflight_details[*].argv`. The known-answer input is a logical
temporary fixture token instead of the publishing machine's temporary path, so
the preflight record remains evidence of the performed check without requiring
the original host filesystem.

`summary.json` and `report.md` are not changed. Their SHA-256 values must stay:

- `summary.json`:
  `131ca52cce2c9108bc7bc7c638070f6c82d1a636d6554dbc9df21697e7f8ef07`;
- `report.md`:
  `1b28385ccf1523fac930feb4dc11542751884bdf99416e98815e0591d1960e51`.

`artifact-sha256.json` and the fair bundle entry in
`evidence_bundles.toml` are updated only for changed provenance files:
`raw.jsonl`, `environment.json`, and `artifact-sha256.json`. The summary and
report digests remain pinned to the preserved values above.

## Runner Changes

`benchmarks/rstim_vs_stim_simulator/run_fair_cli.py` continues to execute real
local binaries when a benchmark is run, but it records portable provenance:

- execution argv stays internal to the runner and uses resolved local paths;
- recorded raw argv is converted to the schema-v2 logical role and repo-relative
  fixture path;
- environment runtime identities are derived from the executed binaries'
  version, basename, and SHA-256 values, without storing their live paths;
- round argv and known-answer preflight details record logical argv.

No benchmark is rerun for this issue. The runner change only ensures future fair
CLI bundles use the portable format.

## Checker Changes

`tools/check_rstim_vs_stim_fair_cli_evidence.py` validates the portable format
as the canonical semantics:

- raw argv must equal the logical schema-v2 argv for each variant and seed;
- any raw argv, round argv, or preflight argv element containing a host-absolute
  path fails before artifact hashes, with variant-specific errors such as
  `stim-cli-b8 argv contains a host-absolute path`;
- fixture, source manifest, and fair manifest provenance remain repo-relative
  and their SHA-256 digests are checked against the current checkout;
- runtime identities must contain exactly role, version, basename, and SHA-256
  for `tool://stim` and `tool://rstim`, with no live path fields;
- `round_argv` must still mirror `raw.jsonl`;
- summary and report are regenerated from raw records exactly as before;
- the historical #406 full-summary SHA-256 check remains unchanged;
- artifact hashes are checked last.

This preserves the existing semantic-before-hash ordering and adds the #480
negative control for host-absolute argv.

## Tests

Update `tools/test_check_rstim_vs_stim_fair_cli_evidence.py` so its temporary
valid bundle uses logical argv and runtime identities. Keep existing summary,
report, artifact-hash, preflight, and raw-semantic negative tests.

Add focused coverage for:

- the committed fair bundle passes the checker;
- a relocated `git archive HEAD` checkout passes the same checker command;
- replacing one logical fixture argument with `/tmp/copied-fixture.stim` and
  refreshing artifact hashes fails with
  `stim-cli-b8 argv contains a host-absolute path`;
- runtime identities reject old live path fields.

Final verification:

```sh
python3 tools/check_rstim_vs_stim_fair_cli_evidence.py \
  --dir benchmarks/rstim_vs_stim_simulator/results/fair-cli-release
tmp="$(mktemp -d)"
git archive HEAD | tar -x -C "$tmp"
(cd "$tmp" && python3 tools/check_rstim_vs_stim_fair_cli_evidence.py \
  --dir benchmarks/rstim_vs_stim_simulator/results/fair-cli-release)
python3 -m unittest tools.test_check_rstim_vs_stim_fair_cli_evidence -q
python3 -m benchmarks.rstim_vs_stim_simulator.validate_evidence_bundles \
  --catalog benchmarks/rstim_vs_stim_simulator/evidence_bundles.toml
cargo test
```

## Out Of Scope

This migration does not rerun timing, optimize reference sampling, update the
site, alter `summary.json`, alter `report.md`, or overwrite the historical #406
evidence.

## Self-Review

- No placeholders remain.
- The selected approach matches issue #480's required raw argv, environment,
  summary/report preservation, verification, and negative control.
- The design separates execution-time local paths from recorded portable
  provenance.
- The checker keeps raw-derived summary/report validation and the historical
  #406 digest guard.
- The scope is limited to fair CLI provenance, its checker/tests, and catalog
  hashes for the changed fair bundle artifacts.
