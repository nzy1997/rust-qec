# Issue 412 Integer-Threshold Noise Masks Design

## Context

`FrameSimulator` currently constructs simple Bernoulli noise masks by comparing one generated `f64` per shot bit. `DEPOLARIZE1` and `DEPOLARIZE2` also perform their own event selection loops with `rng.gen::<f64>() < p` before choosing a Pauli. Issue #412 asks for a shared integer-threshold event mask path so low-probability noise in the selected d11/r100 fixture no longer pays a floating-point compare in each event-selection loop.

Issue #409 is satisfied by merged PR #418, which confirms the selected fixture expands to 88,000 `DEPOLARIZE2` targets.

## Approaches Considered

1. **Shared integer-threshold Bernoulli helper.** Convert `p` to a `u64` threshold over the full `u64` RNG range, fill a mask by comparing `rng.gen::<u64>() < threshold`, and route all simple Bernoulli masks through the helper. `DEPOLARIZE1` and `DEPOLARIZE2` consume the mask and only choose Paulis for set event bits. This is the selected approach because it is scoped, testable, and preserves the depolarizing Pauli choice distribution.
2. **Only optimize `random_bits_with_prob`.** This improves `X_ERROR`, `Y_ERROR`, `Z_ERROR`, correlated errors, and heralded masks, but it leaves depolarizing event selection on the old floating path. This misses the issue's main fixture hotspot.
3. **Rewrite depolarizing channels as full channel samplers.** This could combine event selection and Pauli choice into one draw per bit, but it changes random-consumption structure more broadly and risks changing Pauli semantics beyond the requested event-mask change.

## Design

Add a helper in `rstim/src/sim/frame.rs` that takes the word count, valid shot bit count, probability, and RNG, then returns a `Vec<u64>` mask. For `p <= 0` it returns all zero words. For `p >= 1` it returns all valid shot bits set. For `0 < p < 1`, it computes an integer threshold from `p * 2^64` and sets each valid bit when a generated `u64` is below the threshold.

The helper will mask the final partial word according to `FrameSimulator::batch_size`, so returned `BitTable` rows do not expose out-of-range shot bits from noise masks when the batch size is not a multiple of 64.

`X_ERROR`, `Y_ERROR`, `Z_ERROR`, `CORRELATED_ERROR`, `ELSE_CORRELATED_ERROR`, `HERALDED_ERASE`, and the herald mask in `HERALDED_PAULI_CHANNEL_1` continue to call the helper. `DEPOLARIZE1` and `DEPOLARIZE2` will call the same helper for event selection, then iterate over set event bits to run the existing uniform Pauli choice logic. The Pauli choice remains `0..3` for one-qubit depolarizing errors and `0..15` mapped through `two_qubit_pauli` for two-qubit depolarizing errors.

`PAULI_CHANNEL_1`, `PAULI_CHANNEL_2`, and the Pauli-selection step inside `HERALDED_PAULI_CHANNEL_1` keep their existing floating cumulative-distribution draws because they are not simple Bernoulli event selection.

## Testing

Add `rstim/tests/frame_noise_masks.rs` with the issue's expected test names. The tests cover exact empty masks for `p <= 0`, exact valid-shot all-ones behavior for `p >= 1`, low-probability frequency bounds, seeded reproducibility, and source-level guards proving the `DEPOLARIZE1` and `DEPOLARIZE2` match arms route through `random_bits_with_prob` without the old `rng.gen::<f64>() < p` event loop.

Focused verification is:

```sh
cargo test -p rstim --test frame_noise_masks
```

Final verification is:

```sh
cargo test
```

## Out of Scope

- Do not change depolarizing Pauli selection semantics beyond consuming the improved event mask.
- Do not optimize multi-outcome Pauli channels.
- Do not add timing thresholds or benchmark gates.
