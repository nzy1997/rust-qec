# rstim CLI Reference

`rstim` ships with a CLI for inspecting circuits, sampling data, transforming
formats, analyzing detector behavior, generating QEC circuits, and exporting
structured representations.

## Common Conventions

Most commands follow the same I/O pattern:

- `--in <path>` reads from a file; omit it to read from `stdin`
- `--out <path>` writes to a file; omit it to write to `stdout`
- circuits are provided in Stim-like text format
- detector error models are provided in DEM text format where applicable

The main command families are:

- inspection: `stats`
- sampling: `sample`, `detect`, `sample_dem`
- transforms: `convert`, `m2d`
- analysis: `analyze_errors`, `explain_errors`
- generation and export: `gen`, `render_svg`, `export_json`
- interactive visualization: `shot_viewer`

## Open the Interactive Shot Viewer

`shot_viewer` starts the version-matched web application on a random loopback
port and opens it in the default browser:

```sh
rstim shot_viewer
```

The page begins empty. Choose a local `.stim` file, sample a shot, or click an
existing noise site to override that event's realized outcome. The circuit is
parsed and executed locally; the server accepts only loopback requests and does
not receive the selected file. Use `--no_open` when you want to open the printed
URL yourself, or `--port <n>` to request a specific loopback port.

The current diagram can be exported as SVG or single-page vector PDF. Both
exports include the circuit digest, source hash, base seed, rstim version, and
manual overrides as provenance metadata.

The first version fully expands `REPEAT` blocks and rejects oversized inputs
before simulation. Defaults allow at most 256 qubits, 5,000 expanded operations,
5,000 noise events, 5,000 measurement results, and an estimated 100,000 SVG
nodes.

## Inspect Circuits with `stats`

Use `rstim stats` to summarize a circuit without executing it:

```sh
printf 'H 0\nREPEAT 2 {\n  M 0\n  DETECTOR rec[-1]\n  TICK\n}\n' | rstim stats
```

Example output:

```text
instruction_count: 5
repeat_blocks: 1
max_repeat_depth: 1
num_qubits: 1
num_measurements: 2
num_detectors: 2
num_observables: 0
num_ticks: 2
num_sweep_bits: 0
```

The fields fall into two groups:

- structural metrics: `instruction_count`, `repeat_blocks`, `max_repeat_depth`
- expanded execution-facing metrics: `num_qubits`, `num_measurements`, `num_detectors`, `num_observables`, `num_ticks`, `num_sweep_bits`

Structural metrics count the parsed syntax tree. They do not multiply through
`REPEAT` counts. Execution-facing metrics do reflect the logical expanded size
of repeated regions. This makes `stats` useful both for understanding source
shape and for estimating how much data later commands will produce.

For machine-readable output, pass `--json`:

```sh
printf 'M 0\nDETECTOR rec[-1]\n' | rstim stats --json
```

Example output:

```json
{
  "instruction_count": 2,
  "repeat_blocks": 0,
  "max_repeat_depth": 0,
  "num_qubits": 1,
  "num_measurements": 1,
  "num_detectors": 1,
  "num_observables": 0,
  "num_ticks": 0,
  "num_sweep_bits": 0
}
```

## Sample Measurements with `sample`

`sample` runs a circuit and writes measurement results. Important flags:

- `--shots <n>` number of shots, default `1`
- `--out_format <fmt>` output format, default `01`
- `--seed <u64>` deterministic RNG seed
- `--skip_reference_sample` use an all-zero reference sample in data-path logic

Example:

```sh
printf 'R 0\nX 0\nM 0\n' | rstim sample --shots 4 --out_format 01
```

Supported shot output formats include:

- `01` dense text bits
- `b8` dense binary bit-packed bytes
- `r8` sparse run-length binary
- `hits` sparse text indices
- `ptb64` transposed bit-packed binary

## Sample Detection Events with `detect`

`detect` runs a circuit and emits detection events, with optional observable
flips:

- `--append_observables` appends observables to dense output formats
- `--obs_out <path>` writes observables to a separate output stream
- `--obs_out_format <fmt>` chooses that secondary format

For sparse detector-oriented text, use:

```sh
printf 'R 0\nX_ERROR(1) 0\nM 0\nDETECTOR rec[-1]\n' | rstim detect --out_format dets
```

## Analyze Detector Error Models with `analyze_errors`

`analyze_errors` converts a noisy circuit into a detector error model:

```sh
rstim analyze_errors --in circuit.stim --out model.dem
```

Important flags:

- `--approximate_disjoint_errors`
- `--allow_gauge_detectors`
- `--decompose_errors`

Use `--decompose_errors` when you want graphlike error mechanisms suitable for
MWPM-style workflows.

## Convert Shot Formats with `convert`

`convert` rewrites measurement data between supported shot formats:

```sh
rstim convert --in_format 01 --out_format b8 --bits 128 --in shots.txt --out shots.b8
```

You must provide either:

- `--bits <n>`
- or `--circuit <path>` so `rstim` can infer the measurement count

For `ptb64` input, also provide `--shots <n>`.

## Convert Measurements to Detection Events with `m2d`

`m2d` converts measurement samples into detection events using a circuit:

```sh
rstim m2d --circuit circuit.stim --in meas.01 --in_format 01 --out_format dets
```

Important flags:

- `--append_observables`
- `--skip_reference_sample`
- `--sweep <path>`
- `--sweep_format <fmt>`
- `--ran_without_feedback`
- `--shots <n>` required for some packed input formats

This command is useful when measurements are produced separately from `rstim`
but still need to be mapped onto detector semantics.

## Explain Fired Detectors with `explain_errors`

`explain_errors` maps observed detector sets back onto compatible DEM error
terms:

```sh
rstim explain_errors --circuit circuit.stim --in dets.txt --in_format dets
```

You can also provide `--dem <path>` directly instead of deriving the DEM from
the circuit.

Supported `--in_format` values are currently:

- `dets`
- `01`

## Sample Directly from a DEM with `sample_dem`

`sample_dem` draws detection events and observable flips from a detector error
model without re-running the original circuit:

```sh
rstim sample_dem --shots 1000 --out_format dets --in model.dem
```

As with `detect`, you can split observables into a separate stream using
`--obs_out` and `--obs_out_format`.

Add `--trace_out <PATH>` to stream a versioned manifest and one detailed JSON
record per shot. Trace mode stages all outputs, requires named `--out` files,
and is intentionally slower than ordinary detector sampling. See the complete
[local training-data tutorial](training-data.md) for the schema, aligned `b8`
files, Python loading example, reproducibility guidance, and loss caveats.

## Generate Circuits with `gen`

`gen` produces common QEC benchmark circuits:

```sh
rstim gen --code repetition_code --task memory --distance 5 --rounds 5
```

Current generator controls:

- `--code`
- `--task`
- `--distance`
- `--rounds`
- `--before_round_data_depolarization`
- `--after_clifford_depolarization`
- `--before_measure_flip_probability`
- `--after_reset_flip_probability`
- `--after_clifford_loss_probability`
- `--operation_loss_probability` (`rotated_memory_z` loss-visible mode and Mid-SWAP only)
- `--measurement_loss_probability` (`rotated_memory_z` loss-visible mode and Mid-SWAP only)

Each Pauli-noise flag drives exactly the channel it is named after: there is
no "uniform" shortcut, and omitted channels default to zero. In particular
`--after_clifford_depolarization` only inserts `DEPOLARIZE1`/`DEPOLARIZE2`
after Clifford gates; it no longer broadcasts into the other channels.

For `--code surface_code --task rotated_memory_z`, setting
`--operation_loss_probability` or `--measurement_loss_probability` selects the
loss-visible conventional form: the circuit keeps the fixed CNOT layer order
in every round (no alternating A/B schedule, no shuttles), uses the native
Mid-SWAP generator's loss semantics (full rate after resets and single-qubit
gates, half rate on each qubit of a two-qubit gate, measurement-stage `LOSS`
immediately before each readout), emits loss-visible `MRL`/`ML` records in
`loss_flag,value_bit` order, and places exactly one
`TICK[rstim:logical_flip_point]` immediately after the data reset so the circuit
can drive `export_decoder_dataset --mode measurements_blinded`:

```sh
rstim gen \
  --code surface_code \
  --task rotated_memory_z \
  --distance 3 \
  --rounds 2 \
  --before_round_data_depolarization 0.011 \
  --after_clifford_depolarization 0.022 \
  --before_measure_flip_probability 0.033 \
  --after_reset_flip_probability 0.044 \
  --operation_loss_probability 0.055 \
  --measurement_loss_probability 0.066 \
  --out conventional-loss.stim
```

The native Mid-SWAP rotated-memory generator is selected by its dedicated
surface-code task. It requires an odd distance of at least 3 and emits
loss-visible `MRL`/`ML` records in `loss_flag,value_bit` order:

```sh
rstim gen \
  --code surface_code \
  --task rotated_memory_z_midswap \
  --distance 3 \
  --rounds 2 \
  --after_clifford_depolarization 0.001 \
  --operation_loss_probability 0.002 \
  --measurement_loss_probability 0.003 \
  --out midswap.stim
```

For this task, `--after_clifford_depolarization` is the Pauli-noise parameter,
`--operation_loss_probability` controls gate/reset loss, and
`--measurement_loss_probability` controls loss immediately before measurement.
Reset and measurement bit noise use `X_ERROR`; the single-qubit `H` and
two-qubit `CX` channels remain `DEPOLARIZE1` and `DEPOLARIZE2`, respectively.
Measurement-stage loss is physical atom loss reported by the loss flag, not
state-selective readout confusion.
`--after_clifford_loss_probability` is intentionally rejected to keep the two
loss channels unambiguous. The generated `# MIDSWAP_SHUTTLE` comments expose
each persistent logical-to-physical remapping without changing circuit
semantics. The generator emits exactly one `TICK[rstim:logical_flip_point]`
immediately after the data reset and before data reset noise or loss, so the
circuit can be validated for blinded measurement export.

CSS memory circuits can be generated from explicit matrix wrappers:

```sh
rstim gen \
  --code css \
  --task memory \
  --hx hx.json \
  --hz hz.json \
  --basis x \
  --rounds 3 \
  --schedule greedy \
  --observables logicals_x.json
```

`hx.json`, `hz.json`, and observable files use the explicit JSON wrappers
accepted by `rstim::codegen::css`. For CSS memory generation, `--basis x`
interprets `--observables` rows as X-like logical supports, while `--basis z`
interprets them as Z-like logical supports. Explicit observable rows must define
exactly `k` independent logical classes modulo the selected-basis stabilizer
span; invalid rows fail before a circuit is written.

This command is a convenient front door to the circuit generation APIs in
`rstim::codegen`.

## Render SVG diagrams with `render_svg`

`render_svg` is the primary static circuit visualization path. It parses a
Stim-like circuit, builds the repository's QP101 document internally, and emits
an SVG diagram without requiring Typst:

```sh
rstim render_svg --in circuit.stim --out circuit.svg
```

The command follows the common CLI I/O convention. `--in <path>` reads a circuit
from a file; omitting `--in` reads from stdin. `--out <path>` writes the SVG to a
file; omitting `--out` writes SVG to stdout:

```sh
printf 'H 0\nCX 0 1\nTICK\nM 0\n' | rstim render_svg > circuit.svg
```

For seeded sample-shot overlays, pass `--sample_shot` and an optional
deterministic seed:

```sh
rstim render_svg --sample_shot --seed 7 --in circuit.stim --out sample.svg
```

The sample-shot SVG includes visible QP101 annotations for supported sampled
events such as fired noise branches, loss-caused measurement information,
measurement outcomes, and detector flips. `--seed` is only supported with
`--sample_shot`; running `rstim render_svg --seed 7` without `--sample_shot`
fails with `--seed is only supported with --sample_shot`.

For detector-error-model debugging, render one DEM error term as source and
symptom highlights:

```sh
rstim render_svg --highlight_dem_error 0 --in circuit.stim --out highlight.svg
```

`--sample_shot` and `--highlight_dem_error` are mutually exclusive. Use one
overlay mode per render.

## Export QP101 JSON with `export_json`

`export_json` converts a circuit into the repository's QP101 JSON document:

```sh
rstim export_json --in circuit.stim --out circuit.json
```

Formats:

- `--format pretty` default
- `--format compact`

Use `export_json` when you need QP101 structured data for downstream
processing, fixture generation, or the optional legacy/prototype Typst
`qp101-viz` workflow. For ordinary static SVG diagrams, prefer `render_svg`.

## Suggested Workflow

For a typical CLI session:

1. `rstim stats` to inspect circuit size and repeat structure
2. `rstim sample` or `rstim detect` to generate shot data
3. `rstim analyze_errors` to derive a DEM
4. `rstim m2d` or `rstim explain_errors` when converting or debugging data paths
5. `rstim render_svg` when you want a static SVG circuit diagram
6. `rstim export_json` when handing QP101 data to structured tooling

## See Also

- [Showcase index](docs/showcases/README.md)
- [rstim CLI DEM Pipeline showcase](docs/showcases/rstim-cli-dem-pipeline.md)
- [rstim Render SVG Atom-Loss showcase](docs/showcases/rstim-render-svg-atom-loss.md)
