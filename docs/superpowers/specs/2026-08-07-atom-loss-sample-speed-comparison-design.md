# Atom-Loss Sample-Speed Comparison Design

## Objective

Extend the existing `stim-style-surface-sample-d11-r100-b1024` performance
comparison with one directly comparable atom-loss sampling variant. The new
variant must use the same distance-11, 100-round surface-code circuit shape,
1024 shots, warmup/measurement controls, summary schema, and Markdown report as
the existing comparison.

The existing `stim-cli`, `rstim-interpreted`, and `rstim-compiled` variants stay
unchanged. The report gains a fourth variant named
`rstim-interpreted-atom-loss` and a direct atom-loss-over-baseline interpreted
ratio.

## Explored Approaches

1. Add the atom-loss workload as a fourth variant of the existing case. This
   puts the requested comparison in the existing report and preserves the
   original three results. It requires a narrowly scoped extension allowing one
   variant to use a paired circuit source.
2. Add a separate report-only benchmark case. This keeps circuit identity and
   tool/backend identity completely separate, but the result is no longer a
   direct fourth item in the prior comparison and requires a cross-case ratio.
3. Add a dedicated paired Python runner. Alternating baseline and atom-loss
   execution order would provide strong timing symmetry, but it would create a
   separate entry point and report instead of extending the current framework.

Selected approach: option 1. The user explicitly wants one additional item in
the previous comparison. The implementation will keep the new label explicit
about both the backend (`interpreted`) and workload (`atom-loss`) so it cannot
be mistaken for a new simulator backend.

## Circuit Pair

Keep the canonical baseline fixture unchanged:

```text
benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim
```

Add a checked, deterministic atom-loss fixture beside it. It preserves the
baseline circuit's qubits, gates, rounds, measurements, detectors, observables,
and instruction order except for the noise instructions immediately after each
two-qubit gate layer.

For every two-qubit gate layer with targets `q1 q2 ...`, the atom-loss fixture
uses:

```stim
<TWO_QUBIT_GATE> q1 q2 ...
LOSS(p) q1 q2 ...
DEPOLARIZE2(p) q1 q2 ...
```

The `LOSS` target list exactly matches the two-qubit gate target list. `rstim`
samples `LOSS` independently for every target, so each atom receives an
independent loss event. The existing fixture currently uses `CX` for its
two-qubit gate layers; validation nevertheless checks the structural target
contract rather than relying only on a textual count.

Single-qubit noise, reset noise, measurement noise, and all non-noise circuit
instructions remain unchanged. Atom loss is added only after two-qubit gates,
not after single-qubit Clifford gates.

## Probability Contract

The original two-qubit noise probability is `0.001`. After the change, a
two-qubit gate has three independent error events:

1. one `DEPOLARIZE2(p)` event;
2. one `LOSS(p)` event for the first atom;
3. one `LOSS(p)` event for the second atom.

Choose

```text
p = 1 - (1 - 0.001)^(1/3)
  = 1 - 0.999^(1/3)
  ~= 0.0003334445062
```

Therefore the probability that at least one of the three events occurs is
`1 - (1 - p)^3 = 0.001`, preserving the original per-two-qubit-gate aggregate
error probability while splitting it across depolarization and two independent
atom-loss events.

The checked fixture and benchmark metadata use one shared constant or one
stable decimal representation derived from this formula. Tests compare with a
floating-point tolerance rather than depending on display rounding.

## Performance Harness Integration

Extend the existing performance case model with an optional paired atom-loss
circuit source. Only `stim-style-surface-sample-d11-r100-b1024` configures that
source. Other cases retain their current variants and behavior.

Add `rstim-interpreted-atom-loss` as an explicit performance variant. It:

- reads the paired atom-loss fixture;
- runs `SamplingBackend::Interpreted`, which follows the executor fallback
  required by `LOSS`;
- uses `SampleOutputMode::MeasurementsOnly` and 1024 shots, matching the
  baseline sample variants;
- participates in the same warmup and measured-round loop;
- emits the existing timing, shots-per-second, peak-memory, status, and failure
  fields.

Stim is not run on the atom-loss fixture because `LOSS` is an `rstim` extension.
The compiled sampler is not advertised for that fixture because `LOSS` selects
the executor fallback. The original baseline Stim and compiled measurements
remain present and unchanged.

## Summary and Report

Add a comparison kind that reports:

```text
rstim-interpreted-atom-loss / rstim-interpreted
```

The ratio uses median wall time, consistent with existing comparisons. A ratio
greater than one means the atom-loss route is slower. Both variants also retain
their individual median wall times and median shots per second.

The Markdown case section includes this concise explanation:

> Each two-qubit gate has one depolarization event and two independent per-atom
> loss events; using p = 1 - 0.999^(1/3) ~= 0.0003334445062 keeps the probability
> of at least one error equal to 0.001.

The explanation appears only for the configured atom-loss comparison and does
not change unrelated reports.

## Validation and Tests

Add focused tests that establish:

- the existing public case exposes the original three variants plus
  `rstim-interpreted-atom-loss` in stable order;
- the atom-loss variant selects the paired fixture and interpreted executor
  path;
- the paired fixture has the same qubit, measurement, detector, observable, and
  repeat metadata as the baseline;
- every expanded two-qubit gate layer is followed by exactly one `LOSS` layer
  with identical targets and a `DEPOLARIZE2` layer using the same probability;
- no single-qubit gate receives a newly inserted loss layer;
- the configured probability satisfies `1 - (1 - p)^3 = 0.001` within numeric
  tolerance;
- raw JSONL contains completed atom-loss measurement records;
- summary JSON contains the new variant and the atom-loss-over-baseline ratio;
- the Markdown report contains the fourth item, ratio, and one-sentence
  probability explanation;
- existing selected-case and multi-case benchmark runners accept and preserve
  the extended comparison.

Run focused Rust performance tests, Python benchmark-runner tests, the full
`rstim` test suite, and a release-profile one-round smoke benchmark of the
selected case. Generated benchmark output remains under a temporary or ignored
output directory and is not committed as evidence in this change.

## Scope Limits

- Do not change the canonical baseline fixture or its original three variants.
- Do not add atom loss after single-qubit gates.
- Do not add atom-loss support to Stim or the compiled fast path.
- Do not publish or refresh checked performance evidence in this change.
- Do not enforce a performance threshold; the new ratio is report-only.

## Self-Review

- The design contains no unresolved placeholders.
- The probability formula counts all three independent error events.
- The fourth item is clearly labeled as an interpreted atom-loss workload, not
  as a new backend.
- The baseline circuit and results remain unchanged.
- The fixture, raw records, summary, and report all have explicit verification
  coverage.
