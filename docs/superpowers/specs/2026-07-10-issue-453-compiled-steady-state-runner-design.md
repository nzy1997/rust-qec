# Issue 453 Compiled Steady-State Runner Design

## Context

Issue #453 needs a benchmark path that compares Stim and rstim after one-time
setup is complete. The existing fair manifest from #449 provides the canonical
fixture, shot count, measurement count, byte count, and Stim version
requirements. The reusable sampler from #452 provides the Rust API needed to
compile once, build the reference sample once, and sample many times while
retaining a seeded RNG.

The runner must produce machine-checkable lifecycle records. Sample timing must
start immediately before writing a complete `SAMPLE` frame and must stop only
after the complete `RESULT` frame has been read, so delayed response bytes are
included in the measurement. Startup, parsing, compilation, reference
construction, RNG initialization, and shutdown are outside the sample timing
window.

## Approaches Considered

1. Add one focused Python runner, one Stim Python worker, and one rstim Rust
   worker using the issue's binary frame protocol. This is selected because it
   gives both variants the same long-lived request lifecycle while keeping the
   benchmark code local to the `rstim_vs_stim_simulator` harness.
2. Build a general reusable protocol library first, then have the runner and
   workers import it. This would reduce duplication, but it broadens the change
   beyond the single benchmark contract and introduces an API that no other
   caller currently needs.
3. Drive both tools through existing CLI commands and cache state externally.
   This cannot satisfy the issue because CLI process boundaries include startup
   and one-time setup costs, and the Rust compiled sampler state cannot be kept
   alive across separate CLI invocations.

The design uses option 1.

## Design

Add `benchmarks/rstim_vs_stim_simulator/run_compiled_steady.py` as the benchmark
orchestrator. It will validate the #449 manifest before starting canonical
timing, require `stim_version == "1.15.0"`, build the requested rstim worker
binary for the selected profile, run the known-answer preflight for each worker
on `X 0\nM 0\n`, then run one canonical worker session per variant.

The runner will implement the binary frame contract directly:

- frame header: one byte frame type plus unsigned 64-bit little-endian payload
  length;
- `READY` and `FINAL` payloads: JSON telemetry containing variant, compile
  count, reference-build count, sample-call count, fixture SHA-256,
  measurement count, and bytes per shot;
- `SAMPLE` payload: JSON request ID and shot count;
- `RESULT` payload: unsigned 64-bit request ID, unsigned 64-bit cumulative
  sample-call count, then raw shot-major `b8` bytes;
- `ERROR` payload: UTF-8 diagnostic text;
- `STOP` payload: empty.

The canonical run will write exactly one ready, nine sample, and one final
record per variant to `raw.jsonl`. The first two sample records are warmups and
the next seven are measured records. `summary.json` is derived only from the
fourteen measured sample records. `environment.json` records the #450-style
provenance fields plus exact worker argv, Python executable and hash, loaded
Stim extension path and hash, rstim worker path and hash, protocol version, and
`seed_policy = "seed_once_then_advance_across_9_calls"`.

Add `benchmarks/rstim_vs_stim_simulator/workers/stim_compiled_steady.py`. The
worker will require `stim==1.15.0`, load the input once with `stim.Circuit(...)`,
compile one sampler with `compile_sampler(seed=args.seed)`, send `READY`, and
for each `SAMPLE` call `sampler.sample(shots=shots, bit_packed=True).tobytes(order="C")`.
It will maintain a cumulative sample-call count and send `FINAL` after `STOP`.

Add `rstim/src/bin/rstim_compiled_steady_worker.rs`. The worker will parse the
fixture once, compile one `CompiledMeasurementSampler` with
`ReferenceSampleMode::SimulateNoiseless`, retain one `StdRng::seed_from_u64`,
and on each `SAMPLE` call sample in `SampleOutputMode::MeasurementsOnly`. It
will serialize measurements through the existing `write_shots_b8` helper so
the output layout matches the CLI and Stim worker.

All worker failures become `ERROR` frames when possible. The runner treats
unexpected EOF, invalid telemetry, wrong output byte count, nonzero worker exit,
or known-answer output other than `0x01` as fatal and must not write
`summary.json` for rejected runs.

## Testing

Add `benchmarks/rstim_vs_stim_simulator/tests/test_run_compiled_steady.py` with
focused tests around the contract:

- fake workers verify the raw lifecycle shape, measured-only summary, worker
  argv/provenance recording, and acceptance print;
- a fake worker that delays its last result byte by at least 150 ms proves the
  timing window includes complete result-frame receipt;
- a fake worker that sends valid `FINAL` telemetry and exits nonzero proves
  `summary.json` is not written;
- a fake worker returning `0x00` for the known-answer preflight proves canonical
  timing is skipped.

Run the issue command:

- `rm -rf /tmp/rstim-compiled-steady`
- `python3 -m benchmarks.rstim_vs_stim_simulator.run_compiled_steady --manifest benchmarks/rstim_vs_stim_simulator/fair_cli_cases.toml --case stim_surface_d11_r100 --profile release --warmup-rounds 2 --measure-rounds 7 --seed 0 --out-dir /tmp/rstim-compiled-steady`
- `python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_run_compiled_steady -q`
- `cargo test`

## Scope

This change is limited to the compiled steady-state benchmark runner, its two
workers, focused tests, and the design/plan docs required by the local
workflow. It does not publish benchmark artifacts, set a speed-ratio gate, or
change rstim's existing CLI behavior.

## Self-Review

- No placeholders remain.
- The selected approach excludes startup and setup costs from sample timings.
- The known-answer preflight uses `X 0\nM 0\n` and requires `0x01`.
- The Rust worker uses the #452 reusable sampler and keeps one seeded RNG.
- Summary derivation excludes the two warmup samples per variant.
- Negative controls cover delayed final result bytes, nonzero worker exit after
  final telemetry, and failed known-answer output.
