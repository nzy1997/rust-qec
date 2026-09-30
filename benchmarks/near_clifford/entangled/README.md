# Entangled near-Clifford workloads

This separate 28-configuration campaign extends the scale probes without changing
historical drivers, locks, matrices or retained results. These are synthetic
workloads, not application-level logical-error benchmarks.

- `brick_R_W_D`: R alternating T/T-dagger injections into W initially Hadamard
  qubits, followed by D noncommuting nearest-neighbor CZ/CX and H/S layers.
- `parity_R_W_D`: the same layers on W−1 data qubits plus an ancilla that gathers
  long-range parity after each layer, then mixed-basis terminal measurements.
- `rounds_R_W_D`: sparse non-Clifford injections, entangling layers, parity MR,
  record-controlled X and further H/T injections over D rounds, then terminal
  measurements. It exercises unplanned execution, reset and feedback.

R describes construction, not observed runtime rank. The retained untimed
counters report prepared and seeded peak active rank, cache behavior and fallbacks.
Widths 65/129/193 contain data gates across the width, not merely idle padding.
Terminal measurements use reverse order and mixed X/Y/Z bases.

The standalone harness raises the reused test oracle allocation guard from 10 to
16 qubits, with original and adapted source hashes recorded; its matrix arithmetic
is unchanged and the repository test helper is not edited.

The independent dense oracle checks conditional X/Y/Z Born probabilities on three
seeded trajectories and detects entanglement through a one-qubit reduced-state
purity below 0.99. For width ≤16 it checks the exact workload. Larger workloads
use an explicitly recorded reduced 8-data-qubit family witness, retaining depth
and mid-circuit structure. This is not an independent full-width statistical
validation. Terminal outputs are additionally checked against dense joint
support; this does not test full distribution frequencies. Terminal planning may
reorder commuting measurements, so equality to manual per-seed execution is only
required for the unplanned `rounds` family. Structured/flat/prepared/individual
output and RNG checks are required for every exact full-width workload.

```sh
python3 benchmarks/near_clifford/entangled/run.py \
  --baseline 74754aa27461ceca72f9145832e37bfd8040f066 \
  --candidate 1aefe277faa199e59000ee186bed905fcb1668fa \
  --scratch drafts/near-clifford-entangled-reproduction \
  --output drafts/near-clifford-entangled-reproduction/results.json
python3 benchmarks/near_clifford/entangled/verify.py \
  drafts/near-clifford-entangled-reproduction/results.json \
  --git-sources --binaries drafts/near-clifford-entangled-reproduction
```

Use a nonexistent scratch directory. `--quick --only FIXTURE ...` is a smoke run,
which the final evidence verifier rejects. The adapted Rust driver replaces only
fixture construction and adds untimed physics checks to `verify`; all six timing
boundaries, 64-shot warmups, RNG initialization and process isolation come from
the unchanged scale driver. Three paired processes alternate execution order;
each has three repetitions. Baseline/candidate use identical unified Cargo locks.
Both pristine and diagnostic binaries repeat the full output/RNG checks. Oracle,
fixture helper, generated driver, inherited inputs and binaries are hashed.
RSS includes validation, multiple samplers and outputs, not one cache's memory.

After verified timing completes:

```sh
python3 benchmarks/near_clifford/entangled/profile.py RESULTS.json \
  --scratch CAMPAIGN_SCRATCH --fixtures brick_12_12_3 parity_12_129_3 rounds_8_129_4 \
  --output drafts/near-clifford-entangled-profiles
python3 benchmarks/near_clifford/entangled/test_contract.py
```

The profiler reuses the scale frame-pointer/debug build and exact retained circuit
text with the unified lock; profiling timings and samples are separate evidence.
The verifier's negative controls reject missing configurations, altered inputs,
wrong circuit text, modes, medians, semantic payloads and diagnostic gaps, including
when Python assertions are disabled.
