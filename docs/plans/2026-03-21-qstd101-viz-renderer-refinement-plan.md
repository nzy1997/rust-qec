# QSTD101 Viz Renderer Refinement Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Refine the local `qstd101-viz` Typst prototype so the circuit output looks closer to the `yao-rs/visualization` reference while still preserving QSTD101 annotation semantics.

**Architecture:** Keep the public API stable (`timeline-theme`, `qstd101-timeline`, `qstd101-timeline-file`) but change the internal render model from a table-like timeline into a quill/tequila circuit renderer. Use `tick` and `repeat` to define moment boundaries, render ordinary operations on the main qubit wires, and move metadata such as coordinates, detectors, and observables onto lightweight annotation wires. Prefer visual simplification over forcing every semantic detail into a heavy gate box.

**Tech Stack:** Typst, quill/tequila, local example JSON, local `typst compile`

---

### Task 1: Freeze A Concrete Visual Target

**Files:**
- Inspect: `/Users/nzy/rcode/yao-rs/visualization/circuit.typ`
- Inspect: `/Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/lib.typ`
- Test: `/Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/examples/timeline.typ`

**Step 1: List the mismatches to fix**

Capture the specific differences from the current renderer:

- current output is too table-like
- gate placement needs to follow quill packing
- annotation content needs to move off the main wires
- repeat and tick need lighter separators

**Step 2: Compile the current example**

Run:

```bash
/Users/nzy/rcode/rstim/tmp/tools/bin/typst compile --root /Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz /Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/examples/timeline.typ /Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/examples/timeline.pdf
```

Expected: PASS.

**Step 3: Keep the public API fixed**

Do not rename:

- `timeline-theme`
- `qstd101-timeline`
- `qstd101-timeline-file`

### Task 2: Improve The Main Circuit Track

**Files:**
- Modify: `/Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/lib.typ`
- Test: `/Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/examples/timeline.typ`

**Step 1: Map common gates to tequila helpers**

Handle at least:

- `H`
- `X`
- `Y`
- `Z`
- `S`
- `T`
- `CX`
- `CZ`
- `SWAP`
- `M`

Fallback to generic quill gates for anything else.

**Step 2: Treat common Stim measurement/reset forms more intentionally**

Add dedicated display logic for:

- `R`
- `RX`
- `MR`
- `MX`

Expected display strategy:

- resets stay visually lightweight
- measurements look like measurements, not ordinary opaque boxes

**Step 3: Keep multi-qubit spans readable**

Use quill multi-qubit gates for contiguous ranges, and preserve holes with `pass-through`.

**Step 4: Compile the example**

Run:

```bash
/Users/nzy/rcode/rstim/tmp/tools/bin/typst compile --root /Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz /Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/examples/timeline.typ /Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/examples/timeline-main-track.pdf
```

Expected: PASS.

### Task 3: Soften Tick And Repeat Presentation

**Files:**
- Modify: `/Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/lib.typ`
- Test: `/Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/examples/repeat-detector.qstd101.json`

**Step 1: Represent `tick` as a visual separator moment**

Make `tick` visually lighter than a normal gate moment.

**Step 2: Represent `repeat` boundaries as annotations, not blocks that dominate the circuit**

Show:

- `repeat xN`
- `iter k`
- `end repeat`

but keep them lightweight.

**Step 3: Compile the repeat example**

Run:

```bash
/Users/nzy/rcode/rstim/tmp/tools/bin/typst compile --root /Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz /Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/examples/timeline.typ /Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/examples/timeline-repeat.pdf
```

Expected: PASS.

### Task 4: Reduce Annotation Density

**Files:**
- Modify: `/Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/lib.typ`
- Modify: `/Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/README.md`
- Test: `/Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/examples/rstim-fixture.typ`

**Step 1: Keep annotations on dedicated top and bottom wires**

Top wire:

- `qubit_coords`
- `shift_coords`
- `annotation`
- repeat/tick labels when needed

Bottom wire:

- `detector`
- `observable_include`

**Step 2: Collapse multiple annotation strings within one moment**

Join multiple labels into a single lightweight text item per annotation wire per moment.

**Step 3: Update README to match the actual renderer**

Document:

- main track via quill
- annotation wires
- known limitations

**Step 4: Compile a real fixture**

Run:

```bash
/Users/nzy/rcode/rstim/tmp/tools/bin/typst compile --root /Users/nzy/rcode/rstim /Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/examples/rstim-fixture.typ /Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/examples/rstim-fixture-refined.pdf
```

Expected: PASS.

### Task 5: Verify And Preserve Artifacts

**Files:**
- Verify: `/Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/lib.typ`
- Verify: `/Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/README.md`
- Verify: `/Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/examples/timeline.pdf`
- Verify: `/Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/examples/rstim-fixture-refined.pdf`

**Step 1: Run the full local verification set**

Run:

```bash
/Users/nzy/rcode/rstim/tmp/tools/bin/typst compile --root /Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz /Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/examples/timeline.typ /Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/examples/timeline-final.pdf
/Users/nzy/rcode/rstim/tmp/tools/bin/typst compile --root /Users/nzy/rcode/rstim /Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/examples/rstim-fixture.typ /Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/examples/rstim-fixture-final.pdf
```

Expected: both PASS.

**Step 2: Review the file tree**

Confirm the package still contains:

- `typst.toml`
- `lib.typ`
- `README.md`
- example `.typ`
- example `.json`

**Step 3: Leave sync-ready outputs in `tmp`**

Do not move files yet. Keep the updated local package in:

`/Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz`
