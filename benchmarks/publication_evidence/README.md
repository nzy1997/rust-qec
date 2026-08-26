# Publication benchmark evidence bundle (issue #601)

One versioned, machine-readable evidence bundle covering the repository's five
paper-facing benchmark families:

1. `surface-decoder-compare`
2. `bb-circuit-bposd-compare`
3. `rstim-vs-stim-simulator`
4. `rsmp-v1`
5. `qec-code-random-window`

## Layout

- `manifest.toml` — the publication contract: declared seeds `{7, 11, 17, 23, 31}`,
  confidence 0.95, minimum scale levels, hardware profile requirements, and each
  family's scale axes, required baselines, and required ablations.
- `results/<family>/<hardware-id>/<run-id>/` — one directory per run with
  `raw.jsonl` (one record per seed/variant/scale/repetition), `environment.json`
  (provenance), `summary.json` (derived estimates), and `artifact-sha256.json`.
- `report.md` and `readiness.json` — generated outputs; never hand-edit. The
  checker regenerates both and the committed copies must match byte-for-byte.

## Checking

```sh
make publication-evidence-check
```

runs the calibrated checker self-test (one positive plus seven negative-control
fixtures), validates every committed run (hashes, clean provenance, portable
paths, summary estimates recomputed from raw records), and compares the
regenerated report/readiness against the committed copies.

The checker exits 0 when all committed evidence is self-consistent. Its final
line is the full contract `PASS` line only when every publication readiness
gate is satisfied; until the offline campaign fills the declared-seed,
second-hardware, scale-level, baseline, and ablation gaps it prints a
`PARTIAL` line and `readiness.json` lists each open gate. `--require-ready`
turns any readiness gap into a hard failure.

## Importing existing artifacts

`tools/import_publication_evidence.py` converts the repository's committed
benchmark artifacts into bundle runs, recording source artifact hashes. Runs
whose original production provenance (commit, identified hardware, declared-seed
protocol) was not captured are marked `production_provenance.recorded = false`
and appear as readiness gaps; they are secondary diagnostics, not
publication-grade claims.
