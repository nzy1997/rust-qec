# QSTD101 Viz Stim-Style Operator Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Remove fake annotation wires from the local `qstd101-viz` timeline renderer, suppress unused metadata from the circuit view, and render `detector` / `observable_include` as Stim-style inline circuit operators.

**Architecture:** Keep the public Typst API unchanged, but refactor moment normalization so detector / observable items become main-track operator entries instead of bottom-lane text. Reuse the existing measurement-anchor resolution logic, derive a Stim-style host wire from resolved source qubits, and render compact operator boxes with Stim-like text on that host wire.

**Tech Stack:** Typst, quill/tequila, local query-based fixture checks, `typst compile`

---

### Task 1: Freeze Stim-Style Operator Expectations

**Files:**
- Create: `/Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/examples/stim-operator-host.qstd101.json`
- Create: `/Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/examples/stim-operator-host.typ`
- Create: `/Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/examples/stim-operator-forward.qstd101.json`
- Create: `/Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/examples/stim-operator-forward.typ`

**Step 1: Write the host-wire fixture**

Create a focused fixture where measurements occur on `q0` and `q1` and the later operator text can be checked against the chosen Stim-style examples:

- detector source label target: `D0 = m2*m1`
- observable source label target: `L0 *= m7`

The fixture must make it possible to verify:

- detector host wire is `q0`
- observable host wire is `q1`
- detector text uses box label `DETECTOR` with source label `D0 = ...`
- observable text uses box label `OBS_INCLUDE(0)` with source label `L0 *= ...`

**Step 2: Write the forward-reference fixture**

Create a minimal fixture proving that `rec[0]` still renders as unresolved after the operator refactor.

**Step 3: Add failing query checks**

Use `typst query` fixtures so the current implementation fails before the refactor because detector / observable items are still bottom text, not main-track operators.

**Step 4: Commit**

```bash
git add /Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/examples/stim-operator-host.qstd101.json /Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/examples/stim-operator-host.typ /Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/examples/stim-operator-forward.qstd101.json /Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/examples/stim-operator-forward.typ
git commit -m "test: add qstd101 stim-operator fixtures"
```

### Task 2: Remove Fake Annotation Wires

**Files:**
- Modify: `/Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/lib.typ`
- Test: `/Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/examples/anchor-basic.typ`

**Step 1: Write the failing label expectation**

Add a query-based check or render-model check proving the circuit still allocates extra `meta` / `ann` wires before the change.

**Step 2: Refactor wire layout**

Update `qstd101-timeline(...)` and `wire-label-items(...)` so:

- total wires equals `num_qubits`
- wire labels are only `q0`, `q1`, ...
- the rendered qubit rows match the actual qubit indices

**Step 3: Stop showing unused metadata in the timeline**

Remove `qubit_coords` and `shift_coords` from the visible circuit path. Keep parsing semantics intact, but do not emit visual note operators for them.

**Step 4: Re-run the local compile**

Run:

```bash
/Users/nzy/rcode/rstim/tmp/tools/bin/typst compile --format svg --root /Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz /Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/examples/anchor-basic.typ /Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/examples/anchor-basic.svg
```

Expected: PASS, with no fake annotation wires left.

**Step 5: Commit**

```bash
git add /Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/lib.typ
git commit -m "refactor: remove qstd101 annotation wires"
```

### Task 3: Promote Detector And Observable Into Main-Track Operators

**Files:**
- Modify: `/Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/lib.typ`
- Test: `/Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/examples/stim-operator-host.typ`

**Step 1: Write the failing operator-model query**

Verify the render model does not yet place detector / observable entries into `moment.main`.

**Step 2: Add structured operator entries**

Refactor moment normalization so detector / observable instructions are emitted into `moment.main` as structured operator entries, carrying at least:

- `kind`
- `host_qubit`
- `text`
- `sources`

**Step 3: Derive host wire using Stim's rule**

Host-wire selection must use the minimum qubit from resolved measurement sources. If no resolved measurement source exists, use the best available fallback qubit; otherwise fall back to `q0`.

**Step 4: Re-run the query checks**

Run the new query fixture and verify detector / observable entries now appear in `moment.main` with the expected host qubits.

**Step 5: Commit**

```bash
git add /Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/lib.typ
git commit -m "feat: promote qstd101 detector operators into main track"
```

### Task 4: Render Stim-Style Detector And Observable Operators

**Files:**
- Modify: `/Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/lib.typ`
- Test: `/Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/examples/stim-operator-host.typ`
- Test: `/Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/examples/stim-operator-forward.typ`

**Step 1: Write the failing render-text query**

Add a query-based check proving the rendered operator text is not yet in Stim-style form.

**Step 2: Implement the operator renderer**

Add a dedicated render path for detector / observable main-track entries:

- detector box label: `DETECTOR`
- detector source label: `D<index> = ...`
- observable box label: `OBS_INCLUDE(<index>)`
- observable source label: `L<index> *= ...`
- source separators: `*`
- anchor ranges: preserve current compression behavior
- unresolved references: remain explicit

**Step 3: Render as single-wire operators**

Draw detector / observable items as lightweight single-wire gates on the host wire. Do not draw spans or connector lines.

**Step 4: Re-run the local checks**

Run:

```bash
/Users/nzy/rcode/rstim/tmp/tools/bin/typst compile --format svg --root /Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz /Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/examples/anchor-basic.typ /Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/examples/anchor-basic.svg
/Users/nzy/rcode/rstim/tmp/tools/bin/typst compile --format svg --root /Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz /Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/examples/anchor-repeat.typ /Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/examples/anchor-repeat.svg
```

Expected: PASS, with detector / observable now appearing on the circuit instead of a removed bottom lane.

**Step 5: Commit**

```bash
git add /Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/lib.typ
git commit -m "feat: render qstd101 stim-style detector operators"
```

### Task 5: Update Docs And Re-Verify Real Fixture

**Files:**
- Modify: `/Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/README.md`
- Test: `/Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/examples/rstim-fixture.typ`

**Step 1: Update README**

Document:

- fake annotation wires removed
- unused metadata hidden from the circuit view
- detector / observable inline operator style
- Stim-style host-wire rule

**Step 2: Compile the real fixture**

Run:

```bash
/Users/nzy/rcode/rstim/tmp/tools/bin/typst compile --root /Users/nzy/rcode/rstim /Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/examples/rstim-fixture.typ /Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/examples/rstim-fixture-stim-operator.pdf
/Users/nzy/rcode/rstim/tmp/tools/bin/typst compile --format png --root /Users/nzy/rcode/rstim /Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/examples/rstim-fixture.typ /Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/examples/rstim-fixture-stim-operator.png
```

Expected: PASS.

**Step 3: Run a real-fixture query check**

Verify:

- detector / observable entries are in the main track
- no fake `meta` / `ann` labels remain
- resolved text matches the Stim-style operator format

**Step 4: Commit**

```bash
git add /Users/nzy/rcode/rstim/tmp/tycode/qstd101-viz/README.md
git commit -m "docs: describe qstd101 stim-style operator rendering"
```
