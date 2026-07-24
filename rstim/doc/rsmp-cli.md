# RSMP v1 CLI Guide

This guide covers the operational `pack_samples` and `unpack_samples` CLI
surface for RSMP v1 archives. The original circuit is required for packing,
unpacking, and verify-only validation.

## Pack Samples

Pack measurement samples into an RSMP archive:

```console
cargo run --locked -p rstim --bin rstim -- pack_samples \
  --circuit rstim/tests/fixtures/rsmp/v1/compat.stim \
  --shots 4 \
  --in rstim/tests/fixtures/rsmp/v1/compat-measurements.b8 \
  --in_format b8 \
  --out /tmp/compat.rsmp
```

`pack_samples` accepts measurement input formats `01`, `b8`, and `ptb64`.
The default `--in_format` is `b8`. It does not accept detector-error-model
input; DEM-only input is unsupported because the archive transform is derived
from the original circuit and noiseless reference.

## Unpack Samples

Unpack measurements, detectors, and observables from an archive:

```console
cargo run --locked -p rstim --bin rstim -- unpack_samples \
  --circuit rstim/tests/fixtures/rsmp/v1/compat.stim \
  --in /tmp/compat.rsmp \
  --measurements_out /tmp/measurements.b8 \
  --measurements_out_format b8 \
  --detectors_out /tmp/detectors.01 \
  --detectors_out_format 01 \
  --obs_out /tmp/observables.01 \
  --obs_out_format 01
```

The original circuit is required for unpack because v1 stores a circuit
identity digest and transform dimensions, not a complete replacement for the
circuit. Passing a different circuit fails before decoded outputs are trusted.

## Verify Only

Use `unpack_samples --verify_only` when a nondeveloper needs to validate an
archive without producing result files:

```console
cargo run --locked -p rstim --bin rstim -- unpack_samples \
  --circuit rstim/tests/fixtures/rsmp/v1/compat.stim \
  --in rstim/tests/fixtures/rsmp/v1/compat-v1.rsmp \
  --verify_only
```

On success, verify-only prints a concise archive summary. `--verify_only` is
the recommended nondeveloper validation route because it validates archive
integrity, circuit identity, block order, decompression, logical payload
digests, and the trailer without creating or replacing output files.

## Result Formats

`pack_samples --in_format` supports:

- `01`: text rows of ASCII `0` and `1`, one newline-terminated row per shot.
- `b8`: dense binary rows, LSB-first within each byte.
- `ptb64`: partially transposed bit-packed binary.

`unpack_samples --measurements_out_format` and `--obs_out_format` support
`01`, `b8`, `r8`, `hits`, and `ptb64`.

`unpack_samples --detectors_out_format` supports `01`, `b8`, `r8`, `hits`,
`ptb64`, and `dets`. The `dets` format is detector-only because it includes
detector and logical observable labels in Stim-style text.

## Operational Semantics

File outputs are atomic per file, not a multi-file transaction. Each configured
file output is staged and then renamed to its final path. If a later rename
fails, already-published files are retained and the error reports the retained
paths. Callers that need all-or-nothing multi-file publication must stage in
their own directory and promote that directory after the command succeeds.

Stdout cannot be rolled back. When an unpack stream writes to stdout, a late
failure can leave an already-verified prefix in stdout before the command exits
nonzero. This can happen for trailer, trailing-data, or later-block failures
that are discovered after earlier blocks have been emitted.

Archives are checked for integrity but are not authenticated. Use a separate
signature or MAC layer when producer identity matters.

`--verify_only` is the recommended nondeveloper validation route when no
result files should be created.

Sweep-bit circuits are unsupported in v1. DEM-only input is unsupported. v1
archives provide sequential access only and no random shot access.

## Compression Evidence

The readiness gate validates committed compression evidence; it does not rerun
the timing/evidence generator. To check the committed evidence directly:

```console
python3 tools/check_rsmp_v1_compression_evidence.py \
  --results-dir benchmarks/rstim_vs_stim_simulator/results/rsmp-v1
```

To regenerate the evidence as a separate developer action:

```console
python3 -m benchmarks.rstim_vs_stim_simulator.run_rsmp_compression \
  --results-dir benchmarks/rstim_vs_stim_simulator/results/rsmp-v1
```

The checked gates are `benchmark_raw_lt_20pct`, `benchmark_zstd_lt_75pct`, and
`high_entropy_raw_le_102pct`. They are claims about the pinned evidence cases
and recorded environment only, not a wall-clock performance threshold.

## Documented CLI Surface

```json
{
  "commands": [
    {
      "name": "pack_samples",
      "options": [
        {
          "allowed_values": null,
          "default": null,
          "name": "--benchmark-telemetry-json",
          "required": false,
          "short": null,
          "value": "BENCHMARK_TELEMETRY_JSON"
        },
        {
          "allowed_values": null,
          "default": null,
          "name": "--circuit",
          "required": true,
          "short": null,
          "value": "CIRCUIT"
        },
        {
          "allowed_values": null,
          "default": null,
          "name": "--shots",
          "required": true,
          "short": null,
          "value": "SHOTS"
        },
        {
          "allowed_values": null,
          "default": null,
          "name": "--in",
          "required": true,
          "short": null,
          "value": "IN"
        },
        {
          "allowed_values": [
            "01",
            "b8",
            "ptb64"
          ],
          "default": "b8",
          "name": "--in_format",
          "required": false,
          "short": null,
          "value": "IN_FORMAT"
        },
        {
          "allowed_values": null,
          "default": null,
          "name": "--out",
          "required": true,
          "short": null,
          "value": "OUT"
        },
        {
          "allowed_values": null,
          "default": null,
          "name": "--help",
          "required": false,
          "short": "-h",
          "value": null
        }
      ],
      "required_options": [
        "--circuit",
        "--shots",
        "--in",
        "--out"
      ],
      "usage": "rstim pack_samples [OPTIONS] --circuit <CIRCUIT> --shots <SHOTS> --in <IN> --out <OUT>"
    },
    {
      "name": "unpack_samples",
      "options": [
        {
          "allowed_values": null,
          "default": null,
          "name": "--benchmark-telemetry-json",
          "required": false,
          "short": null,
          "value": "BENCHMARK_TELEMETRY_JSON"
        },
        {
          "allowed_values": null,
          "default": null,
          "name": "--circuit",
          "required": true,
          "short": null,
          "value": "CIRCUIT"
        },
        {
          "allowed_values": null,
          "default": null,
          "name": "--in",
          "required": true,
          "short": null,
          "value": "IN"
        },
        {
          "allowed_values": null,
          "default": null,
          "name": "--measurements_out",
          "required": false,
          "short": null,
          "value": "MEASUREMENTS_OUT"
        },
        {
          "allowed_values": [
            "01",
            "b8",
            "r8",
            "hits",
            "ptb64"
          ],
          "default": "b8",
          "name": "--measurements_out_format",
          "required": false,
          "short": null,
          "value": "MEASUREMENTS_OUT_FORMAT"
        },
        {
          "allowed_values": null,
          "default": null,
          "name": "--detectors_out",
          "required": false,
          "short": null,
          "value": "DETECTORS_OUT"
        },
        {
          "allowed_values": [
            "01",
            "b8",
            "r8",
            "hits",
            "ptb64",
            "dets"
          ],
          "default": "b8",
          "name": "--detectors_out_format",
          "required": false,
          "short": null,
          "value": "DETECTORS_OUT_FORMAT"
        },
        {
          "allowed_values": null,
          "default": null,
          "name": "--obs_out",
          "required": false,
          "short": null,
          "value": "OBS_OUT"
        },
        {
          "allowed_values": [
            "01",
            "b8",
            "r8",
            "hits",
            "ptb64"
          ],
          "default": "b8",
          "name": "--obs_out_format",
          "required": false,
          "short": null,
          "value": "OBS_OUT_FORMAT"
        },
        {
          "allowed_values": null,
          "default": null,
          "name": "--verify_only",
          "required": false,
          "short": null,
          "value": null
        },
        {
          "allowed_values": null,
          "default": null,
          "name": "--help",
          "required": false,
          "short": "-h",
          "value": null
        }
      ],
      "required_options": [
        "--circuit",
        "--in"
      ],
      "usage": "rstim unpack_samples [OPTIONS] --circuit <CIRCUIT> --in <IN>"
    }
  ],
  "schema_version": 1
}
```
