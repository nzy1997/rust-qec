# rstim CLI DEM Pipeline

This showcase runs one tiny noisy circuit through the existing `rstim` CLI:
inspect it, sample detector events, extract a detector error model, and sample
from that DEM.

## What This Shows

The circuit has one qubit, one deterministic `X_ERROR(1)`, one measurement,
one detector, and one observable. That makes every command output small enough
to inspect directly while still exercising the detector-event and DEM path.

The pipeline is:

1. `stats` counts the circuit structure.
2. `detect` samples detector events from the circuit.
3. `analyze_errors` writes a detector error model.
4. `sample_dem` samples detector events from that model.

## Run It

Run these commands from the repository root. The commands use `cargo run -q`
so they exercise the workspace CLI without requiring a separately installed
`rstim` binary.

```sh
workdir="${TMPDIR:-/tmp}/rstim-cli-dem-pipeline"
rm -rf "$workdir"
mkdir -p "$workdir"

cat > "$workdir/pipeline.stim" <<'STIM'
R 0
X_ERROR(1) 0
M 0
DETECTOR rec[-1]
OBSERVABLE_INCLUDE(0) rec[-1]
STIM

cargo run -q -p rstim -- stats --in "$workdir/pipeline.stim"
cargo run -q -p rstim -- detect --shots 1 --out_format dets --in "$workdir/pipeline.stim"
cargo run -q -p rstim -- analyze_errors --in "$workdir/pipeline.stim" --out "$workdir/pipeline.dem"
cat "$workdir/pipeline.dem"
cargo run -q -p rstim -- sample_dem --shots 1 --out_format dets --in "$workdir/pipeline.dem"
```

The documented failure case uses an invalid repeat count:

<!-- rstim-cli-dem-pipeline-bad-input-start -->
```stim
REPEAT two {
  M 0
}
```
<!-- rstim-cli-dem-pipeline-bad-input-end -->

```sh
cat > "$workdir/bad-repeat.stim" <<'STIM'
REPEAT two {
  M 0
}
STIM

cargo run -q -p rstim -- stats --in "$workdir/bad-repeat.stim"
```

## Expected Result

`stats` prints exact field counts for this circuit:

```text
instruction_count: 5
repeat_blocks: 0
max_repeat_depth: 0
num_qubits: 1
num_measurements: 1
num_detectors: 1
num_observables: 1
num_ticks: 0
num_sweep_bits: 0
```

`detect --out_format dets` prints one deterministic detector event and one
observable flip:

```text
shot D0 L0
```

`analyze_errors` writes this DEM:

```text
error(1) D0 L0
```

`sample_dem --out_format dets` samples that DEM back to the same detector and
observable labels:

```text
shot D0 L0
```

The invalid repeat-count example exits nonzero and writes this stderr snippet:

```text
Error: line 1: bad repeat count
```

## Code

Primary CLI documentation:

- [`rstim/doc/cli.md`](rstim/doc/cli.md)

Tests that cover the showcased command families:

- [`rstim/tests/cli_stats.rs`](rstim/tests/cli_stats.rs)
- [`rstim/tests/cli_sample_dem.rs`](rstim/tests/cli_sample_dem.rs)
- [`rstim/tests/cli_integration.rs`](rstim/tests/cli_integration.rs)

## Verification

Validate this page's section structure and repo-relative links:

```sh
python3 tools/check_showcase_docs.py docs/showcases/rstim-cli-dem-pipeline.md
```

Run the CLI tests that cover the showcased commands and the documented
bad-input contract:

```sh
cargo test -p rstim --test cli_stats --test cli_sample_dem --test cli_integration -q
```

Expected: the checker prints `ok:` for this page, and the Cargo command exits
0. The `cli_integration` suite fails if the documented invalid repeat-count
input is replaced by a valid circuit.

## Limits

This is a CLI data-path smoke example, not a simulator parity claim. It uses a
single deterministic error mechanism so the stdout snippets stay exact and
reviewable. It does not cover random-noise statistics, packed binary output
formats, large circuits, or decoder performance.
