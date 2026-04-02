# DEM Provenance Tracking Design

Date: 2026-04-02
Status: Proposed
Scope: `rstim`

## Summary

This design adds an opt-in provenance-tracking analysis path to `rstim` so users can query the precise relationship between circuit-side error sources and final DEM error lines in both directions:

- given a circuit noise source, find the final DEM error lines it contributes to
- given a DEM error line, find the exact circuit-side noise sources that produced it

The tracked path must preserve provenance through:

- nested `REPEAT` structure and concrete repeat iterations
- per-instruction target slots
- per-source branch identity such as `X`, `Y`, `Z`, `XY`, `ZZ`, measurement flips, and correlated-error branches
- DEM target canonicalization
- graphlike decomposition and post-decomposition merging

The default DEM generation path remains unchanged and does not pay the runtime or memory cost of provenance tracking.

## Goals

- Preserve exact many-to-many provenance between circuit-side error sources and final DEM error lines.
- Support source precision at the level of repeat instance, instruction position, target slot, and branch label.
- Export query results as a QP101 document with additional highlight metadata suitable for downstream visualization.
- Keep the existing `circuit_to_dem` and `export_json` behavior unchanged unless the tracked path is explicitly requested.

## Non-Goals

- Do not add provenance storage to the default `DetectorErrorModel`.
- Do not require parser text spans or source line numbers in the first version.
- Do not solve per-component provenance inside a single final DEM line beyond the line-level mapping.
- Do not redesign the generic QP101 core schema for all tools; the query payload is an `rstim` extension.

## Current State

The current implementation in [`rstim/src/error_analyzer.rs`](/Users/nzy/rcode/rstim/rstim/src/error_analyzer.rs) loses source information in two places:

1. Raw circuit-side errors are collected as bare `(probability, targets)` tuples and then merged by canonicalized target set before the final `DetectorErrorModel` is built.
2. `decompose_errors` rewrites non-graphlike error targets and then merges again, without preserving any circuit-side identity.

As a result, `rstim` can currently support DEM-side explanation based on final DEM lines, but it cannot answer precise source-to-DEM questions after merges and decompositions.

## Requirements

### Functional Requirements

- The tracked path must preserve exact source identity from emission to final DEM output.
- Each tracked source must distinguish:
  - static operation path inside nested `REPEAT` bodies
  - dynamic repeat iteration path
  - target slot
  - branch label
- Queries must support:
  - `dem_error -> [source]`
  - `source -> [dem_error]`
- Query export must produce a QP101 JSON document whose base circuit content matches the normal export, with added highlight metadata for the queried sources.

### Performance Requirements

- Provenance tracking must be opt-in.
- The default DEM generation path must remain lightweight and should not allocate provenance structures.
- The tracked path may consume additional memory proportional to the number of emitted raw error sources and final merged provenance links.

## Recommended Architecture

Add a separate tracked-analysis pipeline instead of extending the default pipeline in place.

### Public API Shape

Keep the current API:

- `ErrorAnalyzer::circuit_to_dem(...) -> Result<DetectorErrorModel, String>`

Add tracked variants:

- `ErrorAnalyzer::circuit_to_tracked_dem(...) -> Result<TrackedDemResult, String>`
- `ErrorAnalyzer::circuit_to_tracked_dem_with_options(...) -> Result<TrackedDemResult, String>`
- query-oriented helper APIs as needed for exporting highlight JSON

The tracked result owns:

- the final `DetectorErrorModel`
- the tracked source table
- the forward and reverse provenance indices

### Core Types

#### `TrackedSource`

Represents one precise circuit-side error source. This is finer-grained than a `StimInstr`.

Required fields:

- `source_id`
- `op_path: Vec<usize>`
- `repeat_iterations: Vec<u64>`
- `instr_name: String`
- `target_slots: Vec<usize>`
- `target_qubits: Vec<u32>`
- `branch: SourceBranch`
- `probability_fragment: f64`

Optional fields may be added later for debugging, such as raw DEM targets or parser spans.

#### `SourceBranch`

Internal enum describing the branch identity of a source.

The first version should cover at least:

- `X`
- `Y`
- `Z`
- two-qubit Pauli branches such as `XX`, `XY`, `XZ`, `YX`, `YY`, `YZ`, `ZX`, `ZY`, `ZZ`
- `MeasurementFlip`
- `CorrelatedBranch { index }`
- `Custom { label }`

#### `TrackedErrorTerm`

Tracked version of the current internal `(probability, targets)` item.

Required fields:

- `probability: f64`
- `targets: Vec<DemTarget>`
- `source_ids: Vec<SourceId>`

Semantics:

- This is an intermediate analyzer artifact.
- It represents one current DEM-side error term together with the exact set of circuit-side sources that flow into it.

#### `TrackedDemResult`

Required fields:

- `dem: DetectorErrorModel`
- `sources: Vec<TrackedSource>`
- `dem_error_to_sources: Vec<Vec<SourceId>>`
- `source_to_dem_errors: Vec<Vec<DemErrorId>>`

The `dem_error_to_sources` table indexes only final `error(...)` instructions, not detector annotations or other DEM instructions.

## Traversal Strategy

Tracked mode must not depend on the current flatten-first path for `REPEAT`.

### Why flattening is insufficient

The tracked feature must return both:

- the static operation location inside nested `REPEAT` bodies
- the dynamic repeat instance path for the concrete source occurrence

Flattening preserves neither cleanly. It replaces hierarchical structure with a linear stream, which makes it impossible to recover a stable `repeat[outer] -> repeat[inner] -> operation` identity for downstream visualization.

### Tracked traversal model

Use a recursive traversal that maintains:

- `op_path`: array indices through nested `operations` and `repeat.body`
- `repeat_iterations`: the concrete iteration chosen at each nested `REPEAT`

Every source emitted during analysis captures both vectors.

## Source Emission Rules

All source creation must flow through shared tracked helpers. The tracked path must not directly append bare `(probability, targets)` tuples.

### Emission invariant

At the moment a raw circuit-side source is emitted:

1. create one `TrackedSource`
2. assign a stable `source_id`
3. create one or more `TrackedErrorTerm` values whose `source_ids` initially contain only that `source_id`

This is the only stage where new provenance identities are created. Later stages only transform or merge existing provenance.

### Target-slot precision

For operations with multiple targets, tracked emission must record the exact target slots that caused the source.

Examples:

- `DEPOLARIZE1(p) 3 5` can emit separate sources for slot `[0]` and slot `[1]`
- `PAULI_CHANNEL_2(...) 4 7` records the slot pair `[0, 1]` associated with the two-qubit branch
- measurement noise records the measurement site that produced the source

### Branch precision

Operations that expand into multiple branches must produce source records with explicit `SourceBranch` values.

Examples:

- `DEPOLARIZE1` emits `X`, `Y`, `Z`
- `DEPOLARIZE2` emits two-qubit Pauli branches
- correlated error blocks can emit `CorrelatedBranch { index }`

## Merging And Canonicalization

The tracked path keeps the current probability semantics for merged DEM targets, but it must also preserve source identity.

### Raw merge

Current merge key:

- canonicalized `Vec<DemTarget>`

Tracked merge value:

- merged probability
- union of `source_ids`

If multiple independent circuit-side sources end up with the same target set, the final merged DEM line records all of them in its provenance set.

### Canonicalization rule

Target canonicalization continues to normalize ordering and separator layout exactly as it does today. Provenance is not part of the key; it is carried in the merge value.

## Decomposition Strategy

`decompose_errors` must gain a tracked equivalent or an internal tracked mode.

### Required semantics

When a non-graphlike term is rewritten into graphlike components:

- the rewritten term inherits the original term's `source_ids`
- no new `source_id` values are created during decomposition
- later merge steps union provenance sets when rewritten targets collapse to the same final DEM line

This preserves exact line-level many-to-many provenance.

### Precision boundary

The first version guarantees exact mapping at the final DEM error-line level:

- exact `source -> final DEM lines`
- exact `final DEM line -> sources`

It does not guarantee separate provenance for each `^`-separated component inside a single final DEM line. This is acceptable for the intended visualization workflow because the rendering only needs to highlight circuit-side source locations for a queried DEM line.

## QP101 Highlight Export

The base QP101 document remains structurally identical to the normal export. Query results are attached through a top-level extension block.

### Recommended extension shape

Add highlight metadata under:

- `extensions.rstim_query_highlights`

Recommended structure:

```json
{
  "extensions": {
    "rstim_query_highlights": {
      "version": "1",
      "query": {
        "kind": "dem_error_origin",
        "dem_error_index": 17
      },
      "highlights": [
        {
          "op_path": [12, 3, 5],
          "repeat_iterations": [4, 2],
          "target_slots": [1],
          "target_qubits": [5],
          "branch": "Y",
          "label": "Y"
        }
      ]
    }
  }
}
```

### Field semantics

- `op_path`
  - static path to the operation within nested `operations` and `repeat.body` arrays
- `repeat_iterations`
  - dynamic repeat instance path for the matched occurrence
- `target_slots`
  - precise slot indices inside the matched operation's target list
- `target_qubits`
  - direct qubit indices needed by downstream rendering
- `branch`
  - source branch label, serialized from `SourceBranch`
- `label`
  - renderer-facing display label; initially identical to `branch`

### Why highlights live in `extensions`

This metadata is query-specific, not part of the core circuit definition. It also includes dynamic repeat-instance identity, which does not belong inside the static QP101 operation tree. The QP101 document already reserves `extensions` for non-core auxiliary data, so this is the least invasive and most composable location.

## Query Model

The first version should support at least:

- query by final DEM error index to produce a highlighted QP101 document
- internal reverse lookup from source id to DEM error ids

For a `dem_error_index` query:

1. build tracked analysis result
2. resolve `dem_error_index -> [source_id]`
3. convert each source into one highlight record
4. export the base QP101 document
5. inject `extensions.rstim_query_highlights`

If multiple source ids map to the same:

- `op_path`
- `repeat_iterations`
- `target_slots`
- `branch`

they may be coalesced into one exported highlight record.

## Error Handling

- Invalid DEM error indices must return a clear error.
- If tracked decomposition fails, the tracked API returns the same class of decomposition error as the normal path, with enough context to identify the failing term.
- If a query requests highlight export but the tracked analysis result is unavailable, the export path must fail instead of silently emitting incomplete metadata.

## Performance Trade-Offs

Tracked mode adds overhead from:

- source allocation for every emitted raw error source
- provenance-set unions during merge
- provenance propagation during decomposition
- reverse-index construction for final query support

This is acceptable only because the feature is explicitly opt-in. The default path must remain the recommended path for users who only need the final DEM.

## Testing Strategy

### Unit Tests

- source emission preserves `target_slots` for multi-target noise instructions
- source emission preserves branch identity for `DEPOLARIZE1`, `DEPOLARIZE2`, and correlated blocks
- tracked traversal preserves `repeat_iterations` for nested repeats

### Provenance Correctness Tests

- multiple sources merged into one final DEM line produce an exact union of source ids
- one source that decomposes into multiple final DEM lines produces exact reverse links
- canonicalization does not change the resolved source set

### Export Tests

- highlighted QP101 JSON preserves the original circuit structure
- extension payload includes `op_path`, `repeat_iterations`, `target_slots`, and `branch`
- a representative repeat-containing circuit exports the correct repeat-instance highlight records

## Rollout Plan

Implement in this order:

1. add tracked core data structures and tracked source emission helpers
2. add tracked recursive traversal that preserves repeat identity
3. add tracked merge support
4. add tracked decomposition support
5. add query API and QP101 highlight export
6. add unit and integration coverage for merge, decomposition, and repeat-aware export

## Open Decisions Resolved By This Design

- Provenance tracking is opt-in, not default.
- Source precision includes repeat iterations, target slots, and branch labels.
- The first exported JSON form is a highlighted QP101 document, not a separate draw-only format.
- Query-specific metadata lives in `extensions`, not inside core QP101 operations.
