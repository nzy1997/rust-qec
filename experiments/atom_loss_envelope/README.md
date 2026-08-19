# Atom-loss Envelope-MLE experiment

This unpublished crate is a deterministic correctness reference for decoding an
explicit set of Pauli effects compatible with observed atom losses. It follows
the detector-parity and per-loss exclusivity reduction used by Envelope-MLE in
the Pauli Envelope framework, but does not claim to reproduce the paper's
circuits or numerical results.

The input is intentionally offline. A caller supplies independent Pauli effects
and one non-empty candidate list for each observed loss. Detector and observable
index lists are interpreted modulo two. Candidate `weight` values are additive
non-negative objective costs; they can encode a chosen negative-log prior, but
this v0 schema does not prescribe how those priors are calibrated.

Run the checked positive case with the open-source HiGHS backend:

```sh
cargo run -q -p atom-loss-envelope -- \
  decode \
  --in experiments/atom_loss_envelope/cases/single_loss_observable.json \
  --out /tmp/atom-loss-envelope-result.json \
  --backend highs
```

The output uses `atom-loss-envelope-result.v0`. An optimal solve exits `0`.
A valid but infeasible model exits `3` and still writes a result with status
`infeasible`. Schema and validation failures exit `1` without writing a result.

## Scope boundary

This crate does not yet:

- derive envelope candidates from an `rstim` circuit or a live loss event;
- implement the paper's Mid-SWAP syndrome-extraction circuit;
- widen the stable DEM-only `rsinter::Decoder` interface; or
- run threshold, effective-distance, or publication-scale studies.

Those layers can be added in separate PRs after this explicit-input kernel is
stable. The Python/Gurobi oracle in the companion `auto-decoder` research tree
was used as a semantic cross-check only; no code or sampled pattern data is
copied from that unlicensed tree.

## Envelope-Matching approximation

The `matching` subcommand accepts an explicit MWPM graph, stable loss-to-edge
memberships, and one or more shots. For every distinct observed-loss set it
builds one `rmatching` graph, assigns affected time-like edges `0.25` times the
global mean base weight, assigns affected space-like or boundary edges `0.5`
times that mean, and decodes the corresponding syndromes as a batch.

```sh
cargo run -q -p atom-loss-envelope -- matching \
  --in experiments/atom_loss_envelope/cases/matching_known_answer.json \
  --out /tmp/envelope-matching-result.json
```

The result uses `atom-loss-envelope-matching-result.v0`. Each entry in
`predictions` is a bit mask: bit `i` is the predicted value of logical
observable `i`. The current format supports up to 64 observables.

This command requires explicit, pre-calibrated loss-to-edge memberships. It
does not infer them from a circuit and does not make a SOTA performance or
logical-error-rate claim.

The input fields are:

- `num_detectors` and `num_observables` (`0..=64` observables);
- `edges`, each with a unique `id`, detector `node1`, optional `node2`,
  observable indices, non-negative finite base `weight`, and `kind`;
- `loss_edge_map`, containing stable loss IDs and their affected edge IDs; and
- one or more `shots`, each listing observed detectors and loss IDs.

An edge has `kind: "boundary"` exactly when `node2` is `null`; internal edges
are `time_like` or `space_like`. IDs and indices must resolve within the same
document. A shot is rejected when an odd number of its detection events falls
in a graph component with no boundary, because that syndrome has no perfect
matching. Validation failures exit `1`, print an actionable message to stderr,
and do not write a result document.
