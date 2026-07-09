# Issue 427 Sparse Bernoulli Frame Noise Mask Design

## Context

`FrameSimulator` currently routes simple frame noise, correlated errors, and the
event-selection step for `DEPOLARIZE1` and `DEPOLARIZE2` through
`random_bits_with_prob_into` in `rstim/src/sim/frame.rs`. Issue #412 replaced
the older per-bit floating-point test with an integer-threshold dense path, and
issue #413 reused depolarizing scratch buffers. The remaining low-probability
hotspot is that the dense helper still draws one RNG value for every valid shot
bit.

The selected release smoke fixture uses many `p = 0.001` frame-noise
operations, so the mask generator should avoid visiting every shot bit when the
expected number of set bits is small. Public CLI output, Stim compatibility,
checked benchmark artifacts, and depolarizing Pauli selection semantics must not
change.

## Approaches Considered

1. Add a sparse Bernoulli branch inside `random_bits_with_prob_into`. This is
   the recommended approach because all existing frame-noise callers already
   share that helper, including depolarizing event masks, and the change stays
   private to `rstim/src/sim/frame.rs`.
2. Add sparse logic only to the `X_ERROR`/`Y_ERROR`/`Z_ERROR` match arms. This
   would help simple noise but miss correlated errors and depolarizing event
   masks, so it would leave a significant part of the fixture on the dense path.
3. Replace the helper with a vectorized external distribution. This could be
   clean eventually, but it adds a dependency and makes deterministic RNG
   consumption harder to reason about for this focused issue.

The design uses option 1.

## Design

Keep the existing exact fast paths:

- `p <= 0.0` clears the output and returns.
- `p >= 1.0` fills the output with ones and masks unused trailing bits.
- probabilities whose integer threshold rounds to zero produce an empty mask.

For intermediate probabilities, split path selection by a private constant:

```rust
const SPARSE_BERNOULLI_MAX_PROBABILITY: f64 = 0.02;
```

When `p <= SPARSE_BERNOULLI_MAX_PROBABILITY`, use a sparse helper that samples
geometric skips. The helper computes `log1p(-p)` once, repeatedly draws a
uniform `f64` in `(0, 1)`, converts it to the number of failures before the next
success with `floor(ln(u) / ln(1 - p))`, and sets only the resulting valid shot
bit. It advances by `skip + 1` after each event and stops at `valid_bits`.
Because all indices are checked against `valid_bits`, unused tail bits remain
zero without a separate tail-mask pass.

When `p > SPARSE_BERNOULLI_MAX_PROBABILITY`, keep the existing dense
integer-threshold loop in a separate helper. This preserves the current medium
and high probability behavior where geometric jumping is not expected to win.

The sparse helper may change exact seeded masks compared with the old dense
helper because it consumes different RNG draws. The required determinism
contract is that a seeded RNG produces reproducible output for the same code
path and inputs.

## Testing

Extend `rstim/tests/frame_noise_masks.rs`:

- Keep the exact `p = 0` and `p = 1` tests, including partial-word tail masking.
- Keep `low_probability_noise_mask_has_expected_frequency_bounds`, and use a
  large enough batch that an all-zero sparse implementation fails.
- Keep `noise_mask_is_reproducible_for_seeded_rng`.
- Add `low_probability_noise_mask_uses_sparse_path`, using a counting RNG around
  `StdRng` to run `X_ERROR(0.001)` and assert far fewer RNG core calls than the
  dense per-bit path would use while still observing nonzero events.
- Add `medium_probability_noise_mask_keeps_dense_path`, using the same counting
  RNG to run `X_ERROR(0.3)` and assert the helper consumes at least one RNG
  value per valid shot bit.
- Keep source-level checks that `DEPOLARIZE1` and `DEPOLARIZE2` event selection
  remains routed through `random_bits_with_prob`.

Verification commands:

```sh
cargo test -p rstim --test frame_noise_masks
cargo test
```

After the code passes, run the issue's release smoke command and record the
printed rates without turning the wall-clock ratio into a hard regression gate.

## Scope

This change is limited to frame-simulator mask generation and focused tests. It
does not change CLI formats, checked benchmark artifacts, site metadata, QP101
rendering, or public Stim compatibility claims.

## Self-Review

- No placeholder requirements remain.
- The selected approach covers every existing caller of `random_bits_with_prob`,
  including depolarizing event masks from #413.
- Path-selection coverage includes a runtime RNG-call negative control, so a
  dense-only low-probability implementation fails.
- Tail bits remain guarded by valid-bit indexing and the existing `p >= 1`
  exact mask test.
