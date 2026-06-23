# APM Commutation Checks Design

Scope: GitHub issue #136, internal `qec-code` helpers for checked affine
permutation commutation and explicit APM manifest pair constraints.

## Context

Issue #132 added the Table A1 APM manifest under
`qec-code/tests/fixtures/apm/table_a1_manifest.json`. Issue #134 documented
the APM-CSS contract in `qec-code/doc/apm_css.md`, including the residual
condition for affine maps. Issue #135 added the crate-private
`AffinePermutation` type in `qec-code/src/codes/apm.rs`.

This issue should turn the documented residual into the crate's reusable
commutation primitive. It must not generate Delta/Gamma sets, sweep all Gamma
pairs, build sparse matrices, or compute graph girth.

## Chosen Approach

Add the helper API inside `qec-code/src/codes/apm.rs` next to
`AffinePermutation`. Keep it `pub(crate)` so later `qec-code` APM work can use
it without exposing a public crate API.

The selected API shape is:

- `AffinePermutation::commutation_residual(&self, other) -> Result<u64, AffinePermutationError>`
- `AffinePermutation::commutes_with(&self, other) -> Result<bool, AffinePermutationError>`
- `AffineCommutationExpectation` with `Commutes` and `DoesNotCommute`
- `AffineCommutationCheck<'a>` carrying code id, pair labels, maps, and expected behavior
- `validate_affine_commutation_checks(checks) -> Result<(), Vec<AffineCommutationError>>`

The residual helper returns a `Result` so modulus mismatches are checked at the
same boundary as composition. The validator collects all unexpected pair
outcomes into structured errors that include the code id and both pair labels.

## Alternatives Considered

1. Put the validator only in integration tests.

   This would prove the manifest today but leave future APM generation code
   without a production primitive. It also preserves the duplicate documented
   residual currently in `qec-code/tests/code.rs`.

2. Expose a public APM module from `qec-code`.

   This would make the helper easier for integration tests to call, but #135
   intentionally kept APM algebra crate-private until the native generator API
   exists. A public module would create compatibility surface too early.

3. Add crate-private helpers and test them with module unit tests.

   This is the chosen approach. It follows #135, keeps the helper test-visible
   inside `qec-code`, and lets the requested test name run with
   `cargo test -p qec-code affine_commutation_matches_table_a1 -q`.

## Data Flow

Callers construct validated `AffinePermutation` values over the intended
modulus. For manifest `required_commuting_pairs`, the test resolves labels such
as `column_component:f0` and constructs maps over the pair's documented
`modulus`. For `required_noncommuting_pairs`, the test constructs `f[i]` and
`g[j]` over the full manifest `P`.

`commutation_residual` computes:

```text
(a*d + b - c*b - d) mod M
```

for maps `f(x)=a*x+b` and `g(x)=c*x+d`. The maps commute iff the residual is
zero. The unit test also compares this residual result against direct sampled
composition over every `x` in the modulus for the checked P=96 pairs.

## Error Handling

The residual and boolean helper return `AffinePermutationError::ModulusMismatch`
for maps over different moduli.

The validator returns `Vec<AffineCommutationError>` so a caller can see every
bad explicit pair in one pass. The variants are:

- `ModulusMismatch { code_id, left_label, right_label, lhs, rhs }`
- `UnexpectedCommutation { code_id, left_label, right_label, residual }`
- `UnexpectedNoncommutation { code_id, left_label, right_label, residual }`

Display messages include the code id and pair labels, which satisfies the
negative-control requirement.

## Testing

Add module tests in `qec-code/src/codes/apm.rs`:

- `affine_commutation_matches_table_a1` loads the P=96 manifest entry, checks
  each required commuting pair as true, checks each required noncommuting pair
  as false, and confirms residual-based commutation agrees with direct
  composition over every `x` in the chosen modulus.
- The same test mutates validator input by listing a known noncommuting pair as
  required commuting and asserts the structured error names the pair and code
  id.
- `affine_commutation_rejects_modulus_mismatch` covers the primitive and
  validator mismatch paths.

Expected focused verification:

```sh
cargo test -p qec-code affine_commutation_matches_table_a1 -q
```

Expected broad verification:

```sh
cargo test
```
