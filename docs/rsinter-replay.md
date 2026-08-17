# `rsinter replay`

`rsinter replay` runs a RustQEC decoder against a frozen detector dataset. It
is deliberately separate from sampling and scoring: the command reads only a
detector error model and detector rows, and cannot inspect observable answers.

```sh
cargo run --release -p rsinter -- replay \
  --dem model.dem \
  --dets detectors.b8 \
  --decoder rbposd \
  --decoder-config decoder.toml \
  --predictions-out predictions.b8 \
  --stats-out stats.json \
  --batch-size 65536
```

The DEM supplies the detector and observable counts. `detectors.b8` and
`predictions.b8` use Stim's b8 convention: one LSB-first, byte-aligned row per
shot. Unused high bits in each row's last byte must be zero. The input length
normally determines the shot count; `--shots N` additionally validates it and
is required for a DEM with zero detectors.

The decoder is compiled once, then the detector file is streamed in batches.
Both outputs are first written to temporary files in their destination
directories and atomically installed only after validation and decoding
succeed. Input, config, and output paths must all be distinct.

`stats.json` records the normalized decoder configuration, dimensions, batch
count, compile/decode time, throughput, byte counts, and SHA-256 digests of the
DEM, detector input, and predictions. Timing fields vary by machine; the
predictions and content digests are deterministic for a deterministic decoder.

## Decoders and features

| `--decoder` | Cargo feature | Config keys |
| --- | --- | --- |
| `rbposd` | `rbposd-runner` | `bp_method`, `bp_schedule`, `max_bp_iterations`, `early_stop`, `osd_method`, `osd_order` |
| `rbplsd` | `rbposd-runner` | `bp_method`, `bp_schedule`, `max_bp_iterations`, `early_stop`, `lsd_method`, `lsd_order` |
| `rmatching` | `rmatching-runner` | none |
| `rilpqec` | `ilp-runner` | `backend`, `time_limit_s`, `mip_gap`, `threads`, `verbose` |

The default `full` feature enables all four. A smaller build keeps the replay
command but reports a clear error if the selected decoder feature is absent.
Configuration files are TOML and reject unknown or decoder-inappropriate keys.
Omit `--decoder-config` to use the normalized defaults.

Example BP+OSD configuration:

```toml
bp_method = "minimum_sum"       # or "product_sum"
bp_schedule = "parallel"        # or "serial"
max_bp_iterations = 30
early_stop = true
osd_method = "osd0"             # combination_sweep or ldpc_osd_cs
osd_order = 0
```

Example BP+LSD configuration:

```toml
bp_method = "minimum_sum"
bp_schedule = "parallel"
max_bp_iterations = 30
early_stop = true
lsd_method = "localized_statistics"
lsd_order = 0                    # 0 or 1
```

`rmatching` accepts graphlike DEM components only. An error component touching
more than two detectors is rejected instead of being silently approximated.
