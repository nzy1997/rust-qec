# Issue 489 Fold Periodic Reference Sampling Repeats Design

Issue: #489
Date: 2026-07-12

## Context

Issue #488 is closed and merged by PR #505, so this work can build on the
checked-in `ReferenceSampleTree`. The current packed reference builder in
`rstim/src/data_path.rs` expands every `REPEAT` iteration into a flat
`Vec<bool>`. The selected surface-code fixture has `REPEAT 99`; after one
packed-reference round, the packed inverse tableau reaches a period-one loop
state, so 98 loop bodies can be skipped without changing the final reference
bits.

There is no repository `AGENTS.md`, `CLAUDE.md`, `GEMINI.md`, or
`CONVENTIONS.md`. The relevant repository context is the merged #488 tree
design, `packed_reference_routing` tests, `rstim_reference_build_worker`, and
`profile_reference_build.py`.

## Automatic Scope Decisions

This Agent Desk run is non-interactive, so the Standing Answer Policy resolves
the Superpowers gates:

- Visual companion: not used because the task is backend Rust sampling logic.
- Clarifying questions: answered from issue #489, merged #488 context, and
  existing packed-reference telemetry tests.
- Recommended approach: detect exact packed-tableau cycles at repeat-loop
  boundaries, store produced period output in `ReferenceSampleTree`, and
  decompress to the existing flat `Vec<bool>` API at the end.
- Design approval: accepted automatically because the issue gives exact
  interface rules, required tests, verification commands, and out-of-scope
  constraints.
- Spec review: this document is approved for planning after placeholder,
  consistency, and scope checks pass.

## Alternatives Considered

1. Compare only produced measurement bits. This is rejected by the issue's
   negative control: `REPEAT 99 { X 0 } M 0` produces no in-loop bits but
   alternates quantum state, so bit-only folding would incorrectly leave the
   final measurement at `0`.
2. Add a hash-only repeat detector for speed. This is rejected because the
   issue requires exact packed inverse-tableau equality after any hash
   acceleration; the first implementation can use direct equality on cloned
   packed tableaus.
3. Detect exact packed inverse-tableau states at loop boundaries and compress
   the matched period's measurement output in `ReferenceSampleTree`. This is
   chosen because it preserves existing packed-path fallback rules, catches
   period-one and period-two loops, supports nested repeats through recursion,
   and still returns the existing flat `Vec<bool>`.

## Chosen Design

`build_packed_reference_sample` will build a `ReferenceSampleTree` instead of
appending directly into the final `Vec<bool>`. The public result remains
unchanged: the tree is decompressed into a flat `Vec<bool>` before
`ReferenceSampleResult` is returned.

Packed instruction execution will use an accumulator tree:

- measurement bits emitted before any repeat child extend the node
  `prefix_bits`;
- repeat-body output is appended as a suffix child;
- measurement bits emitted after a child are appended as leaf children to
  preserve output order;
- empty children are skipped;
- completed nodes are simplified before returning.

Repeats with `count < 10` execute exactly as before. Their body is run once per
logical iteration, and no skipping is attempted.

Repeats with `count >= 10` record exact `PackedInverseTableau` clones at loop
boundaries. Each executed iteration appends the recursively built body output
to the repeat tree, then compares the current tableau to every previously seen
boundary state. On the first exact match:

1. The period starts at the previous matching iteration and ends at the current
   iteration.
2. The period's already-produced tree children are wrapped in a
   `ReferenceSampleTree` whose `repetitions` is the observed period plus the
   number of whole remaining cycles.
3. Whole remaining cycles are skipped without executing the body.
4. Any leftover remainder iterations execute normally.

This handles period-one, period-two, and transient-prefix cycles without
hashing. Because state equality is on `PackedInverseTableau`, body output alone
cannot cause a fold.

## Telemetry

`ReferenceBuildPhaseCounters` will keep existing fields and add:

- `executed_repeat_iterations`: loop-body executions actually run by the
  packed-reference builder;
- `skipped_repeat_iterations`: loop-body executions skipped after exact cycle
  detection.

`expanded_repeat_iterations` remains the logical repeat-iteration count for
compatibility with existing JSON consumers. The profile script will validate
the new counters and print the issue's requested summary labels:
`executed_repeats` and `skipped_repeats`.

## Tests

Add `rstim/tests/repeat_aware_reference_sample.rs` covering:

- period-one folding for a long measurement-only repeat;
- period-two folding for a long state-alternating repeat with measurement
  output;
- short repeats below 10 executing without skip;
- nested repeats folding recursively;
- the issue's negative control, `REPEAT 99 { X 0 } M 0`, proving state
  comparison is used even when the loop body emits no measurement bits;
- the surface-code fixture's 12,121 zero reference bits, packed-byte digest
  `d95f3eacd05c1ca0d3a90e4a48e1d68b7ef5f2d817da11121ba4b77454b24d3d`, and
  reduced executed/skipped repeat telemetry.

Update existing packed-reference and worker/profile tests so their expected
phase counters include the new execution and skip fields.

Focused verification:

```sh
cargo test -p rstim --test repeat_aware_reference_sample -- --nocapture
cargo test -p rstim --test packed_reference_routing -- --nocapture
cargo test -p rstim --test rstim_reference_build_worker
python3 -m unittest benchmarks.rstim_vs_stim_simulator.tests.test_profile_reference_build -q
```

Final issue verification:

```sh
cargo test -p rstim --test repeat_aware_reference_sample -- --nocapture
cargo build --release -p rstim --bin rstim_reference_build_worker
python3 -m benchmarks.rstim_vs_stim_simulator.profile_reference_build \
  --fixture benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim \
  --worker target/release/rstim_reference_build_worker \
  --out /tmp/rstim-repeat-aware-profile.json
cargo test
```

## Out Of Scope

This design does not fold legacy-fallback circuits, add measurement-record
feedback support to the packed path, change the public flat `Vec<bool>` API, or
introduce hash-only state comparison.

## Self-Review

The spec has no placeholders or contradictory requirements. It keeps the
existing fallback boundary intact, uses the merged #488 tree only as an
internal compression structure, and maps every issue verification requirement
to a concrete test or command.
