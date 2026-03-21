# rstim / QSTD101 Visualization Design

**Date:** 2026-03-21

## Goal

Add a visualization pipeline for `rstim` circuits that uses Rust to export a JSON document and Typst to render diagrams. The JSON format should stay broadly framework-agnostic, but it must preserve the Stim-style semantics that matter for quantum error-correction circuits, including `REPEAT`, `TICK`, coordinate annotations, detector annotations, observable annotations, noise operations, and feedback-style raw targets.

The output of this design is not the final renderer implementation. It is the agreement on the protocol shape, system boundaries, and phased delivery needed to implement the exporter and the Typst package without reworking the schema later.

## Context And Constraints

Three codebases are involved:

1. `/Users/nzy/rcode/rstim`
2. `/Users/nzy/mcode/QProtocal/QSTD101-ZY.md`
3. `/Users/nzy/tycode`

`rstim` already represents circuits as Stim-like instruction trees with explicit `StimInstr::Op` and `StimInstr::Repeat`, and it already supports `QUBIT_COORDS`, `SHIFT_COORDS`, `TICK`, `DETECTOR`, and `OBSERVABLE_INCLUDE`. That means the exporter must preserve hierarchical structure instead of flattening it away.

`QSTD101-ZY.md` is still a draft. The design should therefore keep `version: 1.0` while freely replacing the current gate-list-only structure with a stronger draft that still keeps the same large idea: a generic JSON circuit format with extension fields for framework-specific semantics.

The first Typst deliverable is a timeline circuit renderer. A coordinate-layout renderer is explicitly in scope for the next phase, so the JSON protocol and the Typst package API must preserve layout information from day one even if the first released renderer does not draw it yet.

## Key Decisions

### 1. Keep The Protocol Generic, But Do Not Force A Pure Gate List

The current `QSTD101-ZY.md` draft uses:

```json
{
  "num_qubits": 3,
  "gates": [...]
}
```

That structure is too narrow for `rstim`. Trying to encode `REPEAT`, `TICK`, `DETECTOR`, `SHIFT_COORDS`, or `rec[-k]` feedback by squeezing them into gate entries would make the document harder to read and would silently lose semantics.

The protocol should therefore stay generic in spirit, but the ordered instruction stream should be modeled as `operations`, not `gates`.

### 2. Preserve `REPEAT` As A Nested Block

`REPEAT` is not a cache or a rendering hint. It is part of the circuit semantics and often carries the natural round structure of QEC circuits. The JSON representation should use:

```json
{
  "type": "repeat",
  "count": 100,
  "body": [...]
}
```

The exporter must preserve the tree structure. The Typst renderer may choose to expand a repeat block for drawing, but the protocol itself should remain lossless.

### 3. Preserve `TICK` As An Explicit Operation

The protocol should not derive timing solely from a `moment` or `layer` field. Instead, it should include explicit:

```json
{ "type": "tick" }
```

This keeps the serialized data close to `rstim` IR, supports exact round-trip reasoning, and makes the first timeline renderer straightforward.

### 4. First Renderer: Timeline View

The first Typst package renderer should draw a timeline view with qubits on the vertical axis and operation flow on the horizontal axis. This renderer should draw the full circuit, including:

- gates
- resets
- measurements
- ticks
- detectors
- observables
- noise operations
- coordinate annotations
- shift annotations
- repeat grouping

However, these elements should not all compete for the same visual layer. The renderer should use a `main track + annotation tracks` layout:

- the main track carries wires, gates, resets, measurements, and multi-qubit connectors
- an upper annotation track carries coordinate and shift metadata
- a lower annotation track carries detector and observable metadata
- noise operations may be rendered as compact overlays or side labels instead of full gate boxes

### 5. Layout Rendering Is Deferred, Not Ignored

The first Typst package version can focus on the timeline renderer, but the JSON schema must preserve `QUBIT_COORDS`, `SHIFT_COORDS`, detector coordinates, and any layout-specific extension data so that a later coordinate-layout renderer can be added without changing the exported JSON format.

## Architecture

The system should be split into three independent layers.

### Layer 1: Protocol Draft

`/Users/nzy/mcode/QProtocal/QSTD101-ZY.md` becomes the protocol source of truth. It should define:

- top-level document structure
- core operation model
- extension operation types
- validation rules
- examples for both simple generic circuits and Stim-style QEC circuits

This layer is descriptive only. It does not contain Rust or Typst implementation details beyond what is needed to explain interoperability.

### Layer 2: `rstim` Exporter

`rstim` should expose a library API that converts `StimInstr` trees into a serializable QSTD101 document. The exporter is responsible for preserving semantics, not for deciding visual layout. File writing and CLI integration should be thin wrappers around the structured exporter.

### Layer 3: Typst Package

A new Typst package under `/Users/nzy/tycode` should accept the JSON document and render diagrams. The package should be reusable outside `rstim`; it should consume QSTD101 JSON directly instead of any `rstim`-specific intermediate format.

## Protocol Shape

The recommended top-level JSON structure is:

```json
{
  "standard": "QSTD101-ZY",
  "version": "1.0",
  "num_qubits": 17,
  "operations": [...],
  "metadata": {...},
  "extensions": {...}
}
```

### Top-Level Fields

- `standard`: protocol identifier
- `version`: kept at `1.0` because the document is still a draft
- `num_qubits`: total qubit count
- `operations`: ordered execution stream
- `metadata`: framework-neutral metadata such as source tool, generator info, or description
- `extensions`: optional non-sequential information for rendering, interoperability, or future domains

### Core Operation Type

The generic core is `type: "gate"`:

```json
{
  "type": "gate",
  "gate": "X",
  "targets": [1],
  "controls": [0],
  "control_configs": [true],
  "params": [],
  "display": {
    "label": "X"
  }
}
```

This keeps the protocol useful for ordinary circuit tools that only care about gate operations.

### Standard Extension Operation Types

The protocol should explicitly define these additional operation kinds:

- `repeat`
- `tick`
- `qubit_coords`
- `shift_coords`
- `detector`
- `observable_include`
- `noise`
- `annotation`

These should still live in `operations`, not in separate side arrays, because they affect execution order and interpretation.

### Raw Target Preservation

Stim-style targets cannot always be reduced to plain qubit indices. The protocol should therefore allow an extension field such as `raw_targets` on operations that need it. Example target items include:

```json
{ "kind": "qubit", "index": 3 }
{ "kind": "qubit", "index": 3, "inverted": true }
{ "kind": "rec", "offset": -1 }
{ "kind": "pauli", "basis": "X", "qubit": 5, "inverted": false }
{ "kind": "combiner" }
{ "kind": "sweep", "index": 7 }
```

This preserves meaning without pretending every operation is a normal gate application.

## Timeline Rendering Model

The Typst package should not draw directly from raw JSON items. It should first normalize the JSON document into a render model containing:

- qubit lanes
- moment boundaries
- expanded or grouped repeat regions
- event anchors
- annotation groups
- optional coordinate metadata

This intermediate model is the key to keeping the first renderer and the future layout renderer compatible.

### Repeat Handling

The timeline renderer may expand repeat bodies to display repeated rounds clearly, but it should preserve repeat grouping visually with a frame or label such as `repeat × N`. The exported JSON itself should never flatten repeat blocks by default.

### Detector And Observable Handling

Detectors and observables should not be drawn as ordinary gates. They should appear on annotation tracks with labels and lightweight connectors to the measurements or sources they reference. The renderer can simplify the visual encoding as long as the existence and identity of these annotations remain visible.

### Coordinate And Shift Handling

`qubit_coords` and `shift_coords` should be present in the render model even if the timeline renderer only shows them as text or badges. The later layout renderer will consume the same data to place qubits geometrically.

## Deliverables

The implementation should produce:

1. A revised draft of `/Users/nzy/mcode/QProtocal/QSTD101-ZY.md`
2. A structured exporter in `rstim`
3. A CLI path for writing QSTD101 JSON to disk
4. A Typst package under `/Users/nzy/tycode`
5. A first timeline renderer with examples
6. Test coverage for protocol examples, exporter output, CLI output, and Typst compilation

## Risks

### 1. Blurred Core vs Extension Boundaries

If core execution fields and renderer hints are mixed freely, the protocol will become inconsistent and hard to evolve. The implementation should keep execution order in `operations` and non-sequential rendering or interoperability hints in `extensions`.

### 2. Overloaded Timeline Diagrams

If every semantic item is rendered with equal weight, QEC circuits will become unreadable. The renderer must separate main-track operations from annotation-track elements.

### 3. Premature Layout Coupling

If the first timeline renderer hardcodes assumptions that discard coordinate data, the second renderer will require protocol changes. The render normalization phase should therefore carry both timeline and coordinate metadata from the beginning.

## Verification Strategy

Verification should cover four sample classes:

1. a minimal generic circuit using only ordinary gates
2. a typical Stim/QEC circuit using `REPEAT`, `TICK`, and annotations
3. hard-target circuits that require `raw_targets`
4. real generated circuits such as `repetition_code` and `surface_code`

Rust tests should lock down exported JSON structure with stable assertions or snapshots. Typst verification should compile example documents successfully and, where useful, preserve visual fixtures for representative circuits.

## Next Step

The next implementation phase should start by freezing the draft protocol text, then implementing the exporter in `rstim`, then adding the Typst package and timeline renderer on top of the exported JSON.
