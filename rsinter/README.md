# rsinter

`rsinter` is an experimental parallel sampling, frozen-dataset replay, and
benchmark collection harness for RustQEC decoder comparisons.

## Install

The default installation provides the library, benchmark merging, and the
`predict-zero` benchmark runner. Install a decoder feature to run replay or a
real decoder benchmark. For BP-OSD and MWPM:

```sh
cargo install --locked rsinter --version 0.3.0 --features rbposd-runner,rmatching-runner
rsinter --help
```

For use as a library:

```toml
[dependencies]
rsinter = { version = "0.3.0", features = ["rbposd-runner"] }
```

The RustQEC crates used by this release (`rstim`, `rbposd`, `rmatching`,
`rilpqec`, and `qec-ilp-core`) are all resolved at version 0.3. Rust 1.88 or
newer is required.

## Replay a detector dataset

This complete example creates a one-error detector error model and two b8
detector rows, then decodes them with BP-OSD:

```sh
tmp_dir="$(mktemp -d)"
printf 'error(0.1) D0 L0\n' > "$tmp_dir/model.dem"
printf '\000\001' > "$tmp_dir/detectors.b8"

rsinter replay \
  --dem "$tmp_dir/model.dem" \
  --dets "$tmp_dir/detectors.b8" \
  --decoder rbposd \
  --predictions-out "$tmp_dir/predictions.b8" \
  --stats-out "$tmp_dir/stats.json"

cat "$tmp_dir/stats.json"
```

The statistics report `num_shots: 2`, `num_detectors: 1`, and
`num_observables: 1`. The prediction file contains bytes `00 01`: the second
shot predicts an observable flip.

The input and prediction files use Stim's b8 convention: each shot is an
LSB-first, byte-aligned row. See the full
[replay workflow](https://github.com/nzy1997/rust-qec/blob/master/docs/rsinter-replay.md)
for decoder configuration and output details.

## Plot measured logical error rates

With the `plotting` feature, `rsinter plot-error-rate-csv` reads a CSV containing
`distance`, `physical_error_rate`, `shots`, and `logical_failures` columns. Other
columns are allowed. It plots each distance as a separate series with
likelihood-interval bands on logarithmic axes:

```sh
cargo run --locked -p rsinter --features plotting -- \
  plot-error-rate-csv --input surface-code-ler.csv --out surface-code-ler.svg
```

The [Get started guide](https://nzy1997.github.io/rust-qec/get-started/#logical-error-rate)
uses the installed `rstim surface-code-ler` command to produce its measured
CSV and SVG without a repository checkout at run time.

## Features

Commands remain visible in `--help` when their feature is disabled and report
the feature required to run them.

| Feature | Capability |
| --- | --- |
| `rbposd-runner` | BP-OSD/BP-LSD replay, benchmarks, and `bb-circuit-bposd-memory` |
| `rmatching-runner` | MWPM replay and benchmarks |
| `ilp-runner` | ILP replay and benchmarks; includes native HiGHS build requirements |
| `plotting` | Plot commands and the `surface_code_threshold` example; includes native font dependencies |
| `full` | All runner and plotting features |
| `gurobi` | Adds the configured Gurobi backend to `ilp-runner`; HiGHS remains enabled |

From a source checkout, a focused build is:

```sh
cargo build --locked -p rsinter --features rbposd-runner
```

Use `cargo build --locked -p rsinter --features full` only when all decoder and
plotting capabilities, including their native build dependencies, are needed.
The [benchmark guide](https://nzy1997.github.io/rust-qec/benchmarks/) describes
the benchmark spec and result formats.

Apache-2.0 licensed; `LICENSE` is included in the crate.
