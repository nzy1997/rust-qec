# QP101-ZY: Quantum Circuit JSON Format

**Version:** 1.0
**Status:** Draft
**Contributors:** Design Team
**Date:** 2026-03-21

## Abstract

This standard defines a JSON format for representing quantum circuits in a framework-agnostic manner while preserving ordered execution semantics that cannot be reduced to a flat gate list. The format supports ordinary gates, parameterized operations, nested repeat blocks, explicit timing markers, coordinate annotations, detector and observable annotations, noise operations, and extension points for framework-specific metadata.

The design goal is "generic core + extensions": ordinary circuit tools can consume the core gate model, while richer quantum error-correction and visualization tools can preserve additional semantics without inventing a separate file format.

## Purpose

Quantum computing frameworks use diverse internal representations for circuits, making it difficult to share circuits between tools, validate implementations, and build interoperable visualization pipelines. This standard provides:

- a clear, human-readable JSON format for quantum circuits
- a generic gate model for ordinary circuit interchange
- an ordered `operations` stream for semantics that are not just gates
- extension points for framework-specific metadata and rendering hints
- generic per-operation annotations for visualization, debugging, and query results
- a foundation for visualization, analysis, and future layout-aware renderers

## Scope And Applicability

**This standard covers:**

- JSON representation of quantum circuits with ordered operations
- common gate operations and parameterized gates
- explicit timing separators such as `tick`
- nested repeat structure
- coordinate and annotation operations needed by visualization tools
- detector and observable annotations
- preservation of raw target forms when plain qubit indices are insufficient

**This standard does NOT cover:**

- measurement outcomes or classical post-processing data
- continuous-time evolution or Hamiltonian simulation
- optimization or compilation strategies
- hardware-specific scheduling constraints
- a canonical semantics for every possible custom extension

## Core JSON Document

A QP101-ZY document MUST contain:

```json
{
  "standard": "QP101-ZY",
  "version": "1.0",
  "num_qubits": 2,
  "operations": []
}
```

The top-level `gates` array used by earlier draft text is not part of this draft. Ordered execution is represented by `operations`.

### Top-Level Fields

- `standard` (required): Protocol identifier. MUST be `"QP101-ZY"`.
- `version` (required): Protocol version string. This draft remains at `"1.0"`.
- `num_qubits` (required): Total number of qubits in the circuit. MUST be an integer >= 1.
- `operations` (required): Ordered array of operation objects. May be empty for an identity circuit.
- `metadata` (optional): Framework-neutral metadata such as source tool, generator info, description, or authoring notes.
- `extensions` (optional): Non-sequential extension data for rendering, interoperability, or future domains.

Execution order lives in `operations`. Non-sequential or auxiliary data belongs in `metadata` or `extensions`.

## Operation Model

Each item in `operations` MUST contain a string `type` field. The protocol defines one generic core operation and a set of standard extension operations.

### Common Operation Fields

Any operation MAY carry an `annotations` array. This field is intended for renderer hints, analysis overlays, query results, and other optional markings that should stay attached to a specific operation instead of living in top-level extension state.

Example:

```json
{
  "type": "noise",
  "gate": "DEPOLARIZE1",
  "params": [0.001],
  "raw_targets": [
    { "kind": "qubit", "index": 5 }
  ],
  "annotations": [
    {
      "kind": "marker",
      "target_slots": [0],
      "label": "X",
      "text": "repeat[3]",
      "style": {
        "preset": "danger",
        "color": "red",
        "highlight": true
      },
      "tags": ["dem-origin", "query-result"],
      "context": {
        "dem_error_index": 17,
        "op_path": [12, 3, 5],
        "repeat_iterations": [3],
        "source_branch": "X",
        "target_qubits": [5]
      }
    }
  ]
}
```

Annotation objects use the following common fields:

- `kind` (required): Annotation subtype such as `"marker"` or `"note"`.
- `target_slots` (optional): Zero-based target-slot indices within the owning operation.
- `label` (optional): Short inline label such as `"X"`, `"Y"`, or `"repeat"`.
- `text` (optional): Longer renderer-facing text shown near the marker.
- `style` (optional): Generic style hints such as `preset`, `color`, and `highlight`.
- `tags` (optional): String tags for filtering or downstream processing.
- `context` (optional): Arbitrary machine-readable JSON payload for tool-specific metadata.

### Core Gate Operation

The generic core operation is `type: "gate"`:

```json
{
  "type": "gate",
  "gate": "CX",
  "targets": [1],
  "controls": [0],
  "control_configs": [true],
  "params": [],
  "display": {
    "label": "CX"
  },
  "tags": []
}
```

#### Gate Fields

- `type`: MUST be `"gate"`.
- `gate` (required): Gate name such as `"H"`, `"X"`, `"CX"`, `"RX"`, or a tool-defined gate identifier.
- `targets` (optional but usually present): Array of 0-indexed qubit indices addressed directly by the gate.
- `controls` (optional): Array of 0-indexed control qubit indices.
- `control_configs` (optional): Boolean array aligned with `controls`. `true` means active-high control and `false` means active-low control.
- `params` (optional): Ordered numeric parameter list.
- `raw_targets` (optional): Ordered raw target list for cases where plain `targets` and `controls` are insufficient.
- `display` (optional): Rendering hint object. The standard currently defines optional `display.label`.
- `tags` (optional): String tag list carried through from source tools.

A `gate` operation MUST include enough addressing information to interpret the operation. In practice this means at least one of:

- a non-empty `targets` array
- a non-empty `controls` array
- a non-empty `raw_targets` array

### Standard Extension Operations

The following operation kinds are standard extension operations. They still appear inside `operations` because they affect execution order, interpretation, or visualization.

#### `repeat`

```json
{
  "type": "repeat",
  "count": 2,
  "body": [
    { "type": "tick" },
    { "type": "gate", "gate": "CX", "targets": [0, 1] }
  ]
}
```

- `count` (required): Positive integer repeat count.
- `body` (required): Ordered array of nested operations.

`repeat` preserves hierarchical circuit structure and MUST NOT be flattened by default.

#### `tick`

```json
{ "type": "tick" }
```

`tick` is an explicit execution marker. It is not derived implicitly from column numbers or rendering layers.

#### `qubit_coords`

```json
{
  "type": "qubit_coords",
  "coords": [0.0, 1.0],
  "targets": [3]
}
```

- `coords` (required): Numeric coordinate vector.
- `targets` (required): One or more qubit indices whose coordinates are being declared.

#### `shift_coords`

```json
{
  "type": "shift_coords",
  "delta": [0.0, 1.0]
}
```

- `delta` (required): Numeric coordinate delta vector.

#### `detector`

```json
{
  "type": "detector",
  "coords": [1.0, 0.0, 0.0],
  "sources": [
    { "kind": "rec", "offset": -1 }
  ]
}
```

- `coords` (optional): Numeric coordinate vector. Empty coordinates may be encoded as `[]`.
- `sources` (required): Ordered source-reference array.

Detectors are annotations about circuit outputs and MUST NOT be coerced into ordinary gate entries.

#### `observable_include`

```json
{
  "type": "observable_include",
  "index": 0,
  "sources": [
    { "kind": "rec", "offset": -1 }
  ]
}
```

- `index` (required): Non-negative observable index.
- `sources` (required): Ordered source-reference array.

#### `noise`

```json
{
  "type": "noise",
  "gate": "X_ERROR",
  "params": [0.001],
  "raw_targets": [
    { "kind": "qubit", "index": 5 }
  ]
}
```

- `gate` (required): Noise or fault operator name.
- `params` (optional): Numeric parameter list.
- `raw_targets` (required): Ordered raw target list.

#### `annotation`

```json
{
  "type": "annotation",
  "kind": "comment",
  "text": "Ancilla preparation region"
}
```

- `kind` (required): Annotation subtype identifier.
- `text` (required): Human-readable annotation payload.

## Raw Targets And Source References

Some frameworks use target forms that cannot be reduced to plain qubit indices. QP101-ZY preserves them through explicit target-reference objects. These objects are used:

- in `raw_targets`
- in detector `sources`
- in observable `sources`

### Target-Reference Kinds

#### Qubit Target

```json
{ "kind": "qubit", "index": 3 }
```

Optional inversion may be represented as:

```json
{ "kind": "qubit", "index": 3, "inverted": true }
```

#### Measurement Record Reference

```json
{ "kind": "rec", "offset": -1 }
```

#### Pauli Product Target

```json
{ "kind": "pauli", "basis": "X", "qubit": 5 }
```

Optional inversion may be represented as:

```json
{ "kind": "pauli", "basis": "Z", "qubit": 7, "inverted": true }
```

Allowed `basis` values are `"X"`, `"Y"`, and `"Z"`.

#### Combiner

```json
{ "kind": "combiner" }
```

#### Sweep Target

```json
{ "kind": "sweep", "index": 7 }
```

## Validation Rules

A conforming document MUST satisfy all of the following:

1. `standard` MUST equal `"QP101-ZY"`.
2. `version` MUST equal `"1.0"` for this draft.
3. `num_qubits` MUST be an integer >= 1.
4. All qubit indices in `targets`, `controls`, `qubit` target references, and `qubit_coords.targets` MUST be in `[0, num_qubits)`.
5. `repeat.count` MUST be an integer >= 1.
6. `repeat.body` MUST be present and MUST be an array.
7. `tick` MUST NOT carry qubit indices, raw targets, or parameters.
8. `detector.sources` and `observable_include.sources` MUST be explicit source-reference arrays.
9. `observable_include.index` MUST be a non-negative integer.
10. `control_configs`, if present, MUST have the same length as `controls`.
11. `noise.raw_targets` MUST be present and MUST preserve target order.
12. `raw_targets` MUST preserve data that cannot be losslessly normalized into plain qubit indices.

## Examples

### Example A: Ordinary Gate-Only Circuit

```json
{
  "standard": "QP101-ZY",
  "version": "1.0",
  "num_qubits": 2,
  "operations": [
    { "type": "gate", "gate": "H", "targets": [0] },
    {
      "type": "gate",
      "gate": "X",
      "targets": [1],
      "controls": [0],
      "control_configs": [true]
    },
    { "type": "gate", "gate": "M", "targets": [0, 1] }
  ],
  "metadata": {
    "source": "generic-demo"
  }
}
```

### Example B: Stim-Style QEC Circuit

```json
{
  "standard": "QP101-ZY",
  "version": "1.0",
  "num_qubits": 2,
  "operations": [
    { "type": "qubit_coords", "coords": [0.0, 0.0], "targets": [0] },
    { "type": "qubit_coords", "coords": [1.0, 0.0], "targets": [1] },
    { "type": "tick" },
    {
      "type": "repeat",
      "count": 2,
      "body": [
        { "type": "gate", "gate": "CX", "targets": [0, 1] },
        { "type": "tick" },
        { "type": "gate", "gate": "M", "targets": [1] },
        {
          "type": "detector",
          "coords": [0.5, 0.0, 0.0],
          "sources": [
            { "kind": "rec", "offset": -1 }
          ]
        }
      ]
    },
    {
      "type": "observable_include",
      "index": 0,
      "sources": [
        { "kind": "rec", "offset": -1 }
      ]
    }
  ],
  "metadata": {
    "framework": "rstim"
  }
}
```

## Rendering Guidance

This standard does not mandate a single renderer, but it is designed to support:

- timeline renderers with qubits on the vertical axis and operations on the horizontal axis
- future coordinate-layout renderers that consume `qubit_coords`, `shift_coords`, and annotation coordinates

Renderers SHOULD treat `detector`, `observable_include`, `qubit_coords`, and `shift_coords` as annotation-like elements rather than ordinary gate boxes. Renderers MAY expand `repeat` blocks for display, but serialized documents SHOULD preserve the nested structure.

## Compatibility Notes

- Tools that only understand generic circuits MAY ignore unknown extension operations, provided they do not claim full semantic fidelity.
- Tools that need full semantic fidelity SHOULD preserve unknown fields and unknown operation types when possible.
- Draft implementations SHOULD prefer adding information through optional fields or standard extension operations instead of inventing a second top-level execution stream.
