# Issue 459 Reference Build Evidence Design

Issue: #459 Publish packed reference-sampling phase evidence
Date: 2026-07-11

## Context

Issue #458 routed supported noiseless reference construction through the packed
inverse backend and proved the canonical d11/r100 fixture returns 12,121 false
reference bits. Issue #454 published a checked long-lived-worker evidence
bundle for compiled steady-state sampling, but that timing still includes more
than the reference construction phase. Issue #459 needs a phase-scoped
reference-build artifact: both Stim and rstim parse the fixture once, build a
reference sample nine times, emit identical packed bytes, and time only the
internal reference construction plus materialization of the packed bytes.

This Agent Desk run is non-interactive, so the Standing Answer Policy resolves
the Superpowers gates:

- Visual companion: not used because the work is a backend benchmark artifact.
- Clarifying questions: answered from issue #459, #454, #458, and the existing
  benchmark/checker patterns.
- Design approval: accepted automatically because the issue gives exact paths,
  protocol name, variants, counters, required hashes, and verification commands.
- Spec review: this document is approved for planning after checking for
  placeholders, contradictions, ambiguity, and unrelated scope.

## Alternatives Considered

1. Reuse the compiled-steady binary frame protocol and add a reference-only
   command. This keeps transport code similar, but the issue explicitly asks for
   JSONL protocol `reference-build-v1`.
2. Add one runner that imports both Python Stim and Rust rstim logic in process.
   This would be simpler, but it would not prove symmetric long-lived-worker
   behavior or exclude process startup and IPC from the internal timer.
3. Add a JSONL runner with separate long-lived Stim and rstim workers. This is
   the chosen approach because it matches the issue interface and keeps the
   timer inside each worker around only reference construction and final packed
   byte materialization.

## Chosen Design

Add `benchmarks/rstim_vs_stim_simulator/run_reference_build_benchmark.py` as the
release runner. It accepts the issue-required CLI, validates the canonical
fixture and `cases.full.toml` hash, launches both canonical workers, sends one
`load` request per worker, then sends two warmup and seven measured
`build_reference` requests. The runner records only build responses in
`raw.jsonl`, so the raw file has exactly 18 rows.

Use JSONL messages with `protocol="reference-build-v1"`. A load request
contains the fixture path. A build request contains a monotonic request ID. Each
build response returns `elapsed_ns`, base64 packed bytes, byte SHA-256, backend,
measurement-bit count, packed-byte count, `parse_count`, and
`reference_build_count`. The runner validates every response before writing
artifacts.

The Stim worker parses the fixture once with `stim.Circuit`, requires
`stim.__version__ == "1.15.0"`, then times `circuit.reference_sample()` plus
little-endian b8 packing with `numpy.packbits(..., bitorder="little")`. The
timer stops before hashing, base64, and JSON serialization.

The rstim worker parses once with `parse_lines`, then times
`build_reference_sample_with_decision` plus custom little-endian b8 packing. It
requires the packed reference decision and reports `backend="packed_inverse"`.
If the fixture ever falls back to legacy, the worker returns an error response
instead of publishing ambiguous evidence.

## Artifact Schema

Publish under
`benchmarks/rstim_vs_stim_simulator/results/reference-build-release/`:

- `raw.jsonl`: 18 build rows, variants `stim-reference-b8` and
  `rstim-packed-reference-b8`, phases `warmup` and `measured`, rounds 0 through
  8, packed bytes, digests, timer scope, backend, and counters.
- `summary.json`: derived from the seven measured rows per variant with count,
  min/median/max elapsed time, digest, backend, measurement bits, packed bytes,
  parse count, and final reference-build count.
- `report.md`: a small markdown table rendered only from `summary.json`.
- `environment.json`: release profile, exact runner and worker argv, no-seed
  policy, git commit and dirty state captured before artifact writing, fixture
  and manifest paths/hashes, Stim version, executable/module/binary hashes,
  rustc/cargo/Python versions, OS/CPU, rounds, protocol, and timer scope.
- `artifact-sha256.json`: lowercase SHA-256 digests for the other four files.

The runner writes `artifact-sha256.json` after the other files so a fresh `/tmp`
run is immediately checker-valid.

## Checker Behavior

Add `tools/check_rstim_vs_stim_reference_build_evidence.py`. It validates in
this order:

1. Required files exist and no extra bundle files are present.
2. `raw.jsonl` parses as exactly 18 build rows with the two required variants.
3. Each row decodes base64 bytes, recomputes the byte SHA-256, checks 12,121
   bits, 1,516 bytes, digest
   `d95f3eacd05c1ca0d3a90e4a48e1d68b7ef5f2d817da11121ba4b77454b24d3d`,
   timer scope `reference_build_only`, `parse_count == 1`, and per-variant
   `reference_build_count == 1..9`.
4. rstim rows require `backend="packed_inverse"`.
5. `summary.json` is regenerated from measured raw rows and must match
   byte-for-byte.
6. `report.md` is regenerated from `summary.json` and must match
   byte-for-byte.
7. `environment.json` is cross-checked against raw-derived values, canonical
   fixture/manifest hashes, exact round counts, protocol, timer scope, Stim
   `1.15.0`, worker argv, executable paths, and path hashes.
8. Only after semantic, summary, report, and environment checks pass,
   `artifact-sha256.json` is validated.

The negative controls target semantic validation before hash validation:
changed decoded bytes, mismatched digests, legacy rstim backend, parsing in the
timer scope, `parse_count != 1`, missing final `reference_build_count == 9`,
non-recomputable summary, and missing hash manifest.

## Testing

Add runner tests in
`benchmarks/rstim_vs_stim_simulator/tests/test_run_reference_build_benchmark.py`
with fake JSONL workers that prove the runner emits 18 raw rows, summary,
environment, report, and artifact hashes, and that the default/canonical worker
argvs match the issue interface.

Add checker tests in `tools/test_check_rstim_vs_stim_reference_build_evidence.py`
that build a synthetic valid bundle, then mutate it for each required negative
control.

Add a Rust integration test for `rstim_reference_build_worker` that builds the
debug worker, loads `X 0; M 0`, verifies one packed byte `0x01`, confirms
`parse_count == 1`, and observes `reference_build_count` increasing across
requests.

Final verification:

```sh
cargo build --release -p rstim --bin rstim_reference_build_worker
rm -rf /tmp/rstim-reference-build
python3 -m benchmarks.rstim_vs_stim_simulator.run_reference_build_benchmark \
  --fixture benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim \
  --manifest benchmarks/rstim_vs_stim_simulator/cases.full.toml \
  --stim-python "$(command -v python3)" \
  --rstim-worker target/release/rstim_reference_build_worker \
  --warmup-rounds 2 --measure-rounds 7 \
  --out-dir /tmp/rstim-reference-build
python3 tools/check_rstim_vs_stim_reference_build_evidence.py --dir /tmp/rstim-reference-build
python3 tools/check_rstim_vs_stim_reference_build_evidence.py \
  --dir benchmarks/rstim_vs_stim_simulator/results/reference-build-release
python3 -m unittest \
  benchmarks.rstim_vs_stim_simulator.tests.test_run_reference_build_benchmark \
  tools.test_check_rstim_vs_stim_reference_build_evidence -q
cargo test
```

## Out Of Scope

This design does not time parsing, frame construction, IPC, shot sampling, or
base64/hashing/JSON serialization. It does not update site metadata and does
not claim a speed ratio.

## Self-Review

- No placeholders remain.
- The protocol, variants, paths, counts, digest, and timer scope match issue
  #459 exactly.
- The checker validates semantics before artifact hashes.
- The release bundle is generated by the same runner that creates `/tmp`
  verification bundles.
- Site metadata and speed-ratio claims remain out of scope.
