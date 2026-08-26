# Decoder Dataset Export

`rstim export_decoder_dataset` creates two directory bundles: a public bundle for decoder contestants and a private bundle for scoring.

## Detector Mode

```console
rstim export_decoder_dataset \
  --circuit memory.stim \
  --shots 100000 \
  --batch_shots 10000 \
  --mode detectors \
  --public_out public-data \
  --private_out private-truth
```

The public bundle contains detector-event rows in `shots.b8` and the logical-zero `circuit.stim`. The private bundle contains `answers.b8`.

Contestants decode each public row to one predicted observable bit. Scoring compares each prediction with the corresponding private answer bit.

Detector mode rejects both `--logical_x_qubits` and `--logical_z_qubits`.

## Bounded-memory export

The exporter generates and writes shots in bounded batches instead of keeping
the full dataset in memory. `--batch_shots` controls the maximum number of
shots held by one generation batch and defaults to `10000`; lowering it reduces
peak memory at the cost of more sampling calls. The selected value is recorded
in the private manifest so seeded exports remain reproducible.

Output files and their SHA-256 digests are written incrementally. Peak dataset
memory therefore scales with `--batch_shots`, not the total value of `--shots`.

## Optional per-shot error trace

`--error_trace` additionally writes `trace.jsonl` into the private bundle and
a `trace_file` entry into the private manifest. Each line records the complete
noise realization behind one shot — every Pauli branch that fired and every
`LOSS` onset — in the versioned `rstim.error-trace.v1` schema (see
`docs/specs/error-trace-v1.md`). Traced export samples shot by shot, so it is
slower than batch sampling and produces a different batch than an untraced
export with the same seed; within one traced export the trace, shots, answers,
and masks always describe the same executions, and a fixed seed remains
byte-for-byte reproducible.

## Blinded Measurement Mode

Place the canonical tagged instruction exactly once at top level, after ideal
logical-state initialization and before the first positive-probability noise
instruction anywhere in the circuit:

```stim
R 0 1 2
TICK[rstim:logical_flip_point]
```

The tag must annotate `TICK` and must not occur inside a `REPEAT` block. The
legacy `# RSTIM_LOGICAL_FLIP_POINT` comment is not recognized. The exporter
rejects any positive-probability Pauli, loss, correlated-error, or measurement
noise before the marker, including noise nested in a completed `REPEAT` block.
Zero-probability noise instructions are allowed. Noise after the marker,
including physical loss immediately before a loss-visible measurement, remains
part of the sampled circuit.

```console
rstim export_decoder_dataset \
  --circuit memory.stim \
  --shots 100000 \
  --mode measurements_blinded \
  --logical_x_qubits 0,2,4 \
  --public_out public-data \
  --private_out private-truth
```

Blinded mode requires exactly one logical-qubit option. Use `--logical_x_qubits` for an ideal physical `X` representative, such as a logical X for a Z-basis memory experiment. Use `--logical_z_qubits` for an ideal physical `Z` representative, such as a logical Z for an X-basis memory experiment:

```console
rstim export_decoder_dataset \
  --circuit memory-x.stim \
  --shots 100000 \
  --mode measurements_blinded \
  --logical_z_qubits 1,7,13 \
  --public_out public-data \
  --private_out private-truth
```

For each shot, the exporter privately chooses a bit `b`, samples either the public circuit or the circuit with the requested ideal `X` or `Z`, publishes the measurement row, and stores `answer = O_public(m) XOR b` privately. Before exporting, it verifies noiselessly that the injected Pauli preserves every detector value and flips observable 0.

With `--error_trace`, each private trace line also contains the training label
that selected the producer circuit:

```json
"logical_input":{"bit":1,"applied":true,"pauli":"X","support":[0,2,4]}
```

`bit` is exactly the aligned `masks.b8` bit; `applied` is the same value and
states whether the ideal Pauli gate was inserted for that shot. `pauli` and
`support` describe the configured representative. This object is omitted in
detector mode, and the intentional `X` or `Z` gate is never recorded as a
physical noise event. See `doc/examples/load_blinded_training_data.py` for a
small loader that aligns shots, answers, masks, and trace rows.

Here `O_public(m)` is the observable computed from the published measurement row. Contestants must decode the underlying unmasked logical-error bit from each measurement row (or its derived syndrome); they must not submit the directly recomputed public observable `O_public(m)`. The organizer's private scoring key remains `answer = O_public(m) XOR b`, while `masks.b8` retains the private per-shot `b` values used to produce that key.

Equivalently, `O_public(m)` combines the intentional source bit `b` with any
logical flip accumulated from physical errors. XORing out `b` makes `answer`
the supervised physical logical-error target rather than the randomized source
state itself.

## Files

Public files are exactly `manifest.json`, `circuit.stim`, and `shots.b8`. Private files are exactly `manifest.json`, `answers.b8`, and, in blinded measurement mode, `masks.b8`, plus `trace.jsonl` when `--error_trace` is on.

The manifests describe their respective bundle files and associate the public and private bundles with the same dataset. Publish only the public directory; retain the private directory for the scoring authority.
