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
- implement the faster Envelope-Matching approximation;
- widen the stable DEM-only `rsinter::Decoder` interface; or
- run threshold, effective-distance, or publication-scale studies.

Those layers can be added in separate PRs after this explicit-input kernel is
stable. The Python/Gurobi oracle in the companion `auto-decoder` research tree
was used as a semantic cross-check only; no code or sampled pattern data is
copied from that unlicensed tree.
