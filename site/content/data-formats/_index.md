+++
title = "Sample and dataset formats"
template = "data-formats.html"
[extra]
root = ".."
nav = "data-formats"
+++
## Choose the stream by its meaning

| Stream | Width per shot | Contents |
| --- | --- | --- |
| Measurement samples | Circuit measurement count | Readout bits, including paired loss flags where present |
| Detector samples | Detector count | Parities relative to the noiseless reference |
| Observable answers/predictions | Observable count | Private actual flips or decoder predictions |

`--append-observables` concatenates detector bits then observable bits into one row. Keep observables in a separate `--obs-out` file for decoder evaluation. A raw measurement row cannot be used as a detector row; model dimensions and stream type must agree.

## 01 and b8

`01` is text: one character per bit and one line per shot. `b8` is binary: fixed-width, byte-aligned rows in shot order, least-significant bit first within each byte. Row stride is `(bits_per_shot + 7) // 8`. Unused high bits in the last byte must be zero. There is no embedded shape header; preserve the circuit or manifest.

For a row with logical bits `[1,0,1]`, b8 contains byte `0x05`. Two three-bit rows take two bytes, not one packed six-bit stream. Empty-width data cannot reveal the shot count from byte length; callers must preserve it explicitly. The [CLI help](../reference/#all-commands) lists other formats supported by each command; not every command accepts the same encodings.

## Loss records and derived features

For each loss-visible readout, the flag is stored before the value. Preserve both bits, retain their record indices, and deinterleave them into distinct value/mask channels for training. A lost value is a placeholder. Raw detector parities involving it are not reliable syndrome evidence. The [training tutorial](../sampling-data/#loss-tensors) gives tensor shapes and a checked loader.

## Dataset bundles

A public bundle contains `manifest.json`, `circuit.stim` and `shots.b8`. A private bundle contains its manifest and `answers.b8`, plus `masks.b8` for blinded measurements and optional `trace.jsonl`. Pair manifests by `dataset_id`, verify declared hashes and shapes, and preserve shot order. `rstim dataset import` validates imported bundles; native loss decode additionally checks its accepted subset before compiling.

`detectors` mode publishes detector rows. `measurements_blinded` publishes measurement rows from a privately randomized logical input; its private answer is the public observable parity XOR the hidden source bit. Score predictions directly against `answers.b8`; applying the mask again changes the scoring target. The public manifest excludes the seed, private paths, answers and masks.

## Trace contracts and loss logs

Simulator traces (`rstim.sample_trace.v1`) and per-shot dataset error traces (`rstim.error-trace.v1`) are different schemas. A trace can reveal realized physical errors and private logical input metadata; keep it private for blind evaluation. Loss-log ordinals identify loss-visible readouts, not arbitrary measurement-bit indices. Loaders must validate shot and readout alignment rather than zip unrelated files.

The normative [error-trace v1](https://github.com/nzy1997/rust-qec/blob/master/docs/specs/error-trace-v1.md) and [loss-log v1](https://github.com/nzy1997/rust-qec/blob/master/docs/specs/loss-log-v1.md) specify their fields and validation. The <a href="#dataset-export-contract">dataset export contract</a> below is staged from its canonical source.
