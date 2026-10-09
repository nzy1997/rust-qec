+++
title = "rsinter CLI"
template = "cli-help.html"
[extra]
root = ".."
nav = "rsinter-cli"
cli_key = "rsinter"
+++
`rsinter` provides experimental decoder replay and benchmark orchestration. Replay consumes frozen detector inputs; benchmark campaigns generate or collect cases. Neither is a supported universal decoder ranking.

## Decoder replay

`replay` reads a DEM and `b8` detector rows and writes `b8` observable predictions plus JSON statistics. It compiles the decoder once and streams batches. Inputs, configuration and output paths must be distinct; outputs are installed after successful validation and decoding. `--shots` validates the inferred count and is required for a zero-detector model. [Compare two decoders](../decoding/) on the same input before running a large campaign.

| Decoder | Feature | Configuration |
| --- | --- | --- |
| `rmatching` | `rmatching-runner` | No decoder keys; graphlike components required |
| `rbposd` | `rbposd-runner` | BP method/schedule, iteration limit, OSD method/order |
| `rbplsd` | `rbposd-runner` | BP method/schedule, iteration limit, LSD method/order |
| `rilpqec` | `ilp-runner` | Backend, time limit, gap, threads, verbosity |

Configurations are TOML. Unknown or inappropriate keys are rejected. Omitting the config file selects defaults, which are recorded in the statistics. See the [replay contract](https://github.com/nzy1997/rust-qec/blob/master/docs/rsinter-replay.md) for accepted key values.

## Campaign files and resuming

`rsinter bench run --spec campaign.toml --language rust --out results` reads a TOML campaign specification and selects its Rust runners. The required `--language` value must match a runner language in the specification. `--resume` continues the campaign using its recorded state; preserve the specification, input hashes, decoder configuration and state files together. `bench merge` combines compatible outputs; plot commands read their stated CSV or result files. The [campaign schema](https://github.com/nzy1997/rust-qec/blob/master/rsinter/src/bench/spec.rs) and [comparison benchmarks](../benchmarks/decoders/) document reproduction and interpretation.

## Feature availability and errors

Help includes commands whose Cargo feature is disabled; running one reports the feature it needs. Runner features and `plotting` are installed separately in [Get started](../get-started/#install). Plotting needs native font dependencies; ILP additionally needs the native solver toolchain. Failures exit nonzero; do not assume `rstim`'s structured error codes apply to `rsinter`.
