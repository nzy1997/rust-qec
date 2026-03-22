# rstim / QSTD101 Visualization Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a draft QSTD101-based visualization pipeline that exports `rstim` circuits to JSON and renders them with a new Typst package, with timeline rendering delivered first and layout rendering preserved by the data model.

**Architecture:** Freeze the draft protocol first, then add a structured exporter and CLI entry point in `rstim`, then build a Typst package that normalizes the JSON into a render model before drawing a timeline diagram. Keep execution semantics in ordered `operations`, keep render hints in `extensions`, and preserve Stim-only semantics through explicit operation types and `raw_targets`.

**Tech Stack:** Rust, `serde`/`serde_json`, `clap`, `cargo test`, Typst package layout, `typst compile`, JSON fixtures, markdown protocol docs

---

## Preflight

- Worktree setup was attempted and blocked by sandbox permissions when creating a new git ref lock. If you execute this plan in a normal shell session, retry `using-git-worktrees` first and run the tasks in that isolated workspace.
- Design source of truth: `/Users/nzy/rcode/rstim/docs/plans/2026-03-21-rstim-qstd101-visualization-design.md`
- Protocol source of truth: `/Users/nzy/mcode/QProtocal/QSTD101-ZY.md`
- Typst package root to create: `/Users/nzy/tycode/qstd101-viz`
- Repository boundaries:
  - `rstim` code changes live in the git repo rooted at `/Users/nzy/rcode/rstim`
  - protocol-document changes live in the git repo rooted at `/Users/nzy/mcode/QProtocal`
  - `/Users/nzy/tycode/qstd101-viz` does not exist yet and should be created as its own git repo before the first Typst-package commit
- Keep the protocol version at `1.0`
- Do not flatten `REPEAT` in the exported JSON
- Use explicit `{ "type": "tick" }` items instead of only derived layer numbers

### Task 1: Rewrite The QSTD101 Draft Around `operations`

**Files:**
- Modify: `/Users/nzy/mcode/QProtocal/QSTD101-ZY.md`
- Test: `/Users/nzy/mcode/QProtocal/QSTD101-ZY.md`

**Step 1: Write the failing example first**

Add a draft example near the top of the document that the current text cannot explain:

```json
{
  "standard": "QSTD101-ZY",
  "version": "1.0",
  "num_qubits": 2,
  "operations": [
    { "type": "qubit_coords", "coords": [0, 0], "targets": [0] },
    { "type": "tick" },
    {
      "type": "repeat",
      "count": 2,
      "body": [
        { "type": "gate", "gate": "CX", "targets": [1], "controls": [0] },
        { "type": "detector", "sources": [{ "kind": "rec", "offset": -1 }] }
      ]
    }
  ]
}
```

Expected: the surrounding prose is obviously inconsistent because it still defines only `gates`.

**Step 2: Rewrite the top-level schema section**

Replace the `num_qubits + gates` definition with:

```json
{
  "standard": "QSTD101-ZY",
  "version": "1.0",
  "num_qubits": 2,
  "operations": []
}
```

Document:

- `standard`
- `version`
- `num_qubits`
- `operations`
- `metadata`
- `extensions`

**Step 3: Rewrite the operation model**

Add a normative section that defines:

- core `type: "gate"`
- standard extension operations: `repeat`, `tick`, `qubit_coords`, `shift_coords`, `detector`, `observable_include`, `noise`, `annotation`
- optional `raw_targets` item schema for Stim-like targets

Use explicit examples such as:

```json
{ "type": "tick" }
{ "type": "repeat", "count": 100, "body": [] }
{ "type": "detector", "coords": [1, 0], "sources": [{ "kind": "rec", "offset": -1 }] }
```

**Step 4: Rewrite validation rules and examples**

Add validation rules for:

- `repeat.count >= 1`
- `body` is required on `repeat`
- `tick` takes no qubit indices
- detector and observable source items are explicit
- `targets` and `controls` remain bounded by `num_qubits`
- `raw_targets` preserve data that cannot be normalized into plain qubit indices

Include one ordinary gate-only example and one Stim-style QEC example.

**Step 5: Review the draft manually**

Read the whole file and confirm there is no remaining normative dependency on a top-level `gates` array.

**Step 6: Commit**

```bash
git -C /Users/nzy/mcode/QProtocal add /Users/nzy/mcode/QProtocal/QSTD101-ZY.md
git -C /Users/nzy/mcode/QProtocal commit -m "docs: rewrite qstd101 draft around operations"
```

### Task 2: Add QSTD101 Document Types In `rstim`

**Files:**
- Create: `/Users/nzy/rcode/rstim/rstim/src/qstd101.rs`
- Modify: `/Users/nzy/rcode/rstim/rstim/src/lib.rs`
- Test: `/Users/nzy/rcode/rstim/rstim/tests/qstd101_export.rs`

**Step 1: Write the failing test**

```rust
use rstim::qstd101::{Qstd101Document, Qstd101Operation};
use serde_json::json;

#[test]
fn serializes_minimal_gate_document() {
    let doc = Qstd101Document {
        standard: "QSTD101-ZY".to_string(),
        version: "1.0".to_string(),
        num_qubits: 2,
        operations: vec![Qstd101Operation::Gate {
            gate: "H".to_string(),
            targets: vec![0],
            controls: Vec::new(),
            control_configs: None,
            params: Vec::new(),
            raw_targets: None,
            display: None,
            tags: Vec::new(),
        }],
        metadata: None,
        extensions: None,
    };
    let value = serde_json::to_value(&doc).unwrap();
    assert_eq!(value["standard"], json!("QSTD101-ZY"));
    assert_eq!(value["operations"][0]["type"], json!("gate"));
    assert_eq!(value["operations"][0]["gate"], json!("H"));
}
```

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p rstim --test qstd101_export serializes_minimal_gate_document
```

Expected: FAIL with unresolved import errors for `rstim::qstd101`.

**Step 3: Write minimal implementation**

Create `qstd101.rs` with serializable types:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Qstd101Document {
    pub standard: String,
    pub version: String,
    pub num_qubits: usize,
    pub operations: Vec<Qstd101Operation>,
    pub metadata: Option<serde_json::Value>,
    pub extensions: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum Qstd101Operation {
    #[serde(rename = "gate")]
    Gate { ... },
    #[serde(rename = "tick")]
    Tick,
    #[serde(rename = "repeat")]
    Repeat { count: u64, body: Vec<Qstd101Operation> },
    #[serde(rename = "qubit_coords")]
    QubitCoords { coords: Vec<f64>, targets: Vec<u32> },
    #[serde(rename = "shift_coords")]
    ShiftCoords { delta: Vec<f64> },
    #[serde(rename = "detector")]
    Detector { coords: Vec<f64>, sources: Vec<Qstd101TargetRef> },
    #[serde(rename = "observable_include")]
    ObservableInclude { index: u32, sources: Vec<Qstd101TargetRef> },
    #[serde(rename = "noise")]
    Noise { gate: String, params: Vec<f64>, raw_targets: Vec<Qstd101TargetRef> },
    #[serde(rename = "annotation")]
    Annotation { kind: String, text: String },
}
```

Also add:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind")]
pub enum Qstd101TargetRef {
    #[serde(rename = "qubit")]
    Qubit { index: u32, inverted: Option<bool> },
    #[serde(rename = "rec")]
    Rec { offset: i32 },
    #[serde(rename = "pauli")]
    Pauli { basis: String, qubit: u32, inverted: Option<bool> },
    #[serde(rename = "combiner")]
    Combiner,
    #[serde(rename = "sweep")]
    Sweep { index: u32 },
}
```

Expose the module from `lib.rs`.

**Step 4: Run test to verify it passes**

Run:

```bash
cargo test -p rstim --test qstd101_export serializes_minimal_gate_document
```

Expected: PASS.

**Step 5: Commit**

```bash
git add /Users/nzy/rcode/rstim/rstim/src/qstd101.rs /Users/nzy/rcode/rstim/rstim/src/lib.rs /Users/nzy/rcode/rstim/rstim/tests/qstd101_export.rs
git commit -m "feat: add qstd101 document types"
```

### Task 3: Export `StimInstr` Trees To QSTD101

**Files:**
- Modify: `/Users/nzy/rcode/rstim/rstim/src/qstd101.rs`
- Test: `/Users/nzy/rcode/rstim/rstim/tests/qstd101_export.rs`

**Step 1: Write the failing tests**

Add tests for the three critical semantics:

```rust
use rstim::parser::parse_lines;
use rstim::qstd101::{export_qstd101, Qstd101Operation};

#[test]
fn export_preserves_repeat_and_tick() {
    let instrs = parse_lines("H 0\nTICK\nREPEAT 2 {\n  M 0\n}\n").unwrap();
    let doc = export_qstd101(&instrs).unwrap();
    assert!(matches!(doc.operations[1], Qstd101Operation::Tick));
    assert!(matches!(doc.operations[2], Qstd101Operation::Repeat { count: 2, .. }));
}

#[test]
fn export_preserves_detector_and_coords() {
    let instrs = parse_lines("QUBIT_COORDS(1,2) 0\nM 0\nDETECTOR(5,6) rec[-1]\n").unwrap();
    let doc = export_qstd101(&instrs).unwrap();
    assert!(matches!(doc.operations[0], Qstd101Operation::QubitCoords { .. }));
    assert!(matches!(doc.operations[2], Qstd101Operation::Detector { .. }));
}

#[test]
fn export_uses_raw_targets_for_feedback() {
    let instrs = parse_lines("M 0\nCX rec[-1] 1\n").unwrap();
    let doc = export_qstd101(&instrs).unwrap();
    match &doc.operations[1] {
        Qstd101Operation::Gate { raw_targets: Some(raw_targets), .. } => {
            assert_eq!(raw_targets.len(), 2);
        }
        other => panic!("unexpected op: {other:?}"),
    }
}
```

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p rstim --test qstd101_export
```

Expected: FAIL because `export_qstd101` does not exist.

**Step 3: Write minimal implementation**

Add:

```rust
pub fn export_qstd101(instrs: &[StimInstr]) -> Result<Qstd101Document, String> {
    Ok(Qstd101Document {
        standard: "QSTD101-ZY".to_string(),
        version: "1.0".to_string(),
        num_qubits: crate::stats::num_qubits(instrs),
        operations: export_operations(instrs)?,
        metadata: Some(json!({ "framework": "rstim" })),
        extensions: None,
    })
}
```

Implement `export_operations` recursively:

- `StimInstr::Repeat` -> `Qstd101Operation::Repeat`
- `TICK` -> `Qstd101Operation::Tick`
- `QUBIT_COORDS` -> `Qstd101Operation::QubitCoords`
- `SHIFT_COORDS` -> `Qstd101Operation::ShiftCoords`
- `DETECTOR` -> `Qstd101Operation::Detector`
- `OBSERVABLE_INCLUDE` -> `Qstd101Operation::ObservableInclude`
- known noise ops -> `Qstd101Operation::Noise`
- all other ops -> `Qstd101Operation::Gate`

For any operation whose targets are not plain qubit targets, populate `raw_targets`.

**Step 4: Run test to verify it passes**

Run:

```bash
cargo test -p rstim --test qstd101_export
```

Expected: PASS.

**Step 5: Commit**

```bash
git add /Users/nzy/rcode/rstim/rstim/src/qstd101.rs /Users/nzy/rcode/rstim/rstim/tests/qstd101_export.rs
git commit -m "feat: export stim circuits as qstd101"
```

### Task 4: Add A CLI JSON Export Command

**Files:**
- Modify: `/Users/nzy/rcode/rstim/rstim/src/cli.rs`
- Test: `/Users/nzy/rcode/rstim/rstim/tests/cli_export_json.rs`

**Step 1: Write the failing test**

```rust
use std::process::Command;

#[test]
fn export_json_writes_qstd101_document() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let input = tempfile::NamedTempFile::new().unwrap();
    let circuit = "QUBIT_COORDS(0,0) 0\nH 0\nTICK\nM 0\nDETECTOR rec[-1]\n";
    std::fs::write(input.path(), circuit).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_rstim"))
        .arg("export_json")
        .arg("--in")
        .arg(input.path())
        .arg("--out")
        .arg(tmp.path())
        .output()
        .unwrap();
    assert!(output.status.success(), "{:?}", output);
    let text = std::fs::read_to_string(tmp.path()).unwrap();
    assert!(text.contains("\"standard\": \"QSTD101-ZY\""));
    assert!(text.contains("\"type\": \"tick\""));
    assert!(text.contains("\"type\": \"detector\""));
}
```

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p rstim --test cli_export_json export_json_writes_qstd101_document
```

Expected: FAIL because the subcommand does not exist.

**Step 3: Write minimal implementation**

Add a new clap subcommand:

```rust
#[command(name = "export_json")]
ExportJson {
    #[arg(long = "in")]
    r#in: Option<String>,
    #[arg(long)]
    out: Option<String>,
    #[arg(long, default_value = "pretty")]
    format: String,
}
```

Dispatch it to:

```rust
pub fn run_export_json(text: &str, format: &str, w: &mut dyn Write) -> Result<(), String> {
    let instrs = parse_lines(text)?;
    let doc = crate::qstd101::export_qstd101(&instrs)?;
    match format {
        "pretty" => serde_json::to_writer_pretty(w, &doc).map_err(|e| e.to_string())?,
        "compact" => serde_json::to_writer(w, &doc).map_err(|e| e.to_string())?,
        other => return Err(format!("unknown json format: {other}")),
    }
    w.write_all(b"\n").map_err(|e| e.to_string())?;
    Ok(())
}
```

**Step 4: Run test to verify it passes**

Run:

```bash
cargo test -p rstim --test cli_export_json export_json_writes_qstd101_document
```

Expected: PASS.

**Step 5: Commit**

```bash
git add /Users/nzy/rcode/rstim/rstim/src/cli.rs /Users/nzy/rcode/rstim/rstim/tests/cli_export_json.rs
git commit -m "feat: add qstd101 json export command"
```

### Task 5: Add Real Export Fixtures From Generator Circuits

**Files:**
- Create: `/Users/nzy/rcode/rstim/rstim/tests/qstd101_fixtures.rs`
- Create: `/Users/nzy/rcode/rstim/rstim/tests/fixtures/qstd101/`
- Test: `/Users/nzy/rcode/rstim/rstim/tests/qstd101_fixtures.rs`

**Step 1: Write the failing test**

```rust
#[test]
fn repetition_code_fixture_contains_repeat_and_detector() {
    let text = std::fs::read_to_string("tests/fixtures/qstd101/repetition_code_memory_d3_r3.json").unwrap();
    assert!(text.contains("\"type\": \"repeat\""));
    assert!(text.contains("\"type\": \"detector\""));
}
```

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p rstim --test qstd101_fixtures repetition_code_fixture_contains_repeat_and_detector
```

Expected: FAIL because the fixture file does not exist.

**Step 3: Write minimal implementation**

Create a fixture generation test or helper that:

- generates a small repetition code circuit
- exports it to QSTD101
- writes the JSON fixture into `tests/fixtures/qstd101/`

Seed at least:

- `repetition_code_memory_d3_r3.json`
- `surface_code_rotated_memory_x_d3_r3.json`

**Step 4: Run test to verify it passes**

Run:

```bash
cargo test -p rstim --test qstd101_fixtures
```

Expected: PASS.

**Step 5: Commit**

```bash
git add /Users/nzy/rcode/rstim/rstim/tests/qstd101_fixtures.rs /Users/nzy/rcode/rstim/rstim/tests/fixtures/qstd101
git commit -m "test: add qstd101 export fixtures"
```

### Task 6: Create The Typst Package Skeleton

**Files:**
- Create: `/Users/nzy/tycode/qstd101-viz/typst.toml`
- Create: `/Users/nzy/tycode/qstd101-viz/lib.typ`
- Create: `/Users/nzy/tycode/qstd101-viz/README.md`
- Create: `/Users/nzy/tycode/qstd101-viz/examples/minimal-timeline.typ`
- Create: `/Users/nzy/tycode/qstd101-viz/examples/qec-timeline.typ`
- Test: `/Users/nzy/tycode/qstd101-viz/examples/minimal-timeline.typ`

**Step 1: Write the failing compile command**

Run:

```bash
typst compile /Users/nzy/tycode/qstd101-viz/examples/minimal-timeline.typ /tmp/minimal-timeline.svg --format svg
```

Expected: FAIL because the package does not exist.

**Step 2: Write minimal package files**

Initialize the package repository first:

```bash
mkdir -p /Users/nzy/tycode/qstd101-viz
git -C /Users/nzy/tycode/qstd101-viz init
```

`typst.toml`:

```toml
[package]
name = "qstd101-viz"
version = "0.1.0"
compiler = "0.13.0"
entrypoint = "lib.typ"
authors = ["Zhongyi Ni"]
license = "MIT"
description = "Render QSTD101-ZY quantum circuit JSON documents."
exclude = ["examples/"]
```

`lib.typ`:

```typ
#let render-qstd101(path, mode: "timeline") = {
  let doc = json(path)
  if mode == "timeline" {
    render-qstd101-timeline(doc)
  } else {
    panic("unsupported render mode")
  }
}

#let render-qstd101-timeline(doc) = {
  [timeline renderer not implemented yet]
}
```

`examples/minimal-timeline.typ`:

```typ
#import "../lib.typ": render-qstd101
#set page(width: auto, height: auto, margin: 8pt)
#render-qstd101("fixtures/minimal.json")
```

**Step 3: Add a tiny JSON fixture**

Create `/Users/nzy/tycode/qstd101-viz/examples/fixtures/minimal.json` with one `H`, one `tick`, and one `M`.

**Step 4: Run compile to verify the skeleton works**

Run:

```bash
typst compile /Users/nzy/tycode/qstd101-viz/examples/minimal-timeline.typ /tmp/minimal-timeline.svg --format svg
```

Expected: PASS and produce an SVG with placeholder content.

**Step 5: Commit**

```bash
git -C /Users/nzy/tycode/qstd101-viz add .
git -C /Users/nzy/tycode/qstd101-viz commit -m "feat: scaffold qstd101 typst package"
```

### Task 7: Add A Typst Render Normalization Layer

**Files:**
- Modify: `/Users/nzy/tycode/qstd101-viz/lib.typ`
- Test: `/Users/nzy/tycode/qstd101-viz/examples/qec-timeline.typ`

**Step 1: Write the failing visual scenario**

Create a QEC example fixture with:

- `qubit_coords`
- `tick`
- `repeat`
- `detector`
- `observable_include`

and compile it:

```bash
typst compile /Users/nzy/tycode/qstd101-viz/examples/qec-timeline.typ /tmp/qec-timeline.svg --format svg
```

Expected: current output is placeholder-only and does not separate event kinds.

**Step 2: Write minimal normalization helpers**

In `lib.typ`, add helpers that derive:

- qubit lane count from `num_qubits`
- moment boundaries from explicit `tick`
- flattened draw events while preserving repeat labels
- annotation buckets for coordinates, detectors, observables, and noise

Sketch:

```typ
#let normalize-ops(ops, repeat-prefix: ()) = {
  let events = ()
  let moment = 0
  for op in ops {
    if op.type == "tick" {
      moment += 1
    } else if op.type == "repeat" {
      events += normalize-repeat(op, moment, repeat-prefix)
    } else {
      events += ((moment: moment, op: op, repeat-prefix: repeat-prefix),)
    }
  }
  events
}
```

**Step 3: Render the normalized data as debug scaffolding**

Draw:

- qubit wires
- vertical tick separators
- text labels for annotation events above and below the wires

Do not style final gate glyphs yet; just make the structure visible.

**Step 4: Re-run compile to verify the structure**

Run:

```bash
typst compile /Users/nzy/tycode/qstd101-viz/examples/qec-timeline.typ /tmp/qec-timeline.svg --format svg
```

Expected: PASS with separate tracks and visible repeat grouping labels.

**Step 5: Commit**

```bash
git -C /Users/nzy/tycode/qstd101-viz add /Users/nzy/tycode/qstd101-viz/lib.typ /Users/nzy/tycode/qstd101-viz/examples
git -C /Users/nzy/tycode/qstd101-viz commit -m "feat: normalize qstd101 events for timeline rendering"
```

### Task 8: Render Main-Track Quantum Operations

**Files:**
- Modify: `/Users/nzy/tycode/qstd101-viz/lib.typ`
- Test: `/Users/nzy/tycode/qstd101-viz/examples/minimal-timeline.typ`

**Step 1: Write the failing visual expectation**

Use the minimal example and confirm that the current timeline still draws only debug text rather than gate shapes.

**Step 2: Implement gate and connector primitives**

Add Typst helpers for:

- single-qubit gate boxes
- measurement boxes
- reset symbols
- control dots
- target marks
- vertical connectors for two-qubit interactions

Use operation dispatch like:

```typ
#let draw-main-op(op, x, lane-y) = {
  if op.type == "gate" and op.gate == "H" {
    draw-gate-box(x, lane-y.at(op.targets.at(0)), "H")
  } else if op.type == "gate" and op.gate == "CX" {
    draw-controlled-x(x, lane-y.at(op.controls.at(0)), lane-y.at(op.targets.at(0)))
  } else if op.type == "gate" {
    draw-gate-box(x, lane-y.at(op.targets.at(0)), op.display.at("label", default: op.gate))
  }
}
```

**Step 3: Re-run compile**

Run:

```bash
typst compile /Users/nzy/tycode/qstd101-viz/examples/minimal-timeline.typ /tmp/minimal-timeline.svg --format svg
```

Expected: PASS with real gate shapes on the qubit wires.

**Step 4: Add one multi-qubit example**

Extend the fixture with a controlled gate and verify the connector layout.

**Step 5: Commit**

```bash
git -C /Users/nzy/tycode/qstd101-viz add /Users/nzy/tycode/qstd101-viz/lib.typ /Users/nzy/tycode/qstd101-viz/examples
git -C /Users/nzy/tycode/qstd101-viz commit -m "feat: draw qstd101 main-track operations"
```

### Task 9: Render Annotation Tracks And Noise

**Files:**
- Modify: `/Users/nzy/tycode/qstd101-viz/lib.typ`
- Test: `/Users/nzy/tycode/qstd101-viz/examples/qec-timeline.typ`

**Step 1: Write the failing visual expectation**

Confirm that detector, observable, coordinate, shift, and noise items are still either missing or unstyled.

**Step 2: Implement annotation-track rendering**

Add compact glyphs or labels:

- top track: `qubit_coords`, `shift_coords`
- bottom track: `detector`, `observable_include`
- overlays or side badges: `noise`

Use stable colors or shapes by operation kind. Keep them smaller than gate boxes.

**Step 3: Add lightweight source references**

For detectors and observables, draw either:

- numbered labels only, or
- subtle connector lines to the nearest referenced measurement moment

Do not attempt full dependency graphs in this first pass.

**Step 4: Re-run compile**

Run:

```bash
typst compile /Users/nzy/tycode/qstd101-viz/examples/qec-timeline.typ /tmp/qec-timeline.svg --format svg
```

Expected: PASS with visible annotation tracks and no overlap severe enough to hide the main circuit.

**Step 5: Commit**

```bash
git -C /Users/nzy/tycode/qstd101-viz add /Users/nzy/tycode/qstd101-viz/lib.typ /Users/nzy/tycode/qstd101-viz/examples
git -C /Users/nzy/tycode/qstd101-viz commit -m "feat: draw qstd101 annotation tracks"
```

### Task 10: Add Readme And Smoke Verification

**Files:**
- Modify: `/Users/nzy/tycode/qstd101-viz/README.md`
- Create: `/Users/nzy/tycode/qstd101-viz/Makefile`
- Test: `/Users/nzy/tycode/qstd101-viz/examples/minimal-timeline.typ`
- Test: `/Users/nzy/tycode/qstd101-viz/examples/qec-timeline.typ`

**Step 1: Write the failing usage check**

Try to follow the README from a clean shell:

```bash
cd /Users/nzy/tycode/qstd101-viz
make examples
```

Expected: FAIL because the package has no documented smoke path yet.

**Step 2: Add minimal README usage**

Document:

- package purpose
- expected JSON shape
- example import
- timeline renderer entry point
- note that coordinate-layout rendering is planned but not yet implemented

**Step 3: Add a small Makefile**

```makefile
examples:
	typst compile examples/minimal-timeline.typ /tmp/qstd101-minimal.svg --format svg
	typst compile examples/qec-timeline.typ /tmp/qstd101-qec.svg --format svg
```

**Step 4: Run the smoke command**

Run:

```bash
cd /Users/nzy/tycode/qstd101-viz
make examples
```

Expected: PASS and both SVGs are generated.

**Step 5: Commit**

```bash
git -C /Users/nzy/tycode/qstd101-viz add /Users/nzy/tycode/qstd101-viz/README.md /Users/nzy/tycode/qstd101-viz/Makefile
git -C /Users/nzy/tycode/qstd101-viz commit -m "docs: add qstd101 typst package usage"
```

## Final Verification

Run:

```bash
cargo test
```

Expected: PASS.

Run:

```bash
cargo test -p rstim --test qstd101_export --test cli_export_json --test qstd101_fixtures
```

Expected: PASS.

Run:

```bash
typst compile /Users/nzy/tycode/qstd101-viz/examples/minimal-timeline.typ /tmp/minimal-timeline.svg --format svg
typst compile /Users/nzy/tycode/qstd101-viz/examples/qec-timeline.typ /tmp/qec-timeline.svg --format svg
```

Expected: PASS.

## Notes For The Follow-Up Phase

- A later task should add a coordinate-layout renderer that consumes the same JSON document.
- Do not change the exported JSON shape to make the first timeline renderer easier; the point of this plan is to keep layout rendering compatible from the start.
- If Typst compile performance becomes a concern, add a normalization cache inside the package before changing the protocol.

Plan complete and saved to `docs/plans/2026-03-21-rstim-qstd101-visualization-plan.md`. Two execution options:

**1. Subagent-Driven (this session)** - I dispatch fresh subagent per task, review between tasks, fast iteration

**2. Parallel Session (separate)** - Open new session with executing-plans, batch execution with checkpoints

Which approach?
