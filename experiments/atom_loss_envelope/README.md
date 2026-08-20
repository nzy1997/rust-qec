# Atom-loss Envelope-MLE experiment

This unpublished crate is a deterministic correctness reference for decoding an
explicit set of Pauli effects compatible with observed atom losses. It follows
the detector-parity and per-loss exclusivity reduction used by Envelope-MLE in
the Pauli Envelope framework, but does not claim to reproduce the paper's
circuits or numerical results.

The decoders remain intentionally offline. Inputs may be supplied directly as
versioned JSON, or prepared from an RStim Mid-SWAP circuit and b8 measurement
rows with the `prepare` command. Detector and observable index lists are
interpreted modulo two. Candidate `weight` values are additive non-negative
objective costs.

## Prepare circuit-bound loss data

For ordinary circuits, `rstim analyze_errors` produces the DEM and `rstim m2d`
converts measurements into detector rows. `prepare` is the loss-aware equivalent
of those two steps: it additionally learns per-readout loss patterns from a
pure-loss calibration file.

```sh
cargo run -q -p atom-loss-envelope -- prepare \
  --circuit midswap.stim \
  --calibration_in pure-loss-calibration.b8 \
  --calibration_shots 10000 \
  --in target-measurements.b8 \
  --shots 1000 \
  --out /tmp/atom-loss-prepared
```

Both inputs contain complete loss-visible measurement rows in RStim's b8
format. Every `MRL` or `ML` target contributes adjacent
`loss_flag,value_bit` records. Calibration rows with exactly one asserted loss
flag contribute candidates; zero- and multi-loss calibration rows are ignored.
Every circuit loss readout must acquire at least one calibrated pattern.
Sweep-dependent circuits are rejected because this v0 interface does not accept
the per-shot sweep sidecar needed for correct detector conversion.

The output bundle contains:

- `manifest.json`, including input SHA-256 values, row widths, shot counts, and
  calibration/graph diagnostics;
- `observables.b8`, a decoder-neutral target-observable stream for scoring;
- `mle/shot-000000.json` and one subsequent `atom-loss-envelope.v0` file per
  target shot; and
- `matching.json`, one batched `atom-loss-envelope-matching.v0` input.

All inputs and outputs are materialized before the bundle is installed. An
existing non-empty output directory is rejected; an absent or empty output
directory is installed from a same-filesystem staging directory.

`decode` and `matching` never read `observables.b8`; it remains a separate
truth stream, matching the ordinary `rsinter replay` separation between
detector inputs, predictions, and scoring answers.

Prepared files feed the existing decoders without another conversion step:

```sh
cargo run -q -p atom-loss-envelope -- decode \
  --in /tmp/atom-loss-prepared/mle/shot-000000.json \
  --out /tmp/mle-result.json \
  --backend highs

cargo run -q -p atom-loss-envelope -- matching \
  --in /tmp/atom-loss-prepared/matching.json \
  --out /tmp/matching-result.json
```

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

- implement the paper's Mid-SWAP syndrome-extraction circuit;
- widen the stable DEM-only `rsinter::Decoder` interface;
- expose a unified streaming `predictions.b8` replay command for prepared loss
  bundles; or
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

When invoked directly, this command requires explicit, pre-calibrated
loss-to-edge memberships. The `prepare` command can generate them from a
supported circuit and calibration set. Neither path makes a SOTA performance
or logical-error-rate claim.

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
