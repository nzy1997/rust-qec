# Issue 231 Random-Window Upper-Bound Contract Design

Date: 2026-06-25
Status: Accepted automatically under the Agent Desk Standing Answer Policy
Scope: `qec-code` distance-bound result/options contract for `random-window-upper-bound`

## Summary

Issue #231 adds a typed result/options contract for completed
`random-window-upper-bound` runs without implementing the search algorithm.
The JSON result must remain a completed upper-bound distance result with the
same witness, logical-class, options, and provenance shape used by the existing
`randomized-upper-bound` contract, while serializing the method label as
`random-window-upper-bound`.

The implementation keeps `DistanceBoundResult` as the shared result family and
makes the options type generic. Existing randomized callers continue to use
`DistanceBoundResult` with its default `RandomizedUpperBoundOptions`, while
random-window contract tests use `DistanceBoundResult<RandomWindowUpperBoundOptions>`.

## Context

Issue #229 is merged and introduced a method-aware distance-bound validator in
`qec-code/src/distance_bound.rs`. That validator already checks the method
label, `bound_type == upper`, upper-bound and witness weight consistency,
non-identity witness support, stabilizer commutation, stabilizer span exclusion,
logical class, and optional known exact-distance lower safety.

The current result type still stores `RandomizedUpperBoundOptions` directly and
`DistanceBoundResult::completed` still hard-codes
`DistanceBoundMethod::RandomizedUpperBound`. That is safe for the existing
baseline, but it is too easy for a future random-window implementation to reuse
the constructor and silently emit the wrong method label.

## Design

Add a distinct `RandomWindowUpperBoundOptions` struct with the same serialized
fields as the randomized options:

- `iterations`
- `restarts`
- `seed`
- optional `target_weight`

Both option structs validate through the same small helper so their zero-value
rejection messages remain identical.

Make `DistanceBoundResult` generic:

```rust
pub struct DistanceBoundResult<Options = RandomizedUpperBoundOptions> {
    pub status: DistanceBoundStatus,
    pub method: DistanceBoundMethod,
    pub bound_type: BoundType,
    pub upper_bound: usize,
    pub logical_class: LogicalClass,
    pub witness: DistanceBoundWitness,
    pub options: Options,
    pub provenance: DistanceBoundProvenance,
}
```

The default type parameter preserves existing randomized API usage. Add a
private method-aware constructor helper and public method-specific constructors:

- `DistanceBoundResult::completed(...)` remains the randomized constructor.
- `DistanceBoundResult::<RandomWindowUpperBoundOptions>::completed_random_window_upper_bound(...)`
  creates a completed random-window upper-bound result.

Generalize shared validation over an options trait so both result flavors can
reuse the method-aware validator. Keep `validate_randomized_upper_bound_result`
unchanged for existing callers and add
`validate_random_window_upper_bound_result` for the new method. Both wrappers
must reject the other method label with an error naming the expected method.

## Alternatives Considered

### 1. Reuse `RandomizedUpperBoundOptions`

This is the smallest code change, but it does not add a typed random-window
options contract and leaves too much coupling between the two methods.

### 2. Use an untagged options enum inside `DistanceBoundResult`

An untagged enum would keep the JSON field flat, but the randomized and
random-window option objects have identical shapes. Deserializing JSON would
pick the first matching variant, which makes method/options validation
ambiguous.

### 3. Generic `DistanceBoundResult<Options>`

This is the chosen approach. It preserves the existing public result name,
keeps JSON unchanged, allows method-specific option types, and lets validators
remain method-aware without duplicating witness checks.

## Error Contract

Validation errors continue to use `QecError::DistanceBoundValidationFailed`
for result-contract failures. The random-window validator must reject a result
whose method label is `randomized-upper-bound` with:

```text
expected method random-window-upper-bound, got randomized-upper-bound
```

The existing randomized validator must continue to reject a result whose method
label is `random-window-upper-bound` with:

```text
expected method randomized-upper-bound, got random-window-upper-bound
```

Witness validation messages remain method-independent.

## Testing

Add focused tests to `qec-code/tests/distance_bound.rs`:

- `random_window_upper_bound_result_serializes_contract` creates a completed
  random-window result and asserts the serialized JSON has
  `status = completed`, `method = random-window-upper-bound`,
  `bound_type = upper`, `upper_bound` equal to the witness weight, the four
  option fields, and `provenance.tool = qec-code`.
- `random_window_upper_bound_validator_rejects_wrong_method_label` mutates a
  valid random-window result to `method = randomized-upper-bound` and asserts
  the random-window validator rejects it with the expected-method message.
- Add or extend a randomized validator negative control so a randomized result
  labeled `random-window-upper-bound` is still rejected by the existing
  randomized validator.

Run the issue-required focused commands and then `cargo test`.

## Out Of Scope

Do not implement the random-window search algorithm. Do not add CLI flags or
commands. Do not change exact distance output. Do not alias
`randomized-upper-bound` to `random-window-upper-bound`.
