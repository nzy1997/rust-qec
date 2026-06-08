# Surface Decoder Partial Rerun Merge Design

Date: 2026-06-07
Status: Proposed
Scope: Partial reruns for `benchmarks/surface_decoder_compare` full-tier results

## Summary

This design adds a focused rerun mode for the surface decoder comparison
benchmark. The user can rerun a subset of decoders for an existing tier,
replace the matching rows inside the tracked `results.csv`, and then regenerate
the standard comparison plot from the merged result table.

The first target workflow is:

- rerun only BP-family decoders for `full`
- replace the existing `ldpc` and `rbposd` rows in
  `benchmarks/surface_decoder_compare/results/full/results.csv`
- preserve the existing rows for other decoders
- rerender `surface_decoder_compare.png` from the merged full table

The design intentionally treats this as a replacement operation keyed by case
identity, not as a raw append.

## Goals

- Support partial reruns without discarding the rest of the tracked full-tier
  benchmark table.
- Keep the main results path unchanged so existing plotting and repository
  workflows continue to work.
- Make replacement semantics explicit and deterministic.
- Keep the command surface small enough that a user can rerun selected
  decoders in one command.

## Non-Goals

- Do not add a generic result history database.
- Do not silently append duplicate benchmark rows.
- Do not redesign plotting around multiple input files.
- Do not add a backup artifact by default.

## Current State

[`benchmarks/surface_decoder_compare/run_compare.py`](/Users/nzy/rcode/rstim/benchmarks/surface_decoder_compare/run_compare.py:101)
already supports `--decoders`, `--distances`, and `--p-values`, but it always
rewrites `results/<tier>/results.csv` from only the rows produced in the
current run.

[`benchmarks/surface_decoder_compare/plot_compare.py`](/Users/nzy/rcode/rstim/benchmarks/surface_decoder_compare/plot_compare.py:128)
reads one tier-local `results.csv` and renders one tier-local PNG. It assumes
the CSV is already the complete table for that tier.

That means the current code can rerun `ldpc` and `rbposd`, but only by
throwing away all non-BP rows in `results/full/results.csv`.

## Decision Summary

Add an explicit merge mode to `run_compare.py`:

- new CLI flag: `--merge-into-existing`
- intended use: partial reruns with `--decoders`
- behavior: replace only the rows whose benchmark identity matches the new run
  output, preserve all other existing rows, then rewrite the canonical
  `results.csv`

Plotting does not need a new mode. After the merged `results.csv` is written,
the existing plot command rerenders the full comparison figure from the updated
table.

## Alternatives Considered

### 1. Append new rows to the existing CSV

Benefits:

- minimal implementation effort

Costs:

- duplicate rows for the same decoder and case
- ambiguous plotting behavior
- no clear answer for which row is authoritative

This option is rejected.

### 2. Write rerun results to a temporary file and add a separate merge command

Benefits:

- explicit operator flow
- easy to inspect rerun output before merge

Costs:

- two commands for the common case
- higher chance of user error or stale temporary files
- heavier workflow than needed for routine decoder refreshes

This option is not the preferred first step.

### 3. Add one in-place merge mode to `run_compare.py`

Benefits:

- one-step workflow for routine partial reruns
- keeps canonical file paths stable
- preserves current plot contract

Costs:

- `run_compare.py` now owns a small amount of merge logic

This is the chosen option.

## CLI Design

### New Flag

`run_compare.py` gains:

```text
--merge-into-existing
```

### Expected Command

```bash
.venv-surface-decoder/bin/python -m benchmarks.surface_decoder_compare.run_compare \
  --tier full \
  --decoders ldpc,rbposd \
  --merge-into-existing
```

Then rerender:

```bash
.venv-surface-decoder/bin/python -m benchmarks.surface_decoder_compare.plot_compare --tier full
```

### Validation Rules

- `--merge-into-existing` is only valid when `--decoders` is also provided.
- If `--merge-into-existing` is set and the target `results.csv` does not
  exist, treat the existing table as empty and write the new rows as the whole
  file.
- The merge mode works with additional `--distances` and `--p-values` filters.

## Merge Semantics

### Row Identity

A benchmark row is identified by:

- `tier`
- `decoder`
- `distance`
- `rounds`
- `p`
- `seed`

This key represents one decoder result for one benchmark point. `backend` is
not part of the identity because backend choice is result content that may
change between reruns and should therefore be replaced.

### Replacement Rule

Given:

- an existing canonical tier table
- a newly produced rerun row set

the merged table is computed as:

1. load existing rows if present
2. remove any existing row whose identity key matches one of the new rows
3. append all new rows
4. sort rows deterministically
5. rewrite the canonical tier `results.csv`

### Error Rows

Rows with `status=error` produced by the rerun replace matching old rows the
same way as successful rows. This keeps the canonical table honest about the
latest rerun outcome.

## Output Ordering

The rewritten CSV should be stable across runs. Sort rows by:

1. `decoder`
2. `distance`
3. `p`
4. `seed`

If two rows still compare equal on those fields, preserve the natural field
order already defined by the schema and key identity.

## Architecture Changes

The implementation should stay inside `run_compare.py` with small helper
functions:

- load existing rows from CSV
- compute row identity
- merge existing and new rows
- write the merged table using the existing CSV header

`run_suite(...)` should keep its current responsibility of producing the rows
for the current run. The merge behavior should live in `main(...)` or in a thin
helper called from `main(...)`, so tests can cover the CLI mode directly.

## Plotting Impact

No plot interface change is required.

After a successful merge, the canonical file remains:

- `benchmarks/surface_decoder_compare/results/full/results.csv`

The existing plot entrypoint continues to read that file and write:

- `benchmarks/surface_decoder_compare/results/full/surface_decoder_compare.png`

## Testing

Add focused tests for:

- `--merge-into-existing` rejected when `--decoders` is absent
- merge mode preserves unrelated decoder rows
- merge mode replaces matching decoder rows
- merge mode can bootstrap from a missing `results.csv`
- merged output order is deterministic

Existing plot tests should remain valid because the plot contract does not
change.

## Success Criteria

This design is successful if all of the following are true:

- a partial rerun can target `ldpc` and `rbposd` only
- the merged `full/results.csv` still contains the non-BP decoder rows that
  were not rerun
- the rerun rows replace the old BP rows instead of duplicating them
- the existing plot command rerenders the standard full-tier figure from the
  merged table without any manual CSV editing
