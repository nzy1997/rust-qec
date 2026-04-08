# Noise Gate Rendering Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Render known noise gates with correct arity-aware shapes and compact labels/parameter notes without changing the QP101 JSON schema.

**Architecture:** Add a renderer-side noise classification layer in `lib.typ` so `render-main-op` delegates noise ops to dedicated helpers. Keep `noise` JSON untouched, derive singleton and pair groups from `raw_targets`, and verify behavior with focused Typst query fixtures plus representative circuit compilations.

**Tech Stack:** Typst 0.14.x, Quill 0.7.2, local JSON fixtures under `checks/` and `examples/circuits/`

---

### Task 1: Add Noise Semantic Fixtures And Helper Functions

**Files:**
- Create: `checks/noise-render.qp101.json`
- Create: `checks/noise-render.typ`
- Modify: `lib.typ`
- Test: `checks/noise-render.typ`

**Step 1: Write the failing test**

Create `checks/noise-render.qp101.json`:

```json
{
  "standard": "QP101-ZY",
  "version": "1.0",
  "num_qubits": 5,
  "operations": [
    {
      "type": "noise",
      "gate": "X_ERROR",
      "params": [0.001],
      "raw_targets": [
        { "kind": "qubit", "index": 0 }
      ]
    },
    { "type": "tick" },
    {
      "type": "noise",
      "gate": "DEPOLARIZE1",
      "params": [0.001],
      "raw_targets": [
        { "kind": "qubit", "index": 0 },
        { "kind": "qubit", "index": 2 },
        { "kind": "qubit", "index": 4 }
      ]
    },
    { "type": "tick" },
    {
      "type": "noise",
      "gate": "DEPOLARIZE2",
      "params": [0.001],
      "raw_targets": [
        { "kind": "qubit", "index": 1 },
        { "kind": "qubit", "index": 3 }
      ]
    }
  ]
}
```

Create `checks/noise-render.typ`:

```typ
#import "../lib.typ": collect-render-model, noise-render-spec, render-main-op, timeline-theme

#let doc = json("noise-render.qp101.json")
#let ops = doc.at("operations", default: ())
#let model = collect-render-model(ops)
#let theme = timeline-theme()

#metadata((
  x_error: noise-render-spec(ops.at(0)),
  depolarize1: noise-render-spec(ops.at(2)),
  depolarize2: noise-render-spec(ops.at(4)),
)) <noise-render-spec>

#let single = render-main-op(model.moments.at(0).main.at(0), theme)
#let batched = render-main-op(model.moments.at(2).main.at(0), theme)
#let paired = render-main-op(model.moments.at(4).main.at(0), theme)

#metadata((
  single: (
    count: single.len(),
    qubits: single.map(op => op.first().qubit),
    spans: single.map(op => op.first().n),
    supplement_counts: single.map(op => op.first().supplements.len()),
  ),
  batched: (
    count: batched.len(),
    qubits: batched.map(op => op.first().qubit),
    spans: batched.map(op => op.first().n),
    supplement_counts: batched.map(op => op.first().supplements.len()),
  ),
  paired: (
    count: paired.len(),
    lead_qubit: paired.first().first().qubit,
    span: paired.first().first().n,
    supplement_count: paired.first().first().supplements.len(),
  ),
)) <noise-render-structure>
```

**Step 2: Run test to verify it fails**

Run:

```bash
typst query checks/noise-render.typ "<noise-render-spec>" --one
```

Expected: FAIL because `noise-render-spec` is not defined/exported yet.

**Step 3: Write minimal implementation**

Add semantic helpers near the existing gate-label helpers in `lib.typ`:

```typ
#let noise-target-qubits(op) = shifted-qubits(qubits-from-refs(op.at("raw_targets", default: ())))

#let noise-short-label(op) = {
  let display = op.at("display", default: none)
  if display != none and display.at("label", default: none) != none {
    return display.at("label")
  }

  let gate = op.at("gate", default: "")
  if gate == "X_ERROR" { return "XE" }
  if gate == "Z_ERROR" { return "ZE" }
  if gate == "DEPOLARIZE1" { return "D1" }
  if gate == "DEPOLARIZE2" { return "D2" }
  gate
}

#let noise-note-label(op) = {
  let params = op.at("params", default: ())
  if params.len() == 0 {
    return none
  }
  "p=" + params.map(v => fmt-num(v)).join(", ")
}

#let noise-policy(op) = {
  let gate = op.at("gate", default: "")
  if gate == "X_ERROR" or gate == "Z_ERROR" or gate == "DEPOLARIZE1" {
    return "single"
  }
  if gate == "DEPOLARIZE2" {
    return "pair"
  }
  "fallback"
}

#let noise-qubit-groups(op) = {
  let qubits = noise-target-qubits(op)
  let policy = noise-policy(op)

  if policy == "single" {
    return qubits.map(q => (q,))
  }
  if policy == "pair" and calc.rem(qubits.len(), 2) == 0 {
    let groups = ()
    for pair in range(calc.floor(qubits.len() / 2)) {
      let i = pair * 2
      groups.push((qubits.at(i), qubits.at(i + 1)))
    }
    return groups
  }
  ()
}

#let noise-render-spec(op) = (
  policy: noise-policy(op),
  short_label: noise-short-label(op),
  note: noise-note-label(op),
  groups: noise-qubit-groups(op),
)
```

**Step 4: Run test to verify it passes**

Run:

```bash
typst query checks/noise-render.typ "<noise-render-spec>" --one
```

Expected: JSON metadata showing:
- `x_error.policy = "single"` and `x_error.short_label = "XE"`
- `depolarize1.policy = "single"` and `depolarize1.short_label = "D1"`
- `depolarize2.policy = "pair"` and `depolarize2.short_label = "D2"`
- all three notes equal `p=0.001`

**Step 5: Commit**

```bash
git add checks/noise-render.qp101.json checks/noise-render.typ lib.typ
git commit -m "feat: add noise render spec helpers"
```

### Task 2: Render Single-Qubit Noise As Compact Boxes

**Files:**
- Modify: `lib.typ`
- Test: `checks/noise-render.typ`

**Step 1: Write the failing test**

Run:

```bash
typst query checks/noise-render.typ "<noise-render-structure>" --one
```

Expected: FAIL semantically because current structure still reports one rendered op for the batched `DEPOLARIZE1` noise moment instead of three single-wire ops.

**Step 2: Run test to verify it fails**

Inspect the output and confirm that:
- `single.count` is not yet guaranteed to be `1` through the new noise path
- `batched.count` is currently `1` instead of `3`

**Step 3: Write minimal implementation**

Add compact noise-box helpers and wire them into a dedicated renderer path:

```typ
#let noise-note-label-entry(note, theme) = {
  if note == none {
    return ()
  }
  (
    (
      content: text(size: theme.note_font_size - 1pt, fill: theme.note_color)[#note],
      pos: top,
      dy: top-label-clearance(theme),
    ),
  )
}

#let noise-box-constructor(label, theme, note: none, target: none) = (x: auto, y: auto) => gate(
  text(size: theme.note_font_size, fill: theme.color)[#label],
  x: x,
  y: y,
  fill: theme.noise_fill,
  stroke: .6pt + theme.note_color,
  width: reserved-gate-width((
    estimated-text-width(label, theme.note_font_size, padding: 0.7em),
  ), minimum: 1.9em),
  label: noise-note-label-entry(note, theme),
  multi: if target == none { none } else {(
    target: target,
    num-qubits: calc.abs(target) + 1,
    wire-count: 1,
    wire-stroke: auto,
    label: none,
    extent: auto,
    size-all-wires: false,
    inputs: none,
    outputs: none,
    wire-label: (),
    pass-through: (),
  )},
)

#let noise-single-op(qubit, label, theme, note: none) = (
  (
    qubit: qubit,
    n: 1,
    supplements: (),
    constructor: noise-box-constructor(label, theme, note: note),
  ),
)

#let render-noise-op(op, theme) = {
  let spec = noise-render-spec(op)
  if spec.policy == "single" and spec.groups.len() > 0 {
    let ops = ()
    for (index, group) in spec.groups.enumerate() {
      let note = if index == 0 { spec.note } else { none }
      ops.push(noise-single-op(group.first(), spec.short_label, theme, note: note))
    }
    return ops
  }

  let qubits = noise-target-qubits(op)
  let gate = generic-gate(qubits, gate-label(op), theme, fill: theme.noise_fill)
  if gate == none { return () }
  (gate,)
}
```

Then replace the current inline `kind == "noise"` branch in `render-main-op` with:

```typ
if kind == "noise" {
  return render-noise-op(op, theme)
}
```

**Step 4: Run test to verify it passes**

Run:

```bash
typst query checks/noise-render.typ "<noise-render-structure>" --one
```

Expected: metadata showing:
- `single.count = 1`, `single.qubits = (0,)`, `single.spans = (1,)`
- `batched.count = 3`, `batched.qubits = (0, 2, 4)`, `batched.spans = (1, 1, 1)`
- each single/batched entry has `supplement_counts = (0, ...)`

**Step 5: Commit**

```bash
git add lib.typ checks/noise-render.typ
git commit -m "feat: render single-qubit noise as compact boxes"
```

### Task 3: Render DEPOLARIZE2 As A Paired Two-Box Gate

**Files:**
- Create: `checks/noise-render-render.typ`
- Modify: `lib.typ`
- Test: `checks/noise-render.typ`
- Test: `checks/noise-render-render.typ`

**Step 1: Write the failing test**

Create `checks/noise-render-render.typ`:

```typ
#import "../lib.typ": qp101-timeline-file, timeline-theme

#set page(width: auto, height: auto, margin: 10pt)

#qp101-timeline-file(
  "checks/noise-render.qp101.json",
  theme: timeline-theme(step_width: 5.6em),
)
```

**Step 2: Run test to verify it fails**

Run:

```bash
typst query checks/noise-render.typ "<noise-render-structure>" --one
```

Expected: FAIL semantically because `paired` is still reported as the old generic spanning gate instead of one lead op with one supplement.

**Step 3: Write minimal implementation**

Extend `render-noise-op` with a paired-gate helper:

```typ
#let noise-pair-op(q1, q2, label, theme, note: none) = {
  let upper = calc.min(q1, q2)
  let lower = calc.max(q1, q2)
  (
    (
      qubit: upper,
      n: lower - upper + 1,
      supplements: (
        (
          lower,
          noise-box-constructor(label, theme),
        ),
      ),
      constructor: noise-box-constructor(
        label,
        theme,
        note: note,
        target: lower - upper,
      ),
    ),
  )
}
```

Then update `render-noise-op`:

```typ
if spec.policy == "pair" and spec.groups.len() > 0 {
  let ops = ()
  for (index, group) in spec.groups.enumerate() {
    let note = if index == 0 { spec.note } else { none }
    ops.push(noise-pair-op(group.at(0), group.at(1), spec.short_label, theme, note: note))
  }
  return ops
}
```

**Step 4: Run test to verify it passes**

Run:

```bash
typst query checks/noise-render.typ "<noise-render-structure>" --one
typst compile checks/noise-render-render.typ /tmp/qp101-viz-noise-render.pdf
```

Expected:
- `paired.count = 1`
- `paired.lead_qubit = 1`
- `paired.span = 3`
- `paired.supplement_count = 1`
- PDF compile succeeds and visually shows `D2` as two boxes connected by a vertical line, with one `p=0.001` note above the upper box

**Step 5: Commit**

```bash
git add lib.typ checks/noise-render.typ checks/noise-render-render.typ
git commit -m "feat: render depolarize2 as paired noise gate"
```

### Task 4: Verify Real Examples And Update Documentation

**Files:**
- Modify: `README.md`
- Test: `checks/noise-render.typ`
- Test: `checks/noise-render-render.typ`
- Test: `examples/circuits/render.typ`

**Step 1: Write the failing test**

Add a short renderer note to `README.md` once the behavior exists:

```md
- `X_ERROR`, `Z_ERROR`, and `DEPOLARIZE1` render as compact short-label single-qubit noise boxes, even when a single op targets many qubits.
- `DEPOLARIZE2` renders as a two-box connected noise gate with one parameter note above the pair.
```

**Step 2: Run test to verify it fails**

Run the full verification set before the README edit is applied:

```bash
typst query checks/noise-render.typ "<noise-render-spec>" --one
typst query checks/noise-render.typ "<noise-render-structure>" --one
typst compile checks/noise-render-render.typ /tmp/qp101-viz-noise-render.pdf
typst compile --input source=examples/circuits/steane_x_basis_with_flags.json examples/circuits/render.typ /tmp/qp101-viz-steane-noise.pdf
typst compile --input source=examples/circuits/surface_code_d3_with_flags.json examples/circuits/render.typ /tmp/qp101-viz-surface-noise.pdf
```

Expected: all Typst commands succeed and the two example PDFs show:
- `XE` and `D1` as compact single-wire noise boxes
- `D2` as paired two-box gates
- one `p=0.001` note above each logical noise group

**Step 3: Write minimal implementation**

Update the `README.md` notes section with the new rendering rules and keep the wording presentation-focused rather than schema-focused.

**Step 4: Run test to verify it passes**

Run:

```bash
typst query checks/noise-render.typ "<noise-render-spec>" --one
typst query checks/noise-render.typ "<noise-render-structure>" --one
typst compile checks/noise-render-render.typ /tmp/qp101-viz-noise-render.pdf
typst compile --input source=examples/circuits/steane_x_basis_with_flags.json examples/circuits/render.typ /tmp/qp101-viz-steane-noise.pdf
typst compile --input source=examples/circuits/surface_code_d3_with_flags.json examples/circuits/render.typ /tmp/qp101-viz-surface-noise.pdf
```

Expected: all commands succeed with the updated README committed alongside the rendering changes.

**Step 5: Commit**

```bash
git add README.md
git commit -m "docs: document noise gate rendering rules"
```
