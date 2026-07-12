# Issue 484 Checked Evidence Portability Gate Design

## Context

Issue #484 closes the portable evidence migration by making CI validate the
catalog and all four committed checked evidence bundles in a clean checkout.
Issues #480 through #483 already migrated the individual bundles so their
default checkers no longer require the publishing worktree, Stim installation,
Cargo builds, or `target/` products.

## Approach

Use a small aggregate checker at `tools/check_all_portable_evidence.py` with an
explicit bundle-id to checker registry. The command accepts:

```sh
python3 tools/check_all_portable_evidence.py \
  --catalog benchmarks/rstim_vs_stim_simulator/evidence_bundles.toml
```

The checker first calls the existing catalog loader and validator. If the
catalog fails, it prints the validator errors and exits nonzero. If the catalog
passes, it prints the existing catalog PASS line, then runs each registered
bundle checker in catalog order.

The aggregate checker imports the existing checker modules and calls their
`validate_bundle(...)` functions directly. It prints the same PASS line each
checker's CLI prints on success, so downstream logs preserve all existing
evidence confirmations. On failure, it catches the checker error and prints:

```text
FAIL portable checked evidence bundle=<bundle-id>: <checker error>
```

The bundle id comes from the catalog entry, so failures are actionable even when
the underlying checker error only names an artifact field.

## Components

- `tools/check_all_portable_evidence.py`: aggregate CLI, registry, catalog
  validation, pass/fail reporting.
- `tools/test_check_all_portable_evidence.py`: unit and integration coverage for
  success, direct-script help/import behavior, unknown bundle ids, and the fair
  CLI negative control with a rehashed absolute fixture path.
- `.github/workflows/ci.yml`: new `checked-evidence-portability` job immediately
  after checkout. It runs only standard-library Python and does not install
  Stim, configure Rust, use `rust-cache`, build Cargo targets, or touch
  `target/`.

## Data Flow

1. Parse `--catalog`.
2. Load TOML with `tomllib` through `portable_provenance.load_catalog`.
3. Validate schema, bundle ids, artifact hashes, logical commands, runtime
   identities, and checked provenance through `validate_catalog`.
4. For every catalog bundle, resolve `bundle_path` relative to the repository
   root and dispatch the matching registered checker.
5. Print checker-specific PASS lines and finish with:

```text
PASS portable checked evidence bundles=4
```

## Error Handling

Catalog errors are reported with the catalog path prefix, matching the existing
catalog validator CLI. Bundle errors are reported once with the failing bundle
id and the checker's exception text. The aggregate command stops at the first
bundle failure to keep CI logs short and specific.

Unknown catalog bundle ids are hard failures. This keeps the aggregate gate from
silently accepting a new bundle without a registered checker.

## Testing

Run the issue-required checks:

```sh
python3 tools/check_all_portable_evidence.py \
  --catalog benchmarks/rstim_vs_stim_simulator/evidence_bundles.toml
python3 -m unittest tools.test_check_all_portable_evidence -q
```

The final aggregate line must be:

```text
PASS portable checked evidence bundles=4
```

The negative control copies a valid fair CLI fixture, rewrites every fair CLI
raw `--in` argument and environment mirror to an absolute path, regenerates the
derived summary/report and artifact hash manifest, then asserts the aggregate
checker exits nonzero and prints:

```text
FAIL portable checked evidence bundle=fair-cli-release
```

Repository-level verification also runs `cargo test --workspace` as required by
the repo instructions and Agent Desk prompt.
