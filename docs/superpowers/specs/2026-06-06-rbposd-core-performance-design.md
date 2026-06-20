# rbposd Core Performance Design

Date: 2026-06-06
Status: Proposed
Scope: `rbposd` core decoder internals, correctness-preserving performance refactor, and benchmark-facing performance validation

## Summary

This design targets the main performance gap between the in-tree Rust BPOSD
decoder and the Python `ldpc` package on the repository's shared surface-code
benchmark.

An evidence update from issue #100 supersedes the original benchmark-gap
summary in this design. The tracked artifact at
`benchmarks/surface_decoder_compare/results/full/results.csv` is the source for
the current checked-in full-tier comparison rows. It is evidence of the
checked-in benchmark artifact, not a fresh claim about current local machine speed.

In the tracked checked-in full-tier native rows, `rbposd` has lower
`decode_us_per_shot` than `ldpc` for every paired `distance in {3, 5}` and
`p in {0.002, 0.005, 0.010}` case:

| distance | rounds | p | ldpc decode_us_per_shot | rbposd decode_us_per_shot | rbposd / ldpc |
| --- | --- | --- | ---: | ---: | ---: |
| 3 | 3 | 0.002 | 9.28949949957314 | 5.533358 | 0.596 |
| 3 | 3 | 0.005 | 15.255653700023686 | 9.888312299999999 | 0.648 |
| 3 | 3 | 0.010 | 22.875337890445515 | 18.083490234375002 | 0.791 |
| 5 | 5 | 0.002 | 194.81863700011675 | 128.28873740000003 | 0.659 |
| 5 | 5 | 0.005 | 386.04600850012497 | 322.0114498 | 0.834 |
| 5 | 5 | 0.010 | 737.693568638826 | 639.9339513020834 | 0.867 |

The table should not be read as a machine-independent speed promise or as proof
that all `rbposd` configurations are faster than upstream `ldpc`. It only says
that the tracked native full-tier comparison artifact no longer supports the
old claim that default `rbposd` trails `ldpc` on every checked-in case.

The LSD and BP-option milestone work also changes the alignment story. The repo
now has LSD execution and result-row coverage, BP method/schedule configuration,
behavioral teeth for `product_sum` plus `serial`, and checked-in `rsinter`
benchmark spec entries named `rbposd_lsd_order1` and
`rbposd_product_sum_serial`.

Those milestones do not mean the tracked comparison CSV covers every expanded
decoder surface. `benchmarks/surface_decoder_compare/results/full/results.csv`
does not contain checked-in timing rows for `rbposd_lsd_order1` or
`rbposd_product_sum_serial`, and the implemented option surface is still a
narrow subset of upstream `ldpc` rather than full feature parity.

The goal of this work is to remove structural inefficiencies from the
`rbposd` core decode path without changing correctness semantics, benchmark
measurement rules, or the high-level public API shape.

The chosen direction is:

- keep `BpOsdDecoder::new(...)` and `decode(&Syndrome)` as the primary public
  entrypoints
- preserve the current algorithm contract:
  `minimum-sum BP + parallel schedule + OSD_0`
- move repeated decode-time allocation and matrix rebuilding work into
  decoder construction or reusable internal workspaces
- replace nested `Vec<Vec<_>>` hot paths with flat, precompiled adjacency and
  scratch buffers
- keep the optimization centered in `rbposd`, not in benchmark-specific or
  `rsinter`-only fast paths

## Goals

- Reduce `rbposd` `decode_us_per_shot` in
  `benchmarks/surface_decoder_compare` substantially enough that the dominant
  remaining cost is algorithmic work instead of avoidable allocation,
  copying, sorting, and matrix reconstruction.
- Prioritize acceleration of the general-purpose core
  `BpOsdDecoder::decode` path instead of adding benchmark-only shortcuts.
- Preserve existing correctness expectations for `BpOsdDecoder`,
  `DecodeResult`, `OSD_0` tie-breaking, and `rsinter` integration behavior.
- Keep the public API usable for current call sites while allowing major
  internal refactoring.
- Prepare the internals so a later batch or external-workspace API can be
  added without another large rewrite.

## Non-Goals

- Do not change the benchmark's timing boundary or comparison methodology.
- Do not switch BP rules, add `product_sum`, or expand beyond the current
  `OSD_0` path in this work.
- Do not introduce a benchmark-specific decode shortcut that bypasses the
  general `rbposd` core.
- Do not make a public batch decode API part of this delivery.
- Do not use this work to broaden `rbposd` into new quantum-specific surface
  APIs.
- Do not accept any logical-correctness regression in exchange for speed.

## Current State

The current implementation is functionally correct for the checked-in parity
fixtures and integration tests, but the internal execution model is expensive
per decode.

### Current BP hot-path costs

The BP path currently:

- rebuilds `v_to_c` as `Vec<Vec<f64>>` on every decode
- rebuilds `c_to_v` as `Vec<Vec<f64>>` on every decode
- allocates new vectors for `posterior_llrs`, `incoming`, and hard decisions
  during decode
- recomputes residual syndrome by calling `pcm.multiply(&hard_decision)` and
  materializing a new `Syndrome` each iteration

This makes the per-shot cost scale with a large amount of allocation and
boolean-vector churn, not just message-passing work.

### Current OSD hot-path costs

The OSD path currently:

- recomputes the residual syndrome through a new matrix multiply
- sorts columns by reliability for every OSD invocation
- converts the parity-check matrix into a dense row-major boolean matrix for
  every solve
- runs a generic elimination path over freshly rebuilt dense data

This is especially costly on the larger `distance=5` surface-code cases where
the benchmark gap widens most sharply.

### Current correctness baseline

The repository already contains a useful correctness gate:

- `rbposd/tests/reference.rs`
- parity fixture coverage in `rbposd/tests/fixtures/parity`
- `rsinter/tests/decode_rbposd.rs`
- surface decoder compare logical error rate outputs

This means the performance work can be aggressive internally while staying
constrained by repository-owned behavior checks.

## Decision Summary

The refactor should keep the public decoder contract stable while replacing
the internal execution model with precompiled graph data and reusable
workspaces.

The new design is organized around three internal components:

1. `CompiledGraph`
2. `BpWorkspace`
3. `OsdWorkspace`

Together they move static structure out of `decode()`, flatten the numerical
hot path, and eliminate repeated rebuilding of matrix and scratch state.

## Alternatives Considered

### 1. Light allocation cleanup only

This option would keep the present module structure and primarily trim obvious
temporary allocations.

Benefits:

- lowest implementation risk
- smallest diff
- easiest to land quickly

Costs:

- unlikely to close a `40x` to `130x` performance gap
- leaves the nested adjacency and dense OSD rebuild costs intact
- likely turns a structural problem into a smaller structural problem

This is not the recommended option.

### 2. Core execution-model rewrite with stable public API

This option rewrites the internal BP and OSD execution paths around compiled
adjacency and reusable scratch while keeping the public decoder shape intact.

Benefits:

- directly attacks the largest measured bottlenecks
- aligns with the goal of making `BpOsdDecoder::decode` itself fast
- benefits both benchmark and non-benchmark callers
- keeps `rsinter` changes narrow

Costs:

- requires substantial internal refactoring
- needs careful correctness protection around workspace reuse and OSD
  tie-breaking

This is the recommended option.

### 3. Benchmark-facing batch fast path first

This option would optimize primarily around `rsinter` and benchmark call
patterns, possibly exposing a batch-oriented execution path before fixing the
single-decode core.

Benefits:

- highest near-term benchmark upside
- opens a path to amortize setup across many shots

Costs:

- conflicts with the chosen priority of accelerating the general core decode
  path first
- risks adding an optimization that benchmark users see but ordinary
  `BpOsdDecoder` callers do not
- can hide rather than remove the current inner-loop inefficiencies

This is not the recommended first step.

## Recommended Architecture

### Component 1: `CompiledGraph`

`CompiledGraph` is an internal representation derived once from
`ParityCheckMatrix` when the decoder is constructed.

Responsibilities:

- store flattened edge-oriented adjacency for checks and bits
- provide fast lookup from check ranges to edges and bit ranges to edges
- retain enough structure for syndrome checks and OSD support without
  repeatedly walking nested vectors
- expose internal views needed by BP and OSD without changing the public matrix
  API

Required properties:

- immutable after construction
- cheap to reuse across many decode calls
- avoids `Vec<Vec<_>>` traversal in numerical loops

Representative compiled data:

- `edge_bit_ids: Vec<usize>`
- `edge_check_ids: Vec<usize>`
- `check_edge_offsets: Vec<usize>`
- `bit_edge_offsets: Vec<usize>`
- per-column sparse views for OSD support

The exact field layout may vary, but the design requirement is that the decode
loop operates over contiguous edge buffers instead of rebuilding per-check
message vectors on demand.

### Component 2: `BpWorkspace`

`BpWorkspace` is reusable scratch state for one decode execution.

Responsibilities:

- hold all temporary BP buffers needed across iterations
- permit repeated decode calls without rebuilding or reallocating the main
  message arrays
- support future internal batch execution without changing the external API

Required buffers:

- `v_to_c`
- `c_to_v`
- `posterior_llr`
- `incoming_llr_sum` or equivalent bit-accumulation buffer
- `hard_decision_bits`
- residual tracking state such as `residual_bits` or
  `unsatisfied_check_flags`
- reliability output buffer for OSD handoff

The initial public API can still be `&self -> DecodeResult`; the workspace may
be created per call internally at first or managed behind interior mutability
later. The design requirement is that the scratch model is explicit and
reusable, even if the first landing keeps the concurrency story conservative.

### Component 3: `OsdWorkspace`

`OsdWorkspace` captures reusable structures for the `OSD_0` fallback.

Responsibilities:

- avoid repeated dense-matrix materialization from `ParityCheckMatrix`
- reuse ordering and elimination buffers across OSD invocations
- preserve existing tie-breaking semantics for equal reliability cases

Expected reusable state:

- column-order scratch buffer
- RHS scratch buffer
- elimination workspace derived from the parity-check matrix
- temporary correction/residual storage

This workspace should be coupled to the same decoder compilation lifetime as
`CompiledGraph`, not rebuilt from scratch for each OSD use.

## Decode Flow

The refactored `decode()` path should follow this flow:

1. Validate syndrome width.
2. Reinitialize a reusable BP workspace for the current syndrome.
3. Run `minimum-sum` BP over flat edge buffers.
4. Track hard-decision residual state without materializing a fresh syndrome on
   every iteration.
5. If BP reaches zero residual, return directly without touching OSD.
6. If BP does not reach zero residual, pass the minimal needed state to
   `OsdWorkspace`.
7. Run `OSD_0` using reusable ordering and elimination buffers.
8. Return a `DecodeResult` with the existing public semantics.

The main architectural requirement is that only syndrome-dependent data
changes per shot. Static graph structure, elimination structure, and major
scratch buffers should survive across decode calls.

## BP Hot-Path Design

The BP redesign should specifically remove the current repeated work.

### Message storage

- Store check-to-bit and bit-to-check messages in flat edge-indexed arrays.
- Use the compiled offsets to iterate all edges of a check or bit without
  allocating nested vectors.

### Bit aggregation

- Replace iteration-local `incoming = vec![0.0; n]` style buffers with
  preallocated accumulation buffers in `BpWorkspace`.
- Clear or overwrite buffers in place.

### Residual tracking

- Stop calling `pcm.multiply(&hard_decision)` each iteration.
- Compute or update residual state directly from the current hard decision and
  compiled adjacency.
- Prefer maintaining unsatisfied-check flags or equivalent incremental state so
  the decoder can test convergence without building a new `Syndrome`.

### Hard decision and reliability output

- Maintain hard decision bits in reusable storage.
- Generate the reliability values needed by OSD from the already available
  posterior state instead of constructing extra vectors when avoidable.

## OSD Hot-Path Design

The OSD redesign keeps the current algorithm contract but changes how the work
is staged.

### Ordering

- Reuse a dedicated column-order scratch buffer.
- Preserve the current stable ordering semantics for equal scores.
- Limit per-call work to rewriting score-dependent ordering data, not
  reconstructing auxiliary matrix structure.

### Matrix representation

- Avoid `dense_rows()` materialization on every OSD call.
- Precompile the parity-check structure into an elimination-friendly internal
  form that can be reused with different RHS values.

### Solve path

- Rewrite the GF(2) solve path so the expensive static pieces are retained
  across calls.
- Keep correctness criteria identical:
  either produce a valid residual correction or report no OSD solution.

### Integration with BP

- OSD should only run when BP leaves a nonzero residual.
- The handoff should pass already-available hard decision, reliability, and
  residual state rather than recomputing them from scratch.

## Public API And Compatibility

This refactor should preserve the current outward-facing contract unless a
small compatibility adjustment is necessary to make workspace management sound.

Baseline compatibility goals:

- `BpOsdDecoder::new(...)` remains available
- `BpOsdDecoder::decode(&Syndrome)` remains available
- `DecodeResult` fields keep their current meaning
- `CssDecoders` can continue delegating to `BpOsdDecoder`
- `rsinter` can continue compiling DEMs into `BpOsdDecoder` without a new
  benchmark-only code path

Internal helper methods, internal structs, and private modules may change
freely.

## Error Handling

The performance refactor must not weaken existing validation behavior.

Requirements:

- syndrome width mismatch remains an error
- invalid channel probabilities remain an error
- impossible OSD solve remains `DecodeError::NoOsdSolution`
- internal reusable buffers must not leak stale state across decode calls

If workspace reuse requires new internal reset logic, the reset path must be
covered by tests because stale state is a correctness bug, not just a
performance concern.

## Testing Strategy

Testing must scale with the depth of this refactor. The required categories
are:

### 1. Core equivalence tests

Add focused tests for BP and OSD subcomponents on small matrices that confirm:

- valid corrections still satisfy the target syndrome
- stable equal-reliability tie-breaking remains unchanged
- OSD fallback still repairs residuals correctly

### 2. Repeated-decode stability tests

Add tests that reuse one decoder for many distinct syndromes and confirm:

- outputs remain correct across decode order changes
- scratch reuse does not retain stale bits or messages
- OSD and non-OSD calls can interleave safely

### 3. Existing contract preservation

Retain or extend checks around:

- `rbposd/tests/reference.rs`
- parity fixture behavior
- `rsinter/tests/decode_rbposd.rs`
- any existing API-smoke coverage for `CssDecoders` and examples

### 4. Benchmark-facing validation

Use the existing surface decoder benchmark as the final performance validation.

Required checks:

- `rbposd` benchmark rows still produce valid outputs
- logical error rates stay within the existing correctness baseline for the
  shared shot streams
- `decode_us_per_shot` improves materially versus the current checked-in
  results

This design does not require a strict fixed timing threshold in unit tests.
Performance acceptance belongs in the benchmark workflow, not in flaky test
timers.

## Success Criteria

This project is successful only if all of the following are true:

- the public `BpOsdDecoder` usage pattern remains intact for existing callers
- the refactored decoder passes repository correctness gates
- repeated decode calls reuse internal structure instead of rebuilding the main
  BP and OSD state each time
- fresh `surface_decoder_compare` runs, when regenerated for performance work,
  should be reported separately from the tracked CSV artifact cited above
- any remaining `ldpc` comparison claim is grounded in the specific benchmark
  artifact being discussed, not copied forward from stale checked-in numbers

## Implementation Boundaries

The most likely implementation touch points are:

- `rbposd/src/decoder.rs`
- `rbposd/src/bp.rs`
- `rbposd/src/osd.rs`
- `rbposd/src/gf2.rs`
- `rbposd/src/matrix.rs`
- targeted `rbposd` and `rsinter` tests

This work should stay out of:

- benchmark timing-rule changes
- `ldpc` driver changes
- benchmark-only fast paths in `rsinter`
- unrelated refactors across other workspace crates

## Follow-On Work

This design intentionally leaves room for later extensions after the core
performance rewrite is stable:

- internal batch decode over reusable workspaces
- a public batch API if there is a demonstrated need
- further algorithmic tuning after the structural costs are removed

Those are explicitly follow-on items, not requirements for this delivery.
