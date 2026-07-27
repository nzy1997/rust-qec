# Issue 574 Family CLI Verifier Showcase Design

Date: 2026-07-27
Status: Approved by non-interactive Agent Desk standing policy
Scope: GitHub issue #574, Roadmap ID M4-04

## Summary

Issue #574 adds one offline command:

```text
qec-code code css verify-families
```

The command demonstrates the full requested-family catalog after #573. It emits
one deterministic line per manifest family, reports supported available families
as `PASS`, reports `hyperbolic_5_5` and `perturbed_hgp` as `DEFERRED`, and
prints the exact successful summary:

```text
SUMMARY PASS supported=12 deferred=2 failed=0
```

The implementation promotes the #573 catalog parsing and executable positive
case checks into production library code. The CLI uses that code in-process and
reads the checked-in manifest fixture from the source tree. Tests also call the
same verifier entry point with mutated manifest text for deterministic negative
controls.

The issue references `docs/design/2026-07-26-qec-code-family-support.md`, but
that file is not present in this checkout. The local binding context is the
issue body, `qec-code/tests/fixtures/family_manifest/manifest.v1.json`, the
manifest README, `qec-code/src/family_contract.rs`, and the checked-in
Superpowers specs/plans for issues #552 and #573.

## Goals

- Add `code css verify-families` without changing existing `code css list`,
  export, construct, or quantum-tanner behavior.
- Read the checked-in family manifest and execute the positive fixture for each
  available supported family through `parse_css_construction_json` and
  `construct_css`.
- Verify requested-family ID, expected `n`, `m_x`, `m_z`, `rank_x`, `rank_z`,
  `k`, optional distances, row-weight summaries, orthogonality, and provenance
  source.
- Print one byte-stable line per family in manifest order.
- Print `DEFERRED` lines for `hyperbolic_5_5` and `perturbed_hgp` with tracking
  issue and research contract path.
- Treat a supported manifest entry that is not `availability=available` as a
  catalog inconsistency, with a deterministic `FAIL` line.
- Return a nonzero process status for any construction error, fixture mismatch,
  orthogonality error, parse error, malformed deferred metadata, or supported
  status mismatch, while still printing the report.
- Update the showcase documentation with the verifier command, representative
  Rust and CLI construction examples, output interpretation, deferred-family
  boundaries, and fixture-addition guidance.
- Keep the showcase document checker covering the updated page.

## Non-Goals

- Do not implement `hyperbolic_5_5` or `perturbed_hgp`.
- Do not add callable aliases, public constructors, or CLI routes for deferred
  families.
- Do not add network access, subprocess execution, or external solver calls to
  the verifier.
- Do not turn negative executable cases into CLI output. They remain covered by
  the existing family catalog completeness test.
- Do not change the sparse-row JSON output format for existing CSS export
  commands.

## Approaches Considered

### 1. Production verifier module over the manifest fixture

Move the reusable subset of the #573 test-owned manifest schema into a
production module, read `qec-code/tests/fixtures/family_manifest/manifest.v1.json`
at runtime, and expose `verify_checked_in_family_manifest()` plus
`verify_family_manifest_text(text)` for tests.

Benefits:

- one in-process code path for CLI and tests
- negative controls can mutate manifest text without mutating the checkout
- production CLI reports `FAIL` lines while returning nonzero
- no shell-outs or network access
- existing manifest remains the source of truth

Costs:

- production code references the repository fixture path through
  `env!("CARGO_MANIFEST_DIR")`, so the command is primarily a repository
  verifier rather than a standalone installed-binary feature

This is the selected approach.

### 2. Keep all verifier logic in CLI code

Implement parsing, checking, and formatting directly in `qec-code/src/cli.rs`.

Benefits:

- fewer new modules

Costs:

- grows the CLI dispatcher with catalog-specific validation
- makes negative-control tests less direct
- duplicates the shape of existing catalog validation logic in a less reusable
  place

This is not selected.

### 3. Run existing tests from the command

Have `verify-families` shell out to `cargo test -p qec-code --test
family_catalog`.

Benefits:

- quick implementation

Costs:

- explicitly violates the requirement that the verifier runs in-process and
  offline without shelling out
- output would not be the required one-line-per-family transcript

This is not selected.

## Output Contract

Successful available-family lines use compact, deterministic fields:

```text
PASS <family_id> params=<compact-json> n=<n> checks=h_x:<m_x>,h_z:<m_z> ranks=rank_x:<rank_x>,rank_z:<rank_z> k=<k> row_weights=h_x:[w<weight>=<count>,...],h_z:[w<weight>=<count>,...] orthogonal=true provenance=<source>@<digest>
```

The compact JSON comes from the constructor's `BTreeMap` normalized parameters.
Row-weight summaries are sorted by row weight and formatted as
`w<weight>=<count>`. The provenance identifier combines
`CssConstructionProvenance.source` and `normalized_input_digest`.

Deferred lines use:

```text
DEFERRED hyperbolic_5_5 tracking_issue=#571 contract=qec-code/doc/hyperbolic_5_5_contract.md
DEFERRED perturbed_hgp tracking_issue=#572 contract=qec-code/doc/perturbed_hgp_contract.md
```

Failure lines start with `FAIL <family_id>` and include the first deterministic
reason. The required negative-control reasons are:

```text
FAIL generalized_bicycle expected rank_x=5 actual rank_x=4
FAIL generalized_bicycle disposition=supported availability=planned expected=available
```

The summary counts dispositions from the manifest:

```text
SUMMARY PASS supported=12 deferred=2 failed=0
SUMMARY FAIL supported=12 deferred=2 failed=1
```

## Implementation Shape

Add `qec-code/src/family_verifier.rs` with:

- manifest schema structs for the fields used by the verifier
- `FamilyVerificationReport { output: String, failed: usize }`
- `verify_checked_in_family_manifest() -> Result<FamilyVerificationReport, QecError>`
- `verify_family_manifest_text(text: &str) -> FamilyVerificationReport`
- helpers for available-family execution, metadata comparison, deferred-line
  formatting, and deterministic row-weight summaries

Expose the module from `qec-code/src/lib.rs`.

Add `CssCommands::VerifyFamilies` in `qec-code/src/cli.rs`. On success,
`run()` returns the report text. On verifier failure, `run()` returns a new
`QecError::FamilyVerificationFailed { report }`.

Change `qec-code/src/main.rs` only enough to special-case
`FamilyVerificationFailed`: write the embedded report to stdout and exit `1`.
All other errors continue to print to stderr exactly as before.

Add `qec-code/tests/family_cli.rs` with the three issue-required exact tests.
The success test runs the binary command and asserts 12 `PASS` lines, 2
`DEFERRED` lines, the exact summary, empty stderr, and required line fields.
The mutation tests call `verify_family_manifest_text` with edited manifest JSON
and assert deterministic failure text, failure count, and summary.

Update `docs/showcases/qec-code-css-construction.md` and rely on
`tools/check_showcase_docs.py` to enforce the showcase page structure and links.

## Testing

Required verification:

```text
cargo test -p qec-code --test family_cli verify_families_cli_reports_12_pass_and_2_deferred -- --exact
cargo test -p qec-code --test family_cli verify_families_cli_fails_on_mutated_rank -- --exact
cargo test -p qec-code --test family_cli verify_families_cli_fails_when_supported_target_is_planned -- --exact
cargo run -p qec-code -- code css verify-families
python3 tools/check_showcase_docs.py docs/showcases/qec-code-css-construction.md
cargo test
```
