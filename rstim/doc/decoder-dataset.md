# Decoder Dataset Export

`rstim export_decoder_dataset` creates two directory bundles: a public bundle for decoder contestants and a private bundle for scoring.

## Detector Mode

```console
rstim export_decoder_dataset \
  --circuit memory.stim \
  --shots 100000 \
  --mode detectors \
  --public_out public-data \
  --private_out private-truth
```

The public bundle contains detector-event rows in `shots.b8` and the logical-zero `circuit.stim`. The private bundle contains `answers.b8`.

Contestants decode each public row to one predicted observable bit. Scoring compares each prediction with the corresponding private answer bit.

Detector mode rejects both `--logical_x_qubits` and `--logical_z_qubits`.

## Blinded Measurement Mode

Place the marker exactly once as a standalone, top-level comment at the point where an ideal logical Pauli may be inserted. It must not be inside a `REPEAT` block or appended to another instruction.

```stim
R 0 1 2
# RSTIM_LOGICAL_FLIP_POINT
```

Every qubit named by `--logical_x_qubits` or `--logical_z_qubits` must remain
loss-free before the marker. The exporter rejects a positive-probability
`LOSS` on that logical support before the insertion point; move the marker
before the first such `LOSS`. Loss after the marker, including physical loss
immediately before a loss-visible measurement, remains part of the sampled
circuit.

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

Here `O_public(m)` is the observable computed from the published measurement row. Contestants must decode the underlying unmasked logical-error bit from each measurement row (or its derived syndrome); they must not submit the directly recomputed public observable `O_public(m)`. The organizer's private scoring key remains `answer = O_public(m) XOR b`, while `masks.b8` retains the private per-shot `b` values used to produce that key.

## Files

Public files are exactly `manifest.json`, `circuit.stim`, and `shots.b8`. Private files are exactly `manifest.json`, `answers.b8`, and, in blinded measurement mode, `masks.b8`.

The manifests describe their respective bundle files and associate the public and private bundles with the same dataset. Publish only the public directory; retain the private directory for the scoring authority.
