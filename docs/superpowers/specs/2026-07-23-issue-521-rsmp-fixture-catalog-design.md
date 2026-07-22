# Issue 521 rsmp Fixture Catalog Design

Date: 2026-07-23
Status: approved by non-interactive Standing Answer Policy

## Purpose

Issue #521 creates the shared `rsmp` verification catalog used by later
transform, archive, CLI, compatibility, corruption, and performance tests. The
catalog must keep semantic fixture roles, independent known answers, and
format-aware corruption recipes in one checked source of truth.

## Approaches Considered

### 1. One declarative JSON catalog plus a strict Python checker

This is the selected approach. A single `catalog.json` stores all case metadata
and corruption recipes. The checker validates the schema, repository-relative
paths, committed-file SHA-256 values, b8 lengths, shape fields, role coverage,
known-answer coverage, and recipe taxonomy mappings. Small known-answer b8
vectors are committed as binary files. The large d11/r100 benchmark records its
producer command and hashes without duplicating output bytes.

### 2. Separate semantic and corruption catalogs

Separate files reduce each schema's size, but they also allow later tests to
consume only one catalog and silently miss cross-cutting provenance. This issue
exists to prevent that split.

### 3. Generated-only fixtures

Generated fixtures avoid committing small b8 files, but they make the
known-answer cases weaker as independent oracles. The issue explicitly requires
committed, hand-checkable expected bytes for the small cases.

## Catalog Shape

`rstim/tests/fixtures/rsmp/catalog.json` contains:

- `schema_version`;
- `format`;
- `cases`, one object per semantic, known-answer, or benchmark case;
- `corruption_recipes`, one object per deterministic mutation recipe.

Each case records a stable `id`, a plain-language `purpose`, `provenance`, a
repository-relative `circuit_path`, the circuit file SHA-256, dimensions
`M`, `D`, `L`, and `rank_H`, the shot count, the intended consumers, and either
a committed measurement input or a deterministic generation command. Known
answers additionally record committed measurement, detector, and observable
`b8` files with SHA-256 values and pinned Stim cross-check commands. The
benchmark entry must reference
`benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim`
exactly.

`valid_cases=7` in the PASS line counts the seven required semantic roles
covered by `semantic_roles`, not the number of entries in `cases`.

## Checker Design

`tools/check_rsmp_fixture_catalog.py` is a small standard-library script. It
accepts `--repo-root` and `--catalog` options, defaulting to the repository root
and `rstim/tests/fixtures/rsmp/catalog.json`. Validation is fail-fast enough to
produce stable diagnostics naming the invalid field or missing role.

Path validation is lexical and repository-relative: absolute paths and `..`
components are rejected before a file is read. Every committed file SHA-256
listed in the catalog is recomputed. For each committed `b8` file, the checker
compares file size to Stim-style byte-aligned rows,
`shots * ceil(bit_count / 8)`, and verifies unused padding bits in each row are
zero.

Recipe validation rejects duplicate IDs, missing expected codes, raw byte
offset selectors, and invalid mappings from the #520 taxonomy. Unknown
required-feature recipes must use `RSMP_UNSUPPORTED_FEATURE`; malformed
structure, ordering, padding, canonical encoding, and unknown-ID recipes must
use `RSMP_MALFORMED_ARCHIVE`; Zstandard decode recipes must use
`RSMP_DECOMPRESSION_FAILED`; and canonical logical payload mismatches must use
`RSMP_LOGICAL_DIGEST_MISMATCH`.

## Fixtures

Small source circuits live under `rstim/tests/fixtures/rsmp/`. The four
known-answer circuits and their exact b8 files are committed. A README in the
same directory documents the parity calculation and the independent Stim 1.15.0
cross-check command for each known answer.

Additional semantic-role cases may use deterministic producer commands instead
of committed measurement outputs when no small independent oracle is required.
This keeps the catalog useful for downstream tests without over-claiming
unimplemented archive behavior in this issue.

## Testing

The unit tests mutate temporary catalog copies and assert that the checker
rejects each required invalid state with diagnostics naming the offending field
or role. The required commands are:

```console
python3 tools/check_rsmp_fixture_catalog.py
python3 -m unittest tools.test_check_rsmp_fixture_catalog
cargo test
```

The checker's final stdout line is exactly:

```text
PASS rsmp fixture catalog valid_cases=7 known_answers=4 benchmark_cases=1 corruption_recipes>=12
```
