# rstim Stats CLI Design

**Date:** 2026-03-22

## Goal

Add a first-class inspection command to `rstim` so users can quickly understand
the structural and execution-relevant size of a circuit before sampling,
analyzing errors, or exporting data.

The new command is:

```sh
rstim stats
```

It should accept circuit text from `--in <path>` or from `stdin`, matching the
existing CLI conventions. The command has two output modes:

- default human-readable text for terminal use
- `--json` for scripting, tests, and downstream tooling

This is intentionally a read-only inspection command. It does not execute the
circuit, sample random data, or derive a detector error model. The objective is
to expose stable circuit summary information that is already mostly available in
library form, but currently not packaged into a simple CLI entry point.

## Scope

The first version should cover a compact "core summary" instead of trying to be
a full circuit linter or gate-by-gate profiler.

The summary fields are:

- `instruction_count`
- `repeat_blocks`
- `max_repeat_depth`
- `num_qubits`
- `num_measurements`
- `num_detectors`
- `num_observables`
- `num_ticks`
- `num_sweep_bits`

This field set intentionally mixes two perspectives:

- structure size: instruction and repeat metrics
- execution-facing counts: qubits, measurements, detectors, observables, ticks,
  and sweep bits

That split is useful because users often need both answers. A circuit may have a
small structural description due to `REPEAT`, while still implying a large
expanded execution footprint.

## Command Shape

`rstim stats` should be added as a top-level subcommand beside the existing
sampling, conversion, analysis, and generation commands.

Recommended CLI shape:

```sh
rstim stats [--in PATH] [--out PATH] [--json]
```

Behavior rules:

- `--in` is optional; omit it to read circuit text from `stdin`
- `--out` is optional; omit it to write to `stdout`
- default output is text
- `--json` switches the output format to JSON

This design deliberately avoids introducing multiple formatting modes such as
`--format text|json|yaml` in the first pass. The current requirement is only to
support a stable human-readable summary and a stable machine-readable summary.

## Library Architecture

The implementation should not bake the summary logic directly into the CLI.
Instead, `rstim::stats` should gain a library-facing summary type and a single
entry point that computes it from parsed instructions.

Recommended additions in `rstim/src/stats.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CircuitStatsSummary {
    pub instruction_count: usize,
    pub repeat_blocks: usize,
    pub max_repeat_depth: usize,
    pub num_qubits: usize,
    pub num_measurements: usize,
    pub num_detectors: usize,
    pub num_observables: usize,
    pub num_ticks: usize,
    pub num_sweep_bits: usize,
}

pub fn summarize(instrs: &[StimInstr]) -> CircuitStatsSummary
```

This keeps one canonical source of truth for:

- CLI text output
- CLI JSON output
- future library callers
- unit tests that should avoid invoking the full binary

The existing `num_*` helper functions remain useful and should be reused instead
of duplicated. The new work in `summarize` is primarily aggregation plus the
three structural metrics.

## Metric Semantics

The field meanings must be documented and tested clearly.

`instruction_count` is the total number of instructions present in the parsed
tree, counting instructions inside nested `REPEAT` bodies exactly once per
appearance in the syntax tree. It does not multiply by repeat count.

`repeat_blocks` is the number of `REPEAT` instructions present in the parsed
tree.

`max_repeat_depth` is the maximum nesting depth of `REPEAT` blocks. A circuit
with no repeats has depth `0`. A top-level `REPEAT` has depth `1`. Nested
repeats increment depth accordingly.

The remaining counts keep the semantics already established by `rstim::stats`.
In particular, quantities such as `num_measurements`, `num_detectors`, and
`num_ticks` are execution-relevant logical totals, so they do multiply through
repeat counts.

This difference is intentional and should be explained in both tests and docs.
It allows the command to answer both "how big is the source circuit?" and "how
large is the expanded logical workload?" without adding a second command.

## Output Contract

The text output should be simple and stable, using one `key: value` pair per
line in a fixed order:

```text
instruction_count: 5
repeat_blocks: 1
max_repeat_depth: 1
num_qubits: 3
num_measurements: 20
num_detectors: 10
num_observables: 1
num_ticks: 10
num_sweep_bits: 0
```

This format is intentionally plain. It is easy to read in a terminal and easy
to assert in integration tests.

The JSON output should serialize the same fields using snake_case keys:

```json
{
  "instruction_count": 5,
  "repeat_blocks": 1,
  "max_repeat_depth": 1,
  "num_qubits": 3,
  "num_measurements": 20,
  "num_detectors": 10,
  "num_observables": 1,
  "num_ticks": 10,
  "num_sweep_bits": 0
}
```

The JSON schema should remain intentionally flat. Nested groups such as
`{"structure": ..., "execution": ...}` would look tidy, but they make the first
version harder to use from shell tools and noisier to document.

## Error Handling

`rstim stats` should reuse the existing CLI error behavior.

- unreadable `--in` path returns an I/O error
- invalid circuit text returns the parser error
- invalid output path returns a write error

No new validation rules should be introduced in this command. An empty circuit
is valid and should produce a summary where every field is `0`.

The command should not infer or compute anything beyond parsed circuit metadata.
For example, it should not attempt to validate detector determinism or construct
a detector error model. Those remain the responsibility of `analyze_errors`.

## Testing Strategy

Verification should happen at both library and CLI layers.

Library tests should cover:

- empty circuit summary
- simple flat circuit summary
- repeat-containing circuit where structural metrics differ from expanded counts
- nested repeat circuit proving `max_repeat_depth`
- sweep-bit usage proving `num_sweep_bits`

CLI integration tests should cover:

- text output from `stdin`
- JSON output from `stdin`
- file input via `--in`
- parse failure behavior

The most important targeted test is a repeat-based case where:

- `instruction_count` stays small
- `num_measurements` reflects expansion through repeat count

That is the most subtle semantic choice in the design and the easiest one for
users to misunderstand if it is not locked down by tests.

## Documentation Plan

Documentation should be split by audience and purpose instead of repeating the
same content everywhere.

`README.md` should gain a short section showing that `rstim` can inspect
circuits as well as simulate them. One minimal `rstim stats` example is enough.

`rstim/doc/getting_started.md` should add a short workflow section showing how a
user can parse their first circuit mentally by running:

1. `rstim stats`
2. `rstim sample` or `rstim analyze_errors`

The goal there is onboarding, not exhaustive reference.

A new `rstim/doc/cli.md` file should act as the CLI reference. It should group
commands by purpose:

- inspection: `stats`
- sampling: `sample`, `detect`, `sample_dem`
- transforms: `convert`, `m2d`
- analysis: `analyze_errors`, `explain_errors`
- generation/export: `gen`, `export_json`

`rstim stats` should be documented most fully in this file, including:

- input conventions
- text output example
- JSON output example
- explanation of structural vs expanded counts

This split avoids overloading the README while still giving the CLI a coherent
home.

## Non-Goals

This first `stats` command does not:

- provide per-gate histograms
- classify noise operations separately from unitary gates
- analyze DEM size or graph properties
- expand the circuit into a flattened instruction list for output
- introduce subcommands such as `stats gates` or `inspect dem`

Those are valid follow-up directions, but they should not be bundled into the
first inspection command.

## Recommended Next Step

Once implementation begins, start with the library summary type and unit tests.
Then add the CLI wrapper and integration tests. After the command behavior is
stable, update the three documentation surfaces in this order:

1. CLI reference
2. getting started
3. README

That sequence keeps the most detailed source of truth in place before the
higher-level docs are updated to point at it.
