# QSTD101 Viz Semantic Anchor Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add global measurement anchors to the local `qstd101-viz` renderer so detectors and observables resolve `rec[-k]` sources into stable human-readable measurement ids.

**Architecture:** Keep the public Typst API unchanged, extend the internal render model with measurement history, and resolve `detector` / `observable_include` source references against that history after `repeat` expansion. Verify semantics using small SVG-based fixtures and final PDF compilation on a real `rstim` export fixture.

**Tech Stack:** Typst, quill/tequila, local JSON fixtures, `typst compile --format svg`, `typst compile`

---

### Task 1: Add Minimal Anchor Fixtures

**Files:**
- Create: `/Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/examples/anchor-basic.qstd101.json`
- Create: `/Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/examples/anchor-basic.typ`
- Create: `/Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/examples/anchor-repeat.qstd101.json`
- Create: `/Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/examples/anchor-repeat.typ`

**Step 1: Write the failing single-measurement fixture**

Create `anchor-basic.qstd101.json` with one measurement and one detector:

```json
{
  "standard": "QSTD101-ZY",
  "version": "1.0",
  "num_qubits": 1,
  "operations": [
    { "type": "gate", "gate": "M", "targets": [0] },
    {
      "type": "detector",
      "sources": [{ "kind": "rec", "offset": -1 }]
    }
  ]
}
```

Create `anchor-basic.typ` that imports `../lib.typ` and renders that file with `qstd101-timeline-file`.

**Step 2: Run the SVG render and verify the current renderer fails semantically**

Run:

```bash
/Users/nzy/rcode/rstim/tmp/tools/bin/typst compile --format svg --root /Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz /Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/examples/anchor-basic.typ /Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/examples/anchor-basic.svg
rg -n "m1|rec\\[-1\\]" /Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/examples/anchor-basic.svg
```

Expected before implementation: the SVG should still show `rec[-1]` and should not show `m1`.

**Step 3: Write the repeat fixture**

Create `anchor-repeat.qstd101.json` with two repeated measurements and a detector using multiple `rec` offsets. The goal is to force global numbering across expanded rounds.

**Step 4: Commit the fixtures**

```bash
git add /Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/examples/anchor-basic.qstd101.json /Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/examples/anchor-basic.typ /Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/examples/anchor-repeat.qstd101.json /Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/examples/anchor-repeat.typ
git commit -m "test: add qstd101 semantic anchor fixtures"
```

### Task 2: Extend The Render Model With Measurement History

**Files:**
- Modify: `/Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/lib.typ`
- Test: `/Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/examples/anchor-basic.typ`

**Step 1: Write the failing internal expectation first**

Before touching rendering output, change the SVG verification command into an expected semantic check:

```bash
/Users/nzy/rcode/rstim/tmp/tools/bin/typst compile --format svg --root /Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz /Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/examples/anchor-basic.typ /Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/examples/anchor-basic.svg
rg -n "m1" /Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/examples/anchor-basic.svg
```

Expected: FAIL because no global measurement anchor exists yet.

**Step 2: Add a measurement-history builder**

In `lib.typ`, extend the moment normalization phase so it also records measurement-producing operations in visual order. Each measurement-history entry should include:

- `anchor`
- `moment_index`
- `qubit`
- `gate`

**Step 3: Limit first-pass support to explicit measurement-producing gates**

At minimum, recognize:

- `M`
- `MX`
- `MR`

Do not assign anchors to plain `R` or `RX`.

**Step 4: Re-run the single-fixture SVG compile**

Run:

```bash
/Users/nzy/rcode/rstim/tmp/tools/bin/typst compile --format svg --root /Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz /Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/examples/anchor-basic.typ /Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/examples/anchor-basic.svg
```

Expected: PASS.

**Step 5: Commit**

```bash
git add /Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/lib.typ
git commit -m "feat: track qstd101 measurement anchors"
```

### Task 3: Render Anchor Labels On Measurement Gates

**Files:**
- Modify: `/Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/lib.typ`
- Test: `/Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/examples/anchor-basic.typ`

**Step 1: Write the failing semantic check**

Run:

```bash
rg -n "m1" /Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/examples/anchor-basic.svg
```

Expected before rendering update: FAIL or missing anchor text in the measurement gate region.

**Step 2: Add anchor badges to measurement gates**

Update the render path for `M`, `MX`, and `MR` so each emitted measurement record displays a small visual label such as `m1`, `m2`, etc.

For multiple targets in one operation, split them into individual measurement renders so each record gets its own anchor.

**Step 3: Re-run the SVG compile and grep**

Run:

```bash
/Users/nzy/rcode/rstim/tmp/tools/bin/typst compile --format svg --root /Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz /Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/examples/anchor-basic.typ /Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/examples/anchor-basic.svg
rg -n "m1" /Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/examples/anchor-basic.svg
```

Expected: PASS with `m1` visible.

**Step 4: Commit**

```bash
git add /Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/lib.typ
git commit -m "feat: render measurement anchor labels"
```

### Task 4: Resolve Detector And Observable Sources

**Files:**
- Modify: `/Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/lib.typ`
- Test: `/Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/examples/anchor-basic.typ`
- Test: `/Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/examples/anchor-repeat.typ`

**Step 1: Write the failing detector-source check**

Run:

```bash
/Users/nzy/rcode/rstim/tmp/tools/bin/typst compile --format svg --root /Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz /Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/examples/anchor-basic.typ /Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/examples/anchor-basic.svg
rg -n "det m1|det rec\\[-1\\]" /Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/examples/anchor-basic.svg
```

Expected before implementation: `det rec[-1]` still appears, `det m1` does not.

**Step 2: Resolve `rec[-k]` against measurement history**

Update detector and observable label generation so:

- `rec[-k]` resolves to the appropriate global anchor
- non-`rec` sources remain textual
- unresolved `rec` sources render explicitly as unresolved

Compress consecutive anchors into ranges only if they are strictly contiguous.

**Step 3: Verify repeat behavior**

Run:

```bash
/Users/nzy/rcode/rstim/tmp/tools/bin/typst compile --format svg --root /Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz /Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/examples/anchor-repeat.typ /Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/examples/anchor-repeat.svg
rg -n "m1|m2|m3|rec\\[" /Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/examples/anchor-repeat.svg
```

Expected: anchors increase globally across expanded rounds and raw `rec[...]` is no longer the normal display path.

**Step 4: Commit**

```bash
git add /Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/lib.typ
git commit -m "feat: resolve detector sources to measurement anchors"
```

### Task 5: Re-Verify Real rstim Fixture And Docs

**Files:**
- Modify: `/Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/README.md`
- Test: `/Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/examples/rstim-fixture.typ`

**Step 1: Update README**

Document:

- global measurement anchors
- detector/observable source resolution
- supported measurement-producing gates in the first pass

**Step 2: Compile the real fixture to PDF**

Run:

```bash
/Users/nzy/rcode/rstim/tmp/tools/bin/typst compile --root /Users/nzy/rcode/rstim /Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/examples/rstim-fixture.typ /Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/examples/rstim-fixture-semantic-anchor.pdf
```

Expected: PASS.

**Step 3: Compile the real fixture to SVG for text inspection**

Run:

```bash
/Users/nzy/rcode/rstim/tmp/tools/bin/typst compile --format svg --root /Users/nzy/rcode/rstim /Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/examples/rstim-fixture.typ /Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/examples/rstim-fixture-semantic-anchor.svg
rg -n "det m|obs\\[" /Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/examples/rstim-fixture-semantic-anchor.svg
```

Expected: PASS with resolved detector/observable anchor text visible.

**Step 4: Commit**

```bash
git add /Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/README.md
git commit -m "docs: describe qstd101 semantic anchor rendering"
```
