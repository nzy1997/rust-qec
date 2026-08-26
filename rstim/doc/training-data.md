# Local Training Data from `rstim detect`

`rstim detect --trace_out` produces aligned detector inputs, observable targets,
and detailed simulator ground truth. The packed files are suitable for ordinary
decoder training, while the JSONL trace supports debugging and optional
auxiliary supervision.

## Build the matching `rstim` version

From a checkout of this repository:

```sh
cargo build --release --locked -p rstim
export PATH="$PWD/target/release:$PATH"
rstim --version
```

Keep the binary version with the dataset. Trace schemas are versioned, but a
fixed RStim version is also part of reproducibility.

## Obtain and inspect a circuit

Use an existing local `.stim` file or generate one:

```sh
rstim gen \
  --code repetition_code \
  --task memory \
  --distance 3 \
  --rounds 3 \
  --after_clifford_depolarization 0.001 \
  --out circuit.stim

rstim stats --in circuit.stim
```

The reported measurement, detector, and observable counts appear again in the
trace manifest.

## Sample aligned files

```sh
rstim detect \
  --in circuit.stim \
  --shots 1000 \
  --seed 7 \
  --out detectors.b8 \
  --out_format b8 \
  --obs_out observables.b8 \
  --obs_out_format b8 \
  --trace_out traces.jsonl
```

Trace mode requires named output files so all requested files can be staged and
published only after successful sampling. `ptb64` output is not supported in
trace mode; use `b8` for compact rows.

The first JSONL line is the manifest:

```json
{"record_type":"manifest","schema_version":"rstim.sample_trace.v1","rstim_version":"0.2.0","circuit_sha256":"<64 lowercase hex>","seed":7,"shots":1000,"num_measurements":0,"num_detectors":0,"num_observables":0}
```

Every later line is one shot with the same zero-based index as the packed rows:

```json
{"record_type":"shot","shot_index":0,"measurements":[false],"detectors":[true],"observables":[true],"noise_events":[],"measurement_events":[],"detector_events":[],"inapplicable_noise_events":[]}
```

Stable event fields are:

- noise: `op_path`, `repeat_iterations`, `instr_name`, `target_slots`,
  `target_qubits`, `occurred`, and `branch_label`;
- measurement: `op_path`, `repeat_iterations`, `target_slot`, `target_qubit`,
  `instr_name`, `measurement_index`, `bit`, `loss_cause`, and `component`;
- detector: `op_path`, `repeat_iterations`, `detector_index`, and `flipped`;
- inapplicable noise: `op_path`, `repeat_iterations`, and `target_slots`.

## Load the dataset in Python

The checked standard-library example reads `b8`, verifies every alignment, and
leaves the arrays ready to convert into tensors:

```sh
python3 rstim/doc/examples/load_training_data.py \
  --detectors detectors.b8 \
  --observables observables.b8 \
  --trace traces.jsonl
```

Its core outputs are `detector_inputs`, `observable_targets`, and a simple
fixed-width numeric `error_features` summary that can be converted directly to
a tensor. `raw_error_records` keeps the ragged event dictionaries for custom
per-site encodings. Both forms are simulator-only supervision and are not
normally observable on hardware.

## Reproducibility and privacy

Record all of the following with a dataset:

- exact circuit bytes and manifest `circuit_sha256`;
- `rstim_version`;
- seed and shot count;
- detector and observable output formats;
- `schema_version`.

The same values reproduce byte-identical JSONL and packed streams. Changing the
binary version or circuit bytes creates a different provenance identity.

Detailed traces are much larger and slower to produce than ordinary packed
detector samples because RStim records every realized site shot by shot. Use
plain `rstim detect` when detailed simulator truth is unnecessary.

`noise_events`, `loss_cause`, and inapplicable-operation records are simulated
ground truth. Do not expose them, private seeds, or answers as contestant input
in a blinded benchmark. Real experiments only provide information that the
hardware actually heralds.

Finally, `detectors` in `rstim.sample_trace.v1` preserve the circuit's legacy
binary detector calculation. If a detector references a loss-caused measurement,
the fixed measurement bit is a storage placeholder, not a trustworthy matching
syndrome. A loss-aware decoder must first invalidate or combine affected checks
into superchecks using the separate loss information.

## Blinded logical-input training rows

`rstim export_decoder_dataset --mode measurements_blinded --error_trace`
produces a second tensor-friendly layout: public measurement rows, private
answers, private logical masks, and private physical-error traces. Every trace
line includes `logical_input`, whose `bit` and `applied` fields equal the aligned
mask bit and whose `pauli` / `support` fields identify the ideal representative.
The ideal logical gate is deliberately absent from the physical `events` list.

The canonical top-level `TICK[rstim:logical_flip_point]` marks where that ideal
source-state `X` or `Z` is inserted. It must follow ideal logical-state
initialization and precede every positive-probability noise instruction. For a
published measurement row `m`, `O_public(m)` contains both the chosen source bit
and any logical flip accumulated from physical errors. Therefore the supervised
decoder target removes the known training-time source choice:

```text
answer = O_public(measurement) XOR logical_input.bit
```

The logical-input object and physical-error trace are simulator ground truth:
keep both private in a blinded benchmark even though local training may consume
them.

The checked standard-library loader verifies all four streams and returns
fixed-width `measurement_inputs`, `answer_targets`, and `logical_masks`, plus
ragged `trace_records` for auxiliary supervision:

```sh
python3 rstim/doc/examples/load_blinded_training_data.py \
  --public-dir public-data \
  --private-dir private-truth \
  --observable-rec -1
```

Repeat `--observable-rec` for every `rec[-k]` term contributing to observable
0. Convert the three integer matrices directly to tensors; encode the ragged
event records only if the training objective uses simulator-only supervision.
