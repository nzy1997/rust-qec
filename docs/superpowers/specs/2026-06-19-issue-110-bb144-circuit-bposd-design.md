# Issue #110 BB144 Circuit-Level BP-OSD Memory Design

Date: 2026-06-19
Status: Non-interactive Agent Desk design, auto-approved by standing policy
Scope: GitHub issue #110, circuit-level bivariate-bicycle memory reproduction path

## Context

Issue #110 asks `rstim` to reproduce the upstream circuit-level BP-OSD memory
simulation from `sbravyi/BivariateBicycleCodes`, starting with the default
`[[144,12,12]]` bivariate-bicycle code at `p = 0.003`, `num_cycles = 12`, and
`num_trials = 50_000`.

The upstream reference has two stages:

- `decoder_setup.py` builds the BB code, the 7-CNOT-round syndrome extraction
  cycle, single-fault effective decoder matrices, logical-effect augmented
  matrices, and grouped single-fault channel probabilities.
- `decoder_run.py` samples noisy circuit-level memory trials, appends two
  noiseless syndrome cycles, decodes Z faults first using X-check syndrome
  history and then X faults using Z-check syndrome history, and emits one line
  with `physical_error_rate`, `num_cycles`, `num_trials`,
  `num_failed_trials`.

The repository already has `rsinter` for benchmark-style runs and `rbposd` for
BP+OSD decoding. Prior issues added BB72 CSS fixtures and BP+OSD parameter
provenance, but those paths are code-capacity or DEM-oriented rather than the
upstream circuit-level memory simulation requested here.

## Goals

- Add a usable reproduction path for the upstream default BB144 circuit-level
  memory simulation.
- Construct the same fixed BB code with `ell = 12`, `m = 6`, `n = 144`,
  `k = 12`, `A = x^3 + y + y^2`, and `B = y^3 + x + x^2`.
- Build the same 288-qubit named circuit state and 7-round syndrome cycle with
  `sX = [idle, 1, 4, 3, 5, 0, 2]` and `sZ = [3, 5, 0, 1, 2, 4, idle]`.
- Implement the same circuit-level noise sampler for IDLE, CNOT, PrepX,
  PrepZ, MeasX, and MeasZ.
- Build effective X and Z decoder models by propagating all single-location
  X-like and Z-like marginal faults through `num_cycles + 2` cycles, with the
  final two cycles noiseless.
- Group identical syndrome/logical columns and sum their single-fault
  probabilities.
- Decode with `rbposd::BpOsdDecoder` using min-sum BP, `max_iter = 10000`,
  OSD order 7, and per-column channel probabilities.
- Expose a CLI that defaults to the upstream data point but accepts smaller
  trial counts, cycle counts, and deterministic seeds for smoke testing.
- Add fast Rust tests for code shape, cycle schedule, effective-model
  dimensions, zero-failure no-fault smoke behavior, and CLI output format.

## Non-Goals

- Do not make CI run 50,000 circuit-level trials.
- Do not add a general parameterized bivariate-bicycle family API.
- Do not claim bit-for-bit parity with upstream random output; upstream does
  not set a seed.
- Do not change the core `rbposd` algorithm.
- Do not replace the existing `rsinter` DEM benchmark stack.

## Approach Options

### Recommended: Native `rsinter` Reproduction Module

Add a focused `rsinter::bb_circuit_memory` module plus a CLI subcommand. The
module owns the upstream-equivalent BB144 construction, schedule, effective
decoder model, sampler, and BP+OSD trial loop. The CLI prints the upstream
four-column result line and defaults to the upstream configuration.

This is the best fit because `rsinter` is already the workspace benchmark and
sampling harness, while `rbposd` already provides the decoder. The module can
be tested directly without disturbing the existing DEM runner abstractions.

### Alternative: Extend Existing TOML Benchmark Specs

Add a new `input_type = "bb_circuit_memory"` to the current `rsinter bench run`
TOML path. This would reuse result JSONL plumbing, but it would force a circuit
history decoder with two separate effective matrices into a runner interface
that currently assumes one DEM decoder per point.

This is more invasive and would make the issue harder to land safely.

### Alternative: Port The Upstream Python Scripts

Add Python scripts under `benchmarks/` that call the upstream `ldpc` package.
This would be closer to the source line-by-line, but it would not reproduce the
simulation inside the Rust workspace and would add dependency/runtime ambiguity.

This does not meet the issue's intent.

## Design

### Public Interface

Add a new `rsinter` CLI subcommand:

```text
rsinter bb-circuit-bposd-memory \
  --physical-error-rate 0.003 \
  --num-cycles 12 \
  --num-trials 50000
```

The default values are the upstream defaults, so the same command can be run
without flags:

```text
rsinter bb-circuit-bposd-memory
```

The command prints exactly one tab-separated line:

```text
0.003	12	50000	<num_failed_trials>
```

For smoke tests and local checks, the command also supports:

- `--seed <u64>` for deterministic Rust sampling. Omitting it uses entropy,
  matching the upstream script's unseeded behavior.
- `--max-bp-iterations <usize>` with default `10000`.
- `--osd-order <usize>` with default `7`.

### Code And Circuit Model

The new module constructs a fixed `BivariateBicycleCode` with dense binary
`hx` and `hz`, sparse Tanner neighbors, named qubit index ranges, and pure CSS
logical bases:

- X logical rows are selected from `nullspace(hz)` modulo `rowspace(hx)`.
- Z logical rows are selected from `nullspace(hx)` modulo `rowspace(hz)`.

The 288-qubit circuit state uses the upstream linear order:

1. `Xcheck[0..71]`
2. `data_left[0..71]`
3. `data_right[0..71]`
4. `Zcheck[0..71]`

The syndrome cycle stores operations as a compact enum. The schedule and idle
placement match the issue body and upstream scripts exactly.

### Effective Decoder Model

For each noisy operation in `num_cycles * cycle`, the module creates X-like
and Z-like marginal faults with the same probabilities as upstream:

- Measurement and preparation marginal fault: `p`
- Idle X/Z marginal fault: `2p/3`
- CNOT single-qubit control/target X/Z marginal fault: `4p/15`
- CNOT correlated `XX` or `ZZ` marginal fault: `4p/15`

Each single-fault candidate is propagated through the remaining noisy memory
cycles plus two noiseless cycles. The propagated syndrome history is converted
to consecutive-round check differences. The logical vector is appended for the
augmented validation matrix, and identical syndrome/logical columns are grouped
with summed probabilities.

The decoder matrix handed to `rbposd` uses only the syndrome-history rows. The
augmented matrix remains available to map a decoded column vector back to its
predicted logical effect, mirroring upstream `HX` and `HZ`.

### Trial Sampling And Decoding

The trial sampler walks `num_cycles * cycle` and inserts the same stochastic
faults as upstream:

- IDLE chooses uniformly from `X`, `Y`, `Z`.
- CNOT chooses uniformly from the 15 non-identity two-qubit Pauli errors in
  the upstream order.
- PrepX/MeasX insert Z faults.
- PrepZ/MeasZ insert X faults.

For each trial, the simulation decodes Z faults first with the X-check
syndrome history and the Z-fault effective model. If that logical prediction
matches, it decodes X faults with the Z-check syndrome history and the X-fault
effective model. The trial fails if either logical prediction differs from the
actual final logical vector.

### Error Handling

The CLI rejects invalid probabilities, zero trial counts, zero cycle counts,
zero BP iteration limits, and empty effective decoder probability vectors
before running. Internal construction errors return `String` messages because
the `rsinter` CLI currently uses `Result<(), String>` throughout.

### Testing

Fast tests cover:

- BB144 shape: `n2 = 72`, `n = 144`, `k = 12`, 72 X checks, 72 Z checks, and
  row weight 6 for both CSS matrices.
- Cycle shape: 1440 operations, 864 CNOTs, 288 IDLEs, 72 each of PrepX, PrepZ,
  MeasX, and MeasZ.
- Effective model construction on a one-cycle smoke configuration, including
  syndrome row count `72 * (1 + 2)` and non-empty grouped probability columns.
- A no-fault smoke run with very small positive physical error rate and a fixed
  seed, using one cycle and two trials, returns zero failed trials.
- CLI smoke output contains four tab-separated fields.

The required full verification remains `cargo test`.

## Approval

The run is non-interactive. The standing answer policy chooses the recommended
native `rsinter` module approach because it is the least invasive design that
keeps the reproduction in the Rust workspace and reuses the existing BP+OSD
decoder.
