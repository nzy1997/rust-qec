# Performance Parity Benchmark Guide

This document tracks the rerun workflow for the performance parity foundation.

## Full Harness

Run the comparison harness with:

```sh
cargo run -p rstim --example performance_parity_foundation
```

Set `RSTIM_TEST_STIM` if the Stim binary is not on `PATH`:

```sh
RSTIM_TEST_STIM=/absolute/path/to/stim cargo run -p rstim --example performance_parity_foundation
```

The harness emits one JSON line per `(case, tool_variant)` pair.

Current variants:

- `stim-cli`
- `rstim-interpreted`
- `rstim-compiled`
- `rstim-analyzer-flattened`
- `rstim-analyzer-compiled`

Each JSON line includes:

- `case_label`
- `tool_variant`
- `workload`
- `qubits`
- `measurements`
- `detectors`
- `observables`
- `repeat_depth`
- `repeat_count`
- `shots`
- `wall_time_ns`
- `peak_memory_bytes`

On Unix platforms, `peak_memory_bytes` is collected with
`getrusage(RUSAGE_SELF)`.

## Acceptance Workflow

Run these commands before calling the milestone complete:

```sh
cargo test -p rstim --test perf_harness --test compiled_circuit --test compiled_routing --test compiled_sampler --test compiled_analyzer
cargo run -p rstim --example performance_parity_foundation > /tmp/rstim-performance-parity.jsonl
```

Review the JSON lines and confirm:

1. sample and detect show a visible win for `rstim-compiled` over `rstim-interpreted`
2. the repeat-focused analyze case uses `rstim-analyzer-compiled`
3. protection cases still complete successfully
4. semantic regression suites remain green
