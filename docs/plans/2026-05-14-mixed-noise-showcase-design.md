# Mixed-Noise QP101 Showcase Design

**Date:** 2026-05-14

## Goal

Turn the current
`qp101-viz/examples/surface-code-rotated-memory-x-d3-r3-atom-loss.typ`
workflow into a fixed, reproducible showcase example that demonstrates one
rotated surface-code memory-X circuit containing both sparse Pauli noise and
sparse atom loss.

The showcase must answer two separate needs:

1. provide a stable human-facing circuit render in `qp101-viz/examples/`
2. provide a stable generation path owned by `rstim`, so the committed JSON is
   produced from code instead of being treated as hand-maintained fixture data

The example is meant for explanation, renderer verification, and debugging. It
is not meant to be a statistically representative noise model or a generic
noise-parameter benchmark.

## User-Confirmed Display Contract

The final showcase should follow these constraints:

- keep the base code family as rotated surface-code memory-X
- use `distance=3`, `rounds=3`
- include both atom loss and common Pauli-noise-style instructions in the same
  source circuit
- keep all error probabilities small; start with `0.01`
- keep the number of noise sites low enough that the source timeline remains
  readable
- keep the sample-shot overlay visually informative by choosing a seed that
  fires a small number of events instead of increasing the source error rates

The intent is a "mixed noise, but not crowded" circuit.

## Current State

The current
`qp101-viz/examples/surface-code-rotated-memory-x-d3-r3-atom-loss.typ`
example reads two committed JSON files:

- a base exported circuit JSON
- a seeded sample-shot exported JSON

However, the generation process is not packaged as a first-class showcase. The
source `.stim` file currently injects `LOSS(0.1)` onto every data qubit at the
start of each round, which is much denser than the desired mixed-noise example.

The repository already has the pieces needed to make this a proper showcase:

- `rstim::codegen::surface_code::rotated_memory_x(3, 3, 0.0)` can generate the
  clean scaffold circuit
- `rstim` already exports QP101 JSON through `export_qp101`
- `rstim` already exports seeded sample overlays through
  `export_qp101_with_sample_trace`
- `qp101-viz` already renders `LOSS`, `X_ERROR`, `Z_ERROR`, `DEPOLARIZE1`, and
  `DEPOLARIZE2`

The missing piece is a single repository-owned workflow that fixes what the
showcase circuit is and how the committed example artifacts are regenerated.

## Key Design Decision

Do not generate this showcase by turning on uniform codegen noise everywhere.

`NoiseParams::uniform(0.01)` is useful for generic noisy circuits, but it is
the wrong tool for this showcase. It would inject noise at many positions
throughout the rotated surface-code circuit, which weakens the main purpose of
the example:

- the source circuit becomes harder to read
- the timeline render becomes crowded with low-value repetition
- the example stops being curated and starts behaving like a generic noisy
  benchmark circuit

Instead, the showcase should be built from the noiseless rotated-memory-X
generator output and then patched with a small, fixed set of sparse noise sites.
This keeps the circuit semantically realistic enough for visualization while
remaining presentation-friendly.

## Recommended Architecture

The showcase should be split into two ownership layers.

### Layer 1: `rstim` owns generation

`rstim` should define the showcase circuit as code. This layer is responsible
for:

- generating the clean rotated surface-code scaffold
- inserting a fixed sparse set of noise instructions
- producing the committed `.stim` source
- producing the committed base QP101 JSON
- producing the committed seeded sample-shot QP101 JSON

This layer defines the truth of the example.

### Layer 2: `qp101-viz` owns rendering

`qp101-viz/examples/` should keep only human-facing render inputs and the Typst
showcase file. It should not contain hidden logic about how the circuit was
built. The Typst example simply reads the committed JSON files and renders:

- the source circuit
- one seeded sample shot

This keeps the visualization package declarative and lets `rstim` remain the
single source of generation semantics.

## Showcase Circuit Shape

The source circuit should start from:

- `surface_code::rotated_memory_x(3, 3, 0.0)`

Then it should receive a small set of manually chosen noise insertions. The
goal is representation, not coverage. The recommended instruction families are:

- `LOSS(0.01)`
- `X_ERROR(0.01)`
- `Z_ERROR(0.01)`
- `DEPOLARIZE1(0.01)`
- `DEPOLARIZE2(0.01)`

The insertion pattern should stay sparse:

- atom loss only on one or two data qubits per round, not all data qubits
- one or very few `DEPOLARIZE1` sites at round boundaries or idle-like moments
- one or very few `DEPOLARIZE2` sites after representative two-qubit layers
- one or very few `X_ERROR` sites before measurement-sensitive moments
- one or very few `Z_ERROR` sites in an X-basis-sensitive region

This is intentionally not a symmetric or exhaustive noise layout. It is a
curated showcase whose job is to make the visualization readable while still
showing the main supported noise families together in one circuit.

## Sample-Shot Strategy

The source error rates should stay at `0.01`, but the sample-shot visualization
must still be visually informative.

The correct way to satisfy both goals is to separate source semantics from
sample visibility:

- do not raise the source error rates just to make the sample look busy
- instead, search a small set of deterministic seeds and choose one that causes
  a few visible events

The accepted seed should produce a sample with roughly:

- a small number of fired noise annotations
- at least one visible consequence on a measurement or detector when possible
- no overwhelming burst of unrelated noise markers

The seed becomes part of the committed showcase contract and should be recorded
in the documentation and regeneration command.

## File and Naming Plan

The showcase should remain easy to discover from the existing example tree.

Recommended artifact set:

- one committed `.stim` file in `qp101-viz/examples/`
- one committed base `.qp101.json` file in `qp101-viz/examples/`
- one committed sample `.qp101.json` file in `qp101-viz/examples/`
- one `.typ` file in `qp101-viz/examples/` that renders both JSON files

The existing file names may either be updated in place or renamed to reflect
"mixed noise" instead of pure atom loss. Renaming is preferred if it keeps the
example meaning honest, but preserving the existing file location is acceptable
if avoiding churn is more important.

On the `rstim` side, the showcase generation helper should live near the
existing showcase support, such as `rstim/src/showcase.rs` or a closely related
module. It does not need a generic public API beyond what is required to
regenerate the committed example artifacts and test them.

## Regeneration Flow

The repository should expose one clear regeneration path for this example.

Recommended flow:

1. generate the mixed-noise rotated-memory-X circuit text from `rstim`
2. write the `.stim` artifact
3. export the base QP101 JSON from the same circuit
4. run one seeded sample shot on the same circuit
5. export the sample-overlay QP101 JSON

The design does not require a new general-purpose CLI command. A focused helper
used by tests or a small dedicated example binary is enough, as long as the
regeneration steps are deterministic and documented in README text.

The committed artifacts should never be edited manually unless the showcase
definition changes intentionally.

## Testing Strategy

Verification should happen in three layers.

### 1. Circuit-content test

Add a test that asserts the generated showcase circuit contains:

- `LOSS(0.01)`
- `X_ERROR(0.01)`
- `Z_ERROR(0.01)`
- `DEPOLARIZE1(0.01)`
- `DEPOLARIZE2(0.01)`

This test should also guard against obvious density regressions, such as loss
being inserted on every data qubit every round again.

### 2. Export fixture test

Add a fixture-style test that regenerates the base QP101 document and compares
it against the committed JSON artifact. This guards the "source circuit render"
contract.

### 3. Sample fixture test

Add a second fixture-style test that regenerates the seeded sample-shot QP101
document and compares it against the committed sample JSON artifact. This
guards the "sample overlay render" contract.

Together, these tests make the showcase useful as a renderer regression case,
not just as README decoration.

## Documentation Plan

At minimum, both of these locations should mention the new showcase workflow:

- root `README.md`
- `qp101-viz/README.md`

The docs should explain:

- what the showcase demonstrates
- that it contains both sparse Pauli noise and sparse atom loss
- how to regenerate the `.stim`, base JSON, and sample JSON artifacts
- which seed was chosen for the sample-shot example

The documentation should describe the workflow as code-generated and committed,
not hand-authored JSON.

## Failure Modes and Guardrails

The main risks are:

1. sample seed produces no meaningful visible events
2. noise insertion becomes too dense over time
3. artifact regeneration drifts away from the committed files without a clear
   workflow
4. the example name suggests pure atom loss when the contents are now mixed
   noise

The design addresses these with:

- explicit seed selection as part of the contract
- sparse, fixed insertion sites instead of uniform noise parameters
- fixture comparisons for both base and sample exports
- documentation and optional renaming to keep semantics honest

## Out of Scope

This showcase does not attempt to:

- model a production-quality mixed-noise process
- introduce a new general mixed-noise codegen API for all circuits
- compare `rstim` against `stim` on this custom loss-enabled circuit
- render multi-shot summaries
- add new visualization primitives beyond the already supported noise families

Those are separate tasks and should not be bundled into this showcase cleanup.

## Next Step

If implementation continues, the first step should be to create the
`rstim`-side helper that builds the sparse mixed-noise circuit from the clean
rotated-memory-X scaffold. Once that circuit shape is stable, regenerate the
committed `qp101-viz/examples/` artifacts, add fixture tests, and then update
the README text around the example.
