# Issue 321 QEC-Code Random-Window Manifests Design

## Context

Issue #321 asks for benchmark case manifests for the existing
`qec-code code css-distance random-window-upper-bound` command. The manifests
are an evidence path for local wall-clock time and returned upper bounds. They
do not reimplement codeDistancePYPI, QDistEvol, QDistRndMW, m4ri, Gurobi, SAT,
or any other external baseline.

The repository already has issue-225 ladder evidence for random-window upper
bounds and a built-in CSS code interface with `steane`, `surface_rotated`,
`toric`, fixed `bb72`, parameterized bivariate-bicycle IDs, and APM Kasai IDs.
This change adds benchmark-facing TOML manifests plus a Python validator that a
future runner can consume.

## Approaches Considered

1. Add static TOML manifests and a small Python validator.
   - Pros: matches the requested future interface exactly, keeps benchmark
     metadata reviewable, and avoids changing the Rust command.
   - Cons: code-id existence is not checked by Python without shelling out to
     Cargo.

2. Generate TOML from Rust issue-225 fixtures.
   - Pros: reduces duplication for the existing ladder cases.
   - Cons: makes benchmark manifests less directly reviewable and couples this
     evidence contract to Rust integration-test internals.

3. Add a Rust-side manifest validator.
   - Pros: could dispatch `built_in_css_checks` directly.
   - Cons: the requested interface is `python3 -m
     benchmarks.qec_code_random_window.validate_cases <manifest>`, so a Rust
     validator would add an extra wrapper without clear benefit.

Chosen approach: static TOML manifests plus a focused Python standard-library
validator.

## Design

Create `benchmarks/qec_code_random_window/` as a Python package containing:

- `cases.smoke.toml`
- `cases.full.toml`
- `validate_cases.py`
- `tests/test_validate_cases.py`
- invalid test fixtures for duplicate IDs and missing strict baseline keys

Each manifest has a top-level `manifest_version = 1`, `suite =
"qec_code_random_window"`, and a `[[cases]]` array. Each case includes:

- `case_id`
- `code_id`
- `distance_side`
- `iterations`
- `restarts`
- `seed`
- `target_weight`
- `target_upper_bound`
- `baseline_key`
- `baseline_required`

The validator checks shape and types, rejects duplicate `case_id` values, and
enforces the explicit baseline contract. A case with `baseline_required = true`
must have a usable `baseline_key`; empty strings, `none`, and `unmapped:*` are
not usable strict baseline keys. This keeps future `--strict-baselines`
behavior explicit.

## Case Selection

Smoke cases are fast local checks:

- `steane`
- `surface_rotated:d=3`
- `toric:d=3`
- `bb72`

Full cases use the same schema with larger representative code IDs:

- `steane`
- `surface_rotated:d=5`
- `toric:d=5`
- `bb72`
- the issue-225 `bb144` bivariate-bicycle parameterization

`bb144` is the one larger BB/APM-style case because it is already available
through the current `qec-code` code-id interface. APM cases are not included in
this issue because no defensible codeDistancePYPI paper-baseline row is known
from the issue context, and random-window smoke/full budget choices for those
larger matrices would be speculative.

## Baseline Contract

Use `baseline_required = false` for Steane, rotated surface, and toric cases.
They are locally useful smoke/full cases, but the issue context does not provide
defensible codeDistancePYPI paper-row mappings for those exact CSS code IDs.

Use `baseline_required = true` only for `bb72` and `bb144`, with baseline keys
under `codeDistancePYPI:bivariate_bicycle:*`. These local code IDs match the
existing bivariate-bicycle CSS fixtures and target upper bounds already used by
the issue-225 ladder evidence. The later #324/#325 strict-baseline comparison
can resolve those keys to static paper data.

## Error Handling

The validator exits nonzero on invalid input, prints one error per line to
stderr, and includes the case ID or field path in each message. It exits 0 and
prints exactly `PASS` on valid input.

## Testing

Focused validation commands:

- `python3 -m unittest benchmarks.qec_code_random_window.tests.test_validate_cases -q`
- `python3 -m benchmarks.qec_code_random_window.validate_cases benchmarks/qec_code_random_window/cases.smoke.toml`
- `python3 -m benchmarks.qec_code_random_window.validate_cases benchmarks/qec_code_random_window/tests/fixtures/duplicate_case_id.toml`
- `python3 -m benchmarks.qec_code_random_window.validate_cases benchmarks/qec_code_random_window/tests/fixtures/strict_baseline_missing_key.toml`

Required repository checks:

- `cargo run -q -p qec-code -- code css-distance random-window-upper-bound --help`
- `cargo test`

Cargo may need `CARGO_NET_OFFLINE=true` in this sandbox because network access
to crates.io is blocked, but the required plain commands should still be
attempted and reported.
