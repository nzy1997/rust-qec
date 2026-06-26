# Issue 278 ldpc-compatible OSD channel-prior scoring design

Issue: #278 Score ldpc-compatible OSD candidates with channel-prior weights

Date: 2026-06-26

## Context

Issues #276 and #277 are merged. `rbposd` now has a documented
`ldpc`-compatible OSD-CS candidate planner and an explicit
`OsdVariant::LdpcCombinationSweep` selector. The current implementation still
passes BP posterior reliability into candidate comparison for both the legacy
Rust frontier sweep and the new `ldpc`-compatible planner.

Issue #278 narrows the remaining semantic gap: the `ldpc`-compatible mode must
rank OSD candidates with channel-prior objective weights, while the legacy Rust
path must keep its BP-reliability scoring behavior until a separate migration
decision is made.

## Automatic Answers

This Agent Desk run is non-interactive, so the required brainstorming review
gates use the standing answer policy:

- No visual companion is needed because the work is decoder API and test
  behavior, not visual design.
- The design is approved from the issue text, the merged #276 contract, and the
  merged #277 selector implementation.
- Use channel-prior LLR magnitudes as the objective weights. This is the
  safest contract because existing `ChannelModel` validation already produces
  finite prior LLRs for `Bsc` and `BitFlipProbabilities`, and the same weights
  match the log-likelihood objective implied by bit-flip probabilities.
- Keep BP posterior reliability as the ordering input for the reduced OSD
  system in both modes. Issue #276 explicitly separates candidate ordering from
  candidate scoring, and #278 only asks to change scoring in `ldpc` mode.

## Approaches Considered

1. Add explicit OSD objective weights and pass channel-prior weights only for
   `OsdVariant::LdpcCombinationSweep`.
   This is recommended because it changes the smallest behavior surface, keeps
   all planner routing explicit, and preserves existing legacy fixtures.
2. Replace the `reliability` argument with channel-prior weights everywhere.
   This is simpler but changes legacy Rust behavior and contradicts the issue's
   preservation requirement.
3. Reorder the `ldpc` planner by channel-prior weights as well as scoring with
   them.
   This may be a future compatibility topic, but #276 allows ordering to follow
   BP soft information and #278 targets only candidate ranking.

## Design

`BpCore` will expose a crate-private `channel_prior_objective_weights()` view
derived from the stored prior LLR vector. The weights are `abs(prior_llr)` for
each bit. `compute_prior_llrs` already validates all public `ChannelModel`
variants as finite probabilities strictly between 0 and 1, so both `Bsc` and
`BitFlipProbabilities` produce finite objective weights. If a future channel
variant or internal path creates a non-finite objective weight, `BpCore`
construction will reject it with a clear `DecodeError` instead of letting OSD
comparison hit `NaN` ordering.

`rbposd/src/osd.rs` will keep BP posterior reliability as the column-ordering
input. Candidate comparison will receive a separate objective-weight slice.
Legacy OSD modes will pass the same BP reliability slice for both ordering and
scoring, preserving the current behavior. `LdpcCombinationSweep` will pass BP
reliability for ordering and channel-prior objective weights for scoring.

The scoring implementation will stay a simple additive objective over true
correction bits, with the existing lexicographic tie-breaker. The helper will
validate that the objective-weight slice has the bit dimension and contains only
finite values before any candidate enumeration. This keeps error handling
deterministic and avoids panics or arbitrary ordering.

No public config field is added. The mode selector added in #277 is already the
public switch: selecting `OsdVariant::LdpcCombinationSweep` changes the scoring
objective; leaving the legacy/default path unchanged keeps BP-reliability
scoring.

## Testing

Add a focused `rbposd/tests/osd.rs` fixture where BP posterior reliability and
channel-prior weights rank two valid OSD corrections differently:

- `ldpc_osd_cs_uses_channel_prior_candidate_weight` constructs a small
  parity-check matrix, channel, and syndrome that force the OSD path after one
  BP iteration. The `ldpc` mode must choose the correction with lower
  channel-prior objective cost and still satisfy the syndrome.
- `legacy_osd_candidate_scoring_keeps_existing_reliability_behavior` reuses the
  same fixture under the legacy Rust planner. It must choose the correction
  favored by BP-reliability scoring, proving that changing only the mode
  selector changes the chosen candidate.
- The negative control also checks invalid channel probabilities for the same
  `ldpc` mode are rejected through `DecodeError::InvalidProbability`, covering
  missing/unsupported probability data at the current public `ChannelModel`
  boundary.

Regression checks:

```bash
cargo test -p rbposd ldpc_osd_cs_uses_channel_prior_candidate_weight -- --nocapture
cargo test -p rbposd legacy_osd_candidate_scoring_keeps_existing_reliability_behavior -q
cargo test
```
