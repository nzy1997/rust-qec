# Performance Parity Benchmark Guide

This document tracks the rerun workflow for the performance parity evidence
gate.

## Full CI-Equivalent Pipeline

Run the full local pipeline with:

```sh
cargo run -p rstim --bin rstim -- perf ci --out-dir /tmp/rstim-perf-artifacts
```

This writes:

- `/tmp/rstim-perf-artifacts/raw.jsonl`
- `/tmp/rstim-perf-artifacts/summary.json`
- `/tmp/rstim-perf-artifacts/report.md`

If the Stim binary is not on `PATH`, set `RSTIM_TEST_STIM`:

```sh
RSTIM_TEST_STIM=/absolute/path/to/stim cargo run -p rstim --bin rstim -- perf ci --out-dir /tmp/rstim-perf-artifacts
```

## Individual Steps

```sh
cargo run -p rstim --bin rstim -- perf run --out /tmp/raw.jsonl
cargo run -p rstim --bin rstim -- perf summarize --in /tmp/raw.jsonl --out /tmp/summary.json
cargo run -p rstim --bin rstim -- perf gate --in /tmp/summary.json
cargo run -p rstim --bin rstim -- perf report --in /tmp/summary.json --out /tmp/report.md
```

The gate uses same-run median timing and currently enforces:

- `rstim-compiled / rstim-interpreted <= 1.10`
- `rstim-analyzer-compiled / rstim-analyzer-flattened <= 1.10`
- fallback cases must not produce the compiled variant
