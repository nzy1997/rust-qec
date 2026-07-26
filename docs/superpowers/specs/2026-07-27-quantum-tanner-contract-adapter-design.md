# Quantum Tanner Contract Adapter Design

## Context

Issue #570 asks `qec-code` to expose the existing explicit quantum-Tanner constructor through the common CSS construction contract. The current branch already has the dependency work from #553: `CssFamilySpec::QuantumTanner(QuantumTannerSpec)` routes to `quantum_tanner_css_checks`, and `code css quantum-tanner` remains as the legacy CLI path.

## Chosen Approach

Keep `QuantumTannerSpec` and the legacy constructor logic unchanged. The adapter should call `quantum_tanner_css_checks`, then use the shared `construction_result` normalization path so `H_X`, `H_Z`, statistics, orthogonality checks, and sparse-row canonicalization are common with the other family constructors.

Common metadata should be strengthened in `CssConstructionProvenance` with a deterministic normalized input digest and a source description. The digest is computed from a stable JSON payload containing the schema version, construction id, requested-family id, and normalized parameters, so equivalent typed and JSON inputs produce the same digest after parsing. The source description remains at the common layer to avoid adding public fields to `QuantumTannerSpec`.

## Alternatives Considered

1. Add provenance fields directly to `QuantumTannerSpec`. This would preserve raw fixture details such as `fixture_id`, but it would break public struct literals and mix common-contract metadata into the legacy constructor input type.
2. Rewrite the quantum-Tanner constructor around a new family API. This would expand the change surface and violate the issue instruction to preserve the existing group logic and CLI behavior.

## Data Flow

JSON construction requests are parsed by `parse_css_construction_json`. For `construction = "quantum_tanner"`, the request lowers to the existing `QuantumTannerSpec` parser and then to `CssFamilySpec::QuantumTanner`. Rust callers can construct the same enum variant directly. Both paths call `construct_css`, which dispatches to `quantum_tanner_css_checks`, canonicalizes rows, computes stats, and records provenance.

The compatibility fixture is `qec-code/tests/fixtures/quantum_tanner/toric_d4.json`. The contract test compares the common-contract output against the legacy constructor output for canonical rows and asserts the known `n = 16`, `k = 2`, distance `4`, and check weight `4`.

## Error Handling

The adapter must not wrap quantum-Tanner errors in generic construction errors after the legacy parser/constructor has produced a typed `QecError`. The negative controls use `invalid_non_symmetric_a.json` and `invalid_bad_table.json` to verify that `InvalidQuantumTannerGeneratorSet` and `InvalidQuantumTannerGroupTable` are preserved exactly through the common API.

## Testing

Add `qec-code/tests/quantum_tanner_contract.rs` with focused tests named in the issue verification commands:

- `quantum_tanner_toric_d4_matches_legacy_constructor`
- `quantum_tanner_contract_preserves_typed_errors`

Run the two exact tests, the existing quantum-Tanner test selection, and the repository-level `cargo test` before opening the pull request.
