# Issue 98 rbposd Fixture Catalog Design

## Context

`rbposd` now has two fixture surfaces that need shared provenance:

- parity-driver JSON fixtures under `rbposd/tests/fixtures/parity/`
- LSD-specific JSON fixtures under `rbposd/tests/fixtures/lsd/`

Issue #90 added an LSD-only manifest, and issue #97 extended the Python
parity harness so `product_sum`, `serial`, and supported LSD kwargs map to
upstream `ldpc`. Issue #98 should make that metadata shared and canonical
instead of leaving LSD and BP-option work in separate fixture descriptions.

## Approach

Add one checked-in catalog at `rbposd/tests/fixtures/catalog.json`. Each entry
records the fixture id, fixture kind, decoder mode, JSON fixture path, matrix
and syndrome JSON-pointer paths, provenance, verifier command, pass condition,
consuming issue ids, and mode tags.

The catalog is intentionally metadata-only. Existing fixture files remain the
source of matrix, syndrome, channel, config, and expected-result data.

## Rejected Options

1. Keep the existing LSD manifest and add a second BP-option manifest.
   This is the smallest local edit, but it preserves the split metadata model
   that issue #98 is meant to remove.
2. Generate the catalog in Rust or Python from the fixture directories.
   This proves coverage but does not give cold readers stable provenance,
   verifier, and pass-condition text to cite.
3. Move every fixture into one larger JSON document.
   This would make the catalog self-contained, but it would churn existing
   parity-driver and LSD fixture readers without adding useful behavior.

## Catalog Coverage

The shared catalog covers:

- every checked-in LSD fixture under `rbposd/tests/fixtures/lsd/`
- every checked-in parity fixture whose BP config uses a non-default method or
  schedule

Default OSD/BP baseline fixtures remain regular parity fixtures. They are not
BP-option alignment fixtures unless they use a non-default BP selector.

## Rust Validation

Add a test-side catalog parser under `rbposd/dev/fixture_catalog.rs` and a
focused integration test file `rbposd/tests/fixture_catalog.rs`.

The positive test is named
`fixture_catalog_manifest_covers_all_checked_in_lsd_and_bp_cases`. It loads the
catalog, validates required metadata, verifies each catalog path exists, checks
fixture id/name consistency, and compares catalog coverage against the checked
in LSD files and non-default BP parity files.

The negative test is named
`fixture_catalog_rejects_missing_provenance_or_verifier`. It mutates a valid
catalog in memory and verifies missing provenance or verifier text fails
validation.

LSD tests should read the shared catalog for their fixture list rather than
the old LSD-only manifest.

## Python Harness

Update `rbposd/scripts/parity_harness.py` so `--include-lsd` loads LSD cases
from `rbposd/tests/fixtures/catalog.json`. The harness should also use catalog
metadata for cataloged parity fixtures, avoiding duplicate entries when a
cataloged BP-option fixture is also present in the parity fixture directory.

Supported upstream `ldpc` mappings remain strict:

- `bp_variant=minimum_sum` -> `bp_method="minimum_sum"`
- `bp_variant=product_sum` -> `bp_method="product_sum"`
- `schedule=parallel` -> `schedule="parallel"`
- `schedule=serial` -> `schedule="serial"`
- `decoder=bp_lsd` with `localized_statistics` and `lsd_order` 0 or 1 maps
  into `BpLsdDecoder` kwargs

Unsupported BP methods, schedules, early-stop values, OSD methods, LSD methods,
LSD orders, and decoder-mode combinations must raise explicit errors rather
than silently coercing to supported defaults.

## Documentation

Update `rbposd/doc/ldpc_mvp_reference.md` to describe the shared catalog and
remove language that frames the LSD-only manifest as canonical.

## Testing

Run the issue verification:

- `cargo test -p rbposd fixture_catalog_manifest_covers_all_checked_in_lsd_and_bp_cases`
- `python3 -m pytest rbposd/scripts/test_parity_harness.py -k "lsd or bp_method"`
- `cargo test -p rbposd fixture_catalog_rejects_missing_provenance_or_verifier`
- `python3 -m pytest rbposd/scripts/test_parity_harness.py -k unsupported`

Run broader finish gates:

- `cargo test -p rbposd`
- `python3 -m pytest rbposd/scripts/test_parity_harness.py`
- `cargo test`
- `git diff --check`

## Scope

In scope:

- shared fixture catalog metadata
- Rust catalog validation tests
- Python parity-harness catalog loading
- strict supported/unsupported mapping tests
- reference documentation updates

Out of scope:

- `rsinter` runner changes
- benchmark spec coverage
- performance claims
- new decoder algorithms or fixture generation flows
