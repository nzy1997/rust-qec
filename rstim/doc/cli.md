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
- generation and export: `gen`, `export_json`

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
- `--after_clifford_depolarization`

This command is a convenient front door to the circuit generation APIs in
`rstim::codegen`.

## Export QSTD101 JSON with `export_json`

`export_json` converts a circuit into the repository's QSTD101 JSON document:

```sh
rstim export_json --in circuit.stim --out circuit.json
```

Formats:

- `--format pretty` default
- `--format compact`

This is useful for external visualization or structured downstream processing.

## Suggested Workflow

For a typical CLI session:

1. `rstim stats` to inspect circuit size and repeat structure
2. `rstim sample` or `rstim detect` to generate shot data
3. `rstim analyze_errors` to derive a DEM
4. `rstim m2d` or `rstim explain_errors` when converting or debugging data paths
5. `rstim export_json` when handing the circuit to structured tooling
