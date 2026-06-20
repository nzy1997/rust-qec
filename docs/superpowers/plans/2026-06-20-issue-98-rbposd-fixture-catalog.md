# Issue 98 rbposd Fixture Catalog Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add one shared LSD and BP-option fixture catalog and make the Rust and Python parity harnesses consume and validate it.

**Architecture:** Store canonical metadata in `rbposd/tests/fixtures/catalog.json` while keeping existing fixture JSON files as the source of case data. Add a test-side Rust catalog validator for coverage and metadata checks, then update the Python harness so cataloged BP-option and LSD cases are loaded from the catalog without duplicate parity entries.

**Tech Stack:** Rust 2024 integration tests, `serde_json`, Cargo workspace, Python 3 `pytest`, `rbposd/scripts/parity_harness.py`.

## Global Constraints

- Add one checked-in catalog at `rbposd/tests/fixtures/catalog.json`.
- Every catalog entry must include fixture id, fixture kind, decoder mode, fixture path, matrix path, syndrome path, provenance/source, verifier command, pass condition, consuming issue ids, and mode tags.
- The shared catalog must cover every checked-in LSD fixture under `rbposd/tests/fixtures/lsd/`.
- The shared catalog must cover every checked-in parity fixture whose BP config uses a non-default BP method or schedule.
- Default OSD/BP baseline fixtures remain regular parity fixtures and are not catalog-required BP-option fixtures.
- The Python harness must use the shared catalog for LSD cases and cataloged BP-option parity cases.
- Supported upstream `ldpc` mappings remain limited to `minimum_sum`, `product_sum`, `parallel`, `serial`, `OSD_0`, and `localized_statistics` with `lsd_order` 0 or 1.
- Unsupported schedules, methods, early-stop values, OSD variants, LSD methods, LSD orders, and decoder-mode combinations must be rejected explicitly.
- Do not change `rsinter`, benchmark specs, performance docs, or decoder algorithms.
- Run `cargo test -p rbposd fixture_catalog_manifest_covers_all_checked_in_lsd_and_bp_cases`.
- Run `cargo test -p rbposd fixture_catalog_rejects_missing_provenance_or_verifier`.
- Run `python3 -m pytest rbposd/scripts/test_parity_harness.py -k "lsd or bp_method"`.
- Run `python3 -m pytest rbposd/scripts/test_parity_harness.py -k unsupported`.
- Run `cargo test -p rbposd`, `python3 -m pytest rbposd/scripts/test_parity_harness.py`, `cargo test`, and `git diff --check` before finishing.

---

## File Structure

- Create `rbposd/tests/fixtures/catalog.json`: shared metadata catalog.
- Create `rbposd/dev/fixture_catalog.rs`: test-side catalog parser and validator.
- Create `rbposd/tests/fixture_catalog.rs`: focused catalog coverage and negative-control tests.
- Modify `rbposd/tests/lsd.rs`: load LSD fixture list from the shared catalog instead of the LSD-only manifest.
- Delete `rbposd/tests/fixtures/lsd/manifest.json`: remove the disconnected LSD-only manifest.
- Modify `rbposd/scripts/parity_harness.py`: load catalog entries, avoid duplicate cataloged parity fixtures, and reject unsupported decoder-mode combinations.
- Modify `rbposd/scripts/test_parity_harness.py`: cover shared catalog loading, LSD/BP-option mapping, duplicate avoidance, and unsupported modes.
- Modify `rbposd/doc/ldpc_mvp_reference.md` and `rbposd/tests/reference.rs`: document and test the new shared catalog contract.

## Task 1: Shared Catalog and Rust Validation

**Files:**
- Create: `rbposd/tests/fixtures/catalog.json`
- Create: `rbposd/dev/fixture_catalog.rs`
- Create: `rbposd/tests/fixture_catalog.rs`
- Modify: `rbposd/tests/lsd.rs`
- Delete: `rbposd/tests/fixtures/lsd/manifest.json`

**Interfaces:**
- Consumes: checked-in fixture JSON under `rbposd/tests/fixtures/parity/` and `rbposd/tests/fixtures/lsd/`.
- Produces:
  - `fixture_catalog::load_catalog(path: &Path) -> FixtureCatalog`
  - `fixture_catalog::validate_catalog(catalog: &FixtureCatalog, fixture_root: &Path) -> Result<Vec<ValidatedFixtureCatalogEntry>, String>`
  - `fixture_catalog::catalog_path() -> PathBuf`
  - `fixture_catalog::fixture_root() -> PathBuf`
  - tests `fixture_catalog_manifest_covers_all_checked_in_lsd_and_bp_cases` and `fixture_catalog_rejects_missing_provenance_or_verifier`

- [ ] **Step 1: Write the failing Rust catalog tests**

Create `rbposd/tests/fixture_catalog.rs` with:

```rust
#[path = "../dev/fixture_catalog.rs"]
mod fixture_catalog;

#[test]
fn fixture_catalog_manifest_covers_all_checked_in_lsd_and_bp_cases() {
    let catalog = fixture_catalog::load_catalog(&fixture_catalog::catalog_path());
    let entries = fixture_catalog::validate_catalog(&catalog, &fixture_catalog::fixture_root())
        .unwrap();

    let ids = entries
        .iter()
        .map(|entry| entry.id.as_str())
        .collect::<Vec<_>>();

    assert_eq!(
        ids,
        vec![
            "bp_product_sum_serial_sensitive",
            "lsd_order_one_improves_over_baseline",
            "lsd_small_sparse_code",
            "lsd_unsatisfiable_case",
        ]
    );
}

#[test]
fn fixture_catalog_rejects_missing_provenance_or_verifier() {
    let valid = fixture_catalog::load_catalog(&fixture_catalog::catalog_path());

    let mut missing_provenance = valid.clone();
    missing_provenance.fixtures[0].provenance.clear();
    let error =
        fixture_catalog::validate_catalog(&missing_provenance, &fixture_catalog::fixture_root())
            .unwrap_err();
    assert!(
        error.contains("provenance"),
        "expected provenance validation error, got {error:?}"
    );

    let mut missing_verifier = valid.clone();
    missing_verifier.fixtures[0].verifier.clear();
    let error =
        fixture_catalog::validate_catalog(&missing_verifier, &fixture_catalog::fixture_root())
            .unwrap_err();
    assert!(
        error.contains("verifier"),
        "expected verifier validation error, got {error:?}"
    );
}
```

- [ ] **Step 2: Verify the Rust catalog tests are red**

Run:

```bash
cargo test -p rbposd fixture_catalog_manifest_covers_all_checked_in_lsd_and_bp_cases
cargo test -p rbposd fixture_catalog_rejects_missing_provenance_or_verifier
```

Expected before implementation: fail because `rbposd/dev/fixture_catalog.rs`
and/or `rbposd/tests/fixtures/catalog.json` does not exist.

- [ ] **Step 3: Add the shared catalog JSON**

Create `rbposd/tests/fixtures/catalog.json` with entries matching this shape:

```json
{
  "fixtures": [
    {
      "id": "bp_product_sum_serial_sensitive",
      "kind": "bp_option",
      "decoder": "bp_osd",
      "path": "parity/bp_product_sum_serial_sensitive.json",
      "matrix_path": "parity/bp_product_sum_serial_sensitive.json#/matrix",
      "syndrome_path": "parity/bp_product_sum_serial_sensitive.json#/syndrome",
      "provenance": "Repo-owned BP-option teeth fixture introduced by issues #95 and #97 to prove product_sum + serial changes public rbposd parity-driver behavior.",
      "verifier": "cargo test -p rbposd product_sum_serial_teeth_cases",
      "pass_condition": "The product_sum + serial fixture matches its expected Rust parity-driver output and differs from minimum_sum serial and product_sum parallel variants.",
      "consumes": ["#95", "#97", "#98"],
      "modes": ["bp_variant=product_sum", "schedule=serial", "osd_variant=osd0"]
    }
  ]
}
```

Then add the three LSD entries with:

```json
{
  "id": "lsd_small_sparse_code",
  "kind": "lsd",
  "decoder": "bp_lsd",
  "path": "lsd/lsd_small_sparse_code.json",
  "matrix_path": "lsd/lsd_small_sparse_code.json#/matrix",
  "syndrome_path": "lsd/lsd_small_sparse_code.json#/syndrome",
  "provenance": "Repo-owned small sparse LSD alignment case introduced by issue #89 for the first borrowed fixture set.",
  "verifier": "cargo test -p rbposd bplsd_fixture_manifest_cases_decode_cleanly",
  "pass_condition": "BpLsdDecoder with lsd_order=1 decodes the syndrome to residual zero without using OSD.",
  "consumes": ["#89", "#90", "#98"],
  "modes": ["decoder=bp_lsd", "lsd_order=1", "lsd_method=localized_statistics"]
}
```

Use the existing provenance and pass-condition text from
`rbposd/tests/fixtures/lsd/manifest.json` for the other two LSD entries, and
add `#98` to their `consumes` lists.

- [ ] **Step 4: Implement the Rust catalog validator**

Create `rbposd/dev/fixture_catalog.rs` with:

```rust
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct FixtureCatalog {
    pub fixtures: Vec<FixtureCatalogEntry>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum FixtureKind {
    BpOption,
    Lsd,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FixtureCatalogEntry {
    pub id: String,
    pub kind: FixtureKind,
    pub decoder: String,
    pub path: String,
    pub matrix_path: String,
    pub syndrome_path: String,
    pub provenance: String,
    pub verifier: String,
    pub pass_condition: String,
    pub consumes: Vec<String>,
    pub modes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedFixtureCatalogEntry {
    pub id: String,
    pub kind: FixtureKind,
    pub path: PathBuf,
}

pub fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
}

pub fn catalog_path() -> PathBuf {
    fixture_root().join("catalog.json")
}

pub fn load_catalog(path: &Path) -> FixtureCatalog {
    let contents = fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
    serde_json::from_str(&contents)
        .unwrap_or_else(|error| panic!("failed to parse {}: {error}", path.display()))
}
```

In the same file, add `validate_catalog` helpers that enforce:

```rust
if catalog.fixtures.is_empty() {
    return Err("fixture catalog fixtures must not be empty".to_string());
}
if entry.id.trim().is_empty() {
    return Err("fixture catalog entry id must not be empty".to_string());
}
if entry.path.trim().is_empty() {
    return Err(format!("fixture catalog entry {} path must not be empty", entry.id));
}
if entry.matrix_path.trim().is_empty() || !entry.matrix_path.starts_with(&entry.path) {
    return Err(format!("fixture catalog entry {} matrix_path must reference its fixture path", entry.id));
}
if entry.syndrome_path.trim().is_empty() || !entry.syndrome_path.starts_with(&entry.path) {
    return Err(format!("fixture catalog entry {} syndrome_path must reference its fixture path", entry.id));
}
if entry.provenance.trim().is_empty() {
    return Err(format!("fixture catalog entry {} provenance must not be empty", entry.id));
}
if entry.verifier.trim().is_empty() {
    return Err(format!("fixture catalog entry {} verifier must not be empty", entry.id));
}
if entry.pass_condition.trim().is_empty() {
    return Err(format!("fixture catalog entry {} pass_condition must not be empty", entry.id));
}
if entry.consumes.is_empty() || !entry.consumes.iter().any(|value| value == "#98") {
    return Err(format!("fixture catalog entry {} must consume #98", entry.id));
}
if entry.modes.is_empty() {
    return Err(format!("fixture catalog entry {} modes must not be empty", entry.id));
}
```

Also validate duplicate ids and paths, path existence, id/name consistency, and
coverage:

- LSD catalog entries must point under `lsd/` and their fixture JSON `id` must
  match the catalog `id`.
- BP-option entries must point under `parity/`, their fixture JSON `name` must
  match the catalog `id`, and the fixture config must have
  `bp_variant != "minimum_sum"` or `schedule != "parallel"`.
- Every checked-in `lsd/*.json` fixture must have one catalog entry.
- Every checked-in `parity/*.json` fixture with non-default BP config must have
  one catalog entry.
- No catalog entry may point at a fixture outside those required groups.

Sort the returned `ValidatedFixtureCatalogEntry` list by `id`.

- [ ] **Step 5: Port LSD tests to the shared catalog**

In `rbposd/tests/lsd.rs`:

1. Remove the local `LsdFixtureManifest`, `LsdFixtureManifestEntry`,
   `ValidatedLsdManifestEntry`, `lsd_manifest_path`, `assert_manifest_error`,
   and `validate_lsd_fixture_manifest` definitions.
2. Add:

```rust
#[path = "../dev/fixture_catalog.rs"]
mod fixture_catalog;
```

3. In `bplsd_fixture_manifest_cases_decode_cleanly`, replace the manifest load
   with:

```rust
let catalog = fixture_catalog::load_catalog(&fixture_catalog::catalog_path());
let entries = fixture_catalog::validate_catalog(&catalog, &fixture_catalog::fixture_root())
    .unwrap()
    .into_iter()
    .filter(|entry| entry.kind == fixture_catalog::FixtureKind::Lsd)
    .collect::<Vec<_>>();
```

4. Delete `bplsd_fixture_manifest_rejects_invalid_case_metadata`; the shared
   negative coverage now lives in `rbposd/tests/fixture_catalog.rs`.
5. Delete `rbposd/tests/fixtures/lsd/manifest.json`.

- [ ] **Step 6: Verify Task 1 is green**

Run:

```bash
cargo test -p rbposd fixture_catalog_manifest_covers_all_checked_in_lsd_and_bp_cases
cargo test -p rbposd fixture_catalog_rejects_missing_provenance_or_verifier
cargo test -p rbposd bplsd_fixture_manifest_cases_decode_cleanly
```

Expected after implementation: all listed tests pass.

- [ ] **Step 7: Commit Task 1**

Run:

```bash
git add rbposd/dev/fixture_catalog.rs rbposd/tests/fixture_catalog.rs rbposd/tests/fixtures/catalog.json rbposd/tests/lsd.rs rbposd/tests/fixtures/lsd/manifest.json
git commit -m "test: add rbposd shared fixture catalog"
```

## Task 2: Python Harness Catalog Consumption

**Files:**
- Modify: `rbposd/scripts/parity_harness.py`
- Modify: `rbposd/scripts/test_parity_harness.py`

**Interfaces:**
- Consumes:
  - `load_fixture_catalog(catalog_path: Path) -> dict[str, Any]`
  - `iter_catalog_fixture_cases(catalog_path: Path, include_lsd: bool) -> list[dict[str, Any]]`
  - existing `map_config_to_ldpc_kwargs`
  - existing `map_lsd_case_to_ldpc_kwargs`
- Produces:
  - cataloged parity and LSD entries for `build_entries`
  - explicit errors for unsupported decoder-mode combinations

- [ ] **Step 1: Write failing Python catalog tests**

In `rbposd/scripts/test_parity_harness.py`, update imports:

```python
from parity_harness import (
    build_entries,
    classify_mismatch,
    is_real_mismatch,
    iter_catalog_fixture_cases,
    iter_generated_cases,
    iter_lsd_fixture_cases,
    load_fixture_catalog,
    map_config_to_ldpc_kwargs,
    map_lsd_case_to_ldpc_kwargs,
    matrix_to_dense,
)
```

Replace the old `load_lsd_manifest` test with a shared-catalog test that writes
`catalog.json` at a temporary fixture root:

```python
def write_catalog_fixture(root: Path) -> Path:
    catalog_path = root / "catalog.json"
    (root / "lsd").mkdir()
    (root / "parity").mkdir()
    catalog_path.write_text(
        """
{
  "fixtures": [
    {
      "id": "lsd_small_sparse_code",
      "kind": "lsd",
      "decoder": "bp_lsd",
      "path": "lsd/lsd_small_sparse_code.json",
      "matrix_path": "lsd/lsd_small_sparse_code.json#/matrix",
      "syndrome_path": "lsd/lsd_small_sparse_code.json#/syndrome",
      "provenance": "unit test provenance",
      "verifier": "python3 -m pytest rbposd/scripts/test_parity_harness.py -k lsd",
      "pass_condition": "unit test pass condition",
      "consumes": ["#90", "#98"],
      "modes": ["decoder=bp_lsd", "lsd_order=1"]
    },
    {
      "id": "bp_product_sum_serial_sensitive",
      "kind": "bp_option",
      "decoder": "bp_osd",
      "path": "parity/bp_product_sum_serial_sensitive.json",
      "matrix_path": "parity/bp_product_sum_serial_sensitive.json#/matrix",
      "syndrome_path": "parity/bp_product_sum_serial_sensitive.json#/syndrome",
      "provenance": "unit test bp provenance",
      "verifier": "cargo test -p rbposd product_sum_serial_teeth_cases",
      "pass_condition": "unit test bp pass condition",
      "consumes": ["#97", "#98"],
      "modes": ["bp_variant=product_sum", "schedule=serial"]
    }
  ]
}
""",
        encoding="utf-8",
    )
    return catalog_path
```

Add tests named:

- `test_iter_lsd_fixture_cases_loads_catalog_entries`
- `test_iter_lsd_fixture_cases_rejects_empty_catalog_metadata`
- `test_build_entries_uses_catalog_for_bp_option_fixture_without_duplicate`
- `test_map_lsd_case_to_ldpc_kwargs_rejects_unsupported_decoder_mode`
- `test_map_config_to_ldpc_kwargs_rejects_unsupported_osd_variant`

The duplicate test should patch `fixture_case_paths` to return the same
cataloged parity path and assert `build_entries(..., fixture_catalog=...)`
returns one entry for `bp_product_sum_serial_sensitive`, not two.

- [ ] **Step 2: Verify Python catalog tests are red**

Run:

```bash
python3 -m pytest rbposd/scripts/test_parity_harness.py -k "lsd or bp_method"
python3 -m pytest rbposd/scripts/test_parity_harness.py -k unsupported
```

Expected before implementation: fail because `load_fixture_catalog`,
`iter_catalog_fixture_cases`, the new `fixture_catalog` argument, or the new
unsupported-mode checks do not exist.

- [ ] **Step 3: Implement catalog loading in `parity_harness.py`**

Add a CLI option:

```python
parser.add_argument(
    "--fixture-catalog",
    type=Path,
    default=Path("rbposd/tests/fixtures/catalog.json"),
    help="Shared LSD and BP-option fixture catalog.",
)
```

Replace `load_lsd_manifest` with:

```python
def load_fixture_catalog(catalog_path: Path) -> dict[str, Any]:
    with catalog_path.open("r", encoding="utf-8") as infile:
        catalog = json.load(infile)
    if not isinstance(catalog.get("fixtures"), list) or not catalog["fixtures"]:
        raise ValueError(f"Fixture catalog {catalog_path} must contain a non-empty fixtures list")
    return catalog
```

Add `validate_catalog_entry_metadata(entry: dict[str, Any]) -> None` that
checks `id`, `kind`, `decoder`, `path`, `matrix_path`, `syndrome_path`,
`provenance`, `verifier`, `pass_condition`, non-empty `consumes` containing
`#98`, and non-empty `modes`. Error messages should include
`Fixture catalog entry <id> <field> must not be empty`.

Add:

```python
def catalog_fixture_root(catalog_path: Path) -> Path:
    return catalog_path.parent
```

Update `iter_lsd_fixture_cases` to accept `catalog_path: Path`, iterate catalog
entries with `kind == "lsd"`, load `catalog_fixture_root(catalog_path) /
entry["path"]`, validate fixture `id`, and return the existing parity-shaped
LSD cases with tags `["fixture", "lsd", *entry["consumes"]]`.

Add `iter_catalog_fixture_cases(catalog_path: Path, include_lsd: bool)` that:

- returns cataloged `bp_option` parity cases by loading their case JSON from
  `fixture_root / entry["path"]`
- returns cataloged LSD cases only when `include_lsd` is true
- sets each item as `{"source": "catalog_fixture", "case_path": path_or_none, "case": case, "catalog_path": entry["path"]}`

- [ ] **Step 4: Avoid duplicate cataloged parity entries in `build_entries`**

Change `build_entries` signature to:

```python
def build_entries(
    repo_root: Path,
    fixtures_dir: Path,
    skip_generated: bool,
    case_limit: int | None,
    include_lsd: bool = False,
    fixture_catalog: Path = Path("rbposd/tests/fixtures/catalog.json"),
) -> list[dict[str, Any]]:
```

At collection time:

```python
catalog_items = iter_catalog_fixture_cases(fixture_catalog, include_lsd=include_lsd)
cataloged_fixture_paths = {
    Path(item["case_path"]).resolve()
    for item in catalog_items
    if item.get("case_path") is not None
}
case_items.extend(catalog_items)

for fixture_path in fixture_case_paths(fixtures_dir):
    if fixture_path.resolve() in cataloged_fixture_paths:
        continue
    case_items.append({"source": "fixture", "case_path": fixture_path, "case": load_case(fixture_path)})
```

Keep generated cases after checked-in fixture cases unless
`skip_generated=True`.

- [ ] **Step 5: Tighten unsupported mapping errors**

In `map_config_to_ldpc_kwargs`, add:

```python
if osd_variant not in osd_method_map:
    raise ValueError(f"Unsupported osd_variant: {osd_variant}")
```

In `map_lsd_case_to_ldpc_kwargs`, add:

```python
decoder = case.get("decoder")
if decoder not in ("bp_lsd", "bplsd"):
    raise ValueError(f"Unsupported LSD decoder mode: {decoder}")
```

Keep existing `bp_variant`, `schedule`, `early_stop`, `lsd_method`, and
`lsd_order` rejection behavior.

- [ ] **Step 6: Wire the CLI argument through `main`**

Pass `fixture_catalog=args.fixture_catalog` to `build_entries`. Keep
`--lsd-fixtures-dir` only if tests still need it for backward compatibility;
otherwise remove it and its uses.

- [ ] **Step 7: Verify Task 2 is green**

Run:

```bash
python3 -m pytest rbposd/scripts/test_parity_harness.py -k "lsd or bp_method"
python3 -m pytest rbposd/scripts/test_parity_harness.py -k unsupported
```

Expected after implementation: both commands pass.

- [ ] **Step 8: Commit Task 2**

Run:

```bash
git add rbposd/scripts/parity_harness.py rbposd/scripts/test_parity_harness.py
git commit -m "test: use rbposd fixture catalog in parity harness"
```

## Task 3: Reference Documentation and Broad Verification

**Files:**
- Modify: `rbposd/doc/ldpc_mvp_reference.md`
- Modify: `rbposd/tests/reference.rs`

**Interfaces:**
- Consumes: shared catalog contract from Tasks 1 and 2.
- Produces: documentation and reference tests that mention `catalog.json` as the shared catalog.

- [ ] **Step 1: Write failing documentation reference assertions**

In `rbposd/tests/reference.rs`, update `task_6_documentation_surfaces_exist`
required strings:

```rust
for required in [
    "BpLsdDecoder",
    "LsdConfig",
    "LsdMethod",
    "UnsupportedLsdOrder",
    "NoLsdSolution",
    "lsd_order=1",
    "lsd_small_sparse_code.json",
    "#98",
    "Shared LSD and BP-Option Fixture Catalog",
    "rbposd/tests/fixtures/catalog.json",
    "bp_product_sum_serial_sensitive.json",
    "python3 rbposd/scripts/parity_harness.py --include-lsd",
    "python3 -m pytest rbposd/scripts/test_parity_harness.py -k lsd",
] {
```

- [ ] **Step 2: Verify the doc assertion is red**

Run:

```bash
cargo test -p rbposd task_6_documentation_surfaces_exist
```

Expected before documentation update: fail because the reference document still
describes the old LSD-only manifest.

- [ ] **Step 3: Update the reference document**

In `rbposd/doc/ldpc_mvp_reference.md`, replace the `## LSD Fixture Manifest`
section with `## Shared LSD and BP-Option Fixture Catalog`. The section must
state:

- `rbposd/tests/fixtures/catalog.json` is the shared catalog.
- The catalog covers the checked-in LSD fixtures and
  `bp_product_sum_serial_sensitive.json`.
- Each catalog entry records fixture id, kind, decoder, path, matrix path,
  syndrome path, provenance, verifier, pass condition, consuming issue ids, and
  modes.
- Rust tests validate catalog coverage and reject missing provenance or
  verifier metadata.
- The Python parity harness uses the catalog for `--include-lsd` and for
  cataloged BP-option parity cases.

- [ ] **Step 4: Verify Task 3 is green**

Run:

```bash
cargo test -p rbposd task_6_documentation_surfaces_exist
```

Expected after implementation: the test passes.

- [ ] **Step 5: Run issue and finish verification**

Run:

```bash
cargo test -p rbposd fixture_catalog_manifest_covers_all_checked_in_lsd_and_bp_cases
python3 -m pytest rbposd/scripts/test_parity_harness.py -k "lsd or bp_method"
cargo test -p rbposd fixture_catalog_rejects_missing_provenance_or_verifier
python3 -m pytest rbposd/scripts/test_parity_harness.py -k unsupported
cargo test -p rbposd
python3 -m pytest rbposd/scripts/test_parity_harness.py
cargo test
git diff --check
```

Expected: all commands exit 0. Known pre-existing warnings from unrelated crates
may appear during `cargo test`; do not hide them.

- [ ] **Step 6: Commit Task 3**

Run:

```bash
git add rbposd/doc/ldpc_mvp_reference.md rbposd/tests/reference.rs
git commit -m "docs: document rbposd shared fixture catalog"
```
