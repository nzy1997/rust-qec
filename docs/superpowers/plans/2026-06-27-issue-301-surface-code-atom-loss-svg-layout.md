# Issue 301 Surface-Code Atom-Loss and SVG Layout Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an opt-in surface-code after-Clifford atom-loss generation channel, regenerate the atom-loss showcase from that path, and fix built-in SVG layout/labels for wide atom-loss timelines.

**Architecture:** Extend `NoiseParams` with an explicit loss field that defaults to zero and is not set by `uniform()`. Surface-code round emitters add loss immediately after Clifford layers when that field is positive. The SVG renderer packs render items per tick-delimited layer and emits per-item decimal probability labels for known noise boxes.

**Tech Stack:** Rust 2024, Clap CLI, existing `rstim` integration tests, existing QP101 JSON/SVG renderer, Typst smoke compile.

## Global Constraints

- Default `rstim gen` behavior must be unchanged when no atom-loss flag is passed.
- `--after_clifford_depolarization` must continue to emit `DEPOLARIZE1`/`DEPOLARIZE2` and must not imply `LOSS`.
- `--after_clifford_loss_probability 0.01` must emit `LOSS(0.01)` after every surface-code Clifford layer.
- H-layer loss targets are the qubits touched by that H layer.
- CX-layer loss targets are all qubits participating in that CX layer.
- Probability labels in known noise SVG rendering are bare decimals such as `0.01`, not `p=0.01` or `p=1`.
- Each visible `D1`/`LOSS` operation in the SVG must have its own decimal probability label.
- No QP101 JSON schema or semantic field shape changes are planned; do not update `rstim/doc/QP101-ZY.md` unless implementation discovers an actual schema change.
- Do not add new atom-loss default behavior to existing generators.
- Preserve existing public generator function signatures such as `rotated_memory_x(distance, rounds, noise)`.

---

### Task 1: Surface-Code Atom-Loss Generator And CLI

**Files:**
- Modify: `rstim/src/codegen/noise_params.rs`
- Modify: `rstim/src/codegen/surface_code.rs`
- Modify: `rstim/src/cli.rs`
- Modify: `rstim/tests/gen_surface_code.rs`
- Modify: `rstim/tests/cli_gen.rs`
- Modify: `rstim/tests/codegen_noise.rs`
- Modify: `rstim/tests/stim_codegen.rs`

**Interfaces:**
- Consumes: existing `NoiseParams`, `rotated_memory_x_with_params`, CLI `Commands::Gen`.
- Produces: `NoiseParams::after_clifford_loss_probability: f64`, `rstim gen --after_clifford_loss_probability`, and loss emission after surface-code Clifford layers.

- [ ] **Step 1: Add failing generator regression**

Append this test to `rstim/tests/gen_surface_code.rs`:

```rust
#[test]
fn surface_code_after_clifford_atom_loss() {
    use rstim::codegen::NoiseParams;
    use rstim::codegen::surface_code::rotated_memory_x_with_params;
    use rstim::ir::StimInstr;

    let instrs = rotated_memory_x_with_params(3, 3, NoiseParams {
        after_clifford_loss_probability: 0.01,
        ..NoiseParams::none()
    });

    let mut h_layers = 0usize;
    let mut cx_layers = 0usize;
    for pair in instrs.windows(2) {
        let StimInstr::Op {
            name,
            targets,
            ..
        } = &pair[0] else {
            continue;
        };
        if name != "H" && name != "CX" {
            continue;
        }
        let StimInstr::Op {
            name: loss_name,
            args: loss_args,
            targets: loss_targets,
            ..
        } = &pair[1] else {
            panic!("Clifford {name} was not followed by an op: {pair:?}");
        };
        assert_eq!(loss_name, "LOSS", "Clifford {name} should be followed by LOSS");
        assert_eq!(loss_args, &vec![0.01], "LOSS should keep the configured probability");
        assert_eq!(
            loss_targets, targets,
            "LOSS after {name} should target exactly the Clifford layer targets"
        );
        if name == "H" {
            h_layers += 1;
        } else {
            cx_layers += 1;
        }
    }

    assert_eq!(h_layers, 6, "three rounds should each have two H layers");
    assert_eq!(cx_layers, 12, "three rounds should each have four CX layers");
}
```

- [ ] **Step 2: Run generator regression and verify RED**

Run:

```sh
cargo test -p rstim --test gen_surface_code surface_code_after_clifford_atom_loss -q
```

Expected: FAIL at compile time because `NoiseParams` has no
`after_clifford_loss_probability` field yet.

- [ ] **Step 3: Add failing CLI regression**

Append this test to `rstim/tests/cli_gen.rs`:

```rust
#[test]
fn gen_surface_code_atom_loss_is_opt_in_from_cli() {
    use std::fs;
    use std::process::Command;

    let atom_loss_path = std::env::temp_dir().join(format!(
        "rstim-surface-atom-loss-{}.stim",
        std::process::id()
    ));
    let depol_only_path = std::env::temp_dir().join(format!(
        "rstim-surface-depol-only-{}.stim",
        std::process::id()
    ));

    let atom_loss = Command::new(env!("CARGO_BIN_EXE_rstim"))
        .args([
            "gen",
            "--code",
            "surface_code",
            "--task",
            "rotated_memory_x",
            "--distance",
            "3",
            "--rounds",
            "3",
            "--after_clifford_loss_probability",
            "0.01",
            "--out",
            atom_loss_path.to_str().expect("utf8 temp path"),
        ])
        .output()
        .expect("rstim gen atom-loss command should run");
    assert!(
        atom_loss.status.success(),
        "atom-loss command failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&atom_loss.stdout),
        String::from_utf8_lossy(&atom_loss.stderr)
    );
    let atom_loss_text = fs::read_to_string(&atom_loss_path).expect("atom-loss output exists");
    assert!(
        atom_loss_text.contains("LOSS(0.01)"),
        "atom-loss output should contain LOSS(0.01):\n{atom_loss_text}"
    );
    assert!(
        atom_loss_text.contains("H") && atom_loss_text.contains("CX"),
        "positive control should include Clifford layers:\n{atom_loss_text}"
    );

    let depol_only = Command::new(env!("CARGO_BIN_EXE_rstim"))
        .args([
            "gen",
            "--code",
            "surface_code",
            "--task",
            "rotated_memory_x",
            "--distance",
            "3",
            "--rounds",
            "3",
            "--after_clifford_depolarization",
            "0.01",
            "--out",
            depol_only_path.to_str().expect("utf8 temp path"),
        ])
        .output()
        .expect("rstim gen depolarization command should run");
    assert!(
        depol_only.status.success(),
        "depolarization command failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&depol_only.stdout),
        String::from_utf8_lossy(&depol_only.stderr)
    );
    let depol_text = fs::read_to_string(&depol_only_path).expect("depol output exists");
    assert!(depol_text.contains("DEPOLARIZE1(0.01)"));
    assert!(depol_text.contains("DEPOLARIZE2(0.01)"));
    assert!(
        !depol_text.contains("LOSS(0.01)"),
        "depolarization-only generation must not emit loss:\n{depol_text}"
    );

    let _ = fs::remove_file(atom_loss_path);
    let _ = fs::remove_file(depol_only_path);
}
```

- [ ] **Step 4: Run CLI regression and verify RED**

Run:

```sh
cargo test -p rstim --test cli_gen gen_surface_code_atom_loss_is_opt_in_from_cli -q
```

Expected: FAIL because Clap rejects `--after_clifford_loss_probability`.

- [ ] **Step 5: Implement the generator/API change**

In `rstim/src/codegen/noise_params.rs`, add the field and keep `uniform()`
lossless:

```rust
pub struct NoiseParams {
    pub before_round_data_depolarization: f64,
    pub after_clifford_depolarization: f64,
    pub before_measure_flip_probability: f64,
    pub after_reset_flip_probability: f64,
    pub after_clifford_loss_probability: f64,
}

pub fn uniform(noise: f64) -> Self {
    NoiseParams {
        before_round_data_depolarization: noise,
        after_clifford_depolarization: noise,
        before_measure_flip_probability: noise,
        after_reset_flip_probability: noise,
        after_clifford_loss_probability: 0.0,
    }
}
```

Update all struct literals in tests and production to include
`after_clifford_loss_probability: 0.0` where `..NoiseParams::none()` is not
used.

In `rstim/src/codegen/surface_code.rs`, add a helper near `op`:

```rust
fn emit_after_clifford_loss(instrs: &mut Vec<StimInstr>, probability: f64, qubits: &[u32]) {
    if probability <= 0.0 || qubits.is_empty() {
        return;
    }
    let targets: Vec<StimTarget> = qubits.iter().copied().map(StimTarget::Qubit).collect();
    instrs.push(op("LOSS", &[probability], &targets));
}
```

Call it immediately after each H layer and after each non-empty CX layer in both
rotated and unrotated surface-code round emitters:

```rust
emit_after_clifford_loss(instrs, params.after_clifford_loss_probability, &x_measure_qubits);
emit_after_clifford_loss(instrs, params.after_clifford_loss_probability, &cnot_layers[k]);
```

- [ ] **Step 6: Implement the CLI plumbing**

In `Commands::Gen`, add:

```rust
#[arg(long = "after_clifford_loss_probability", default_value = "0")]
after_clifford_loss_probability: f64,
```

Thread this value through `main_inner`, `run_gen`, and
`generate_common_circuit_text` by replacing the common-code `noise: f64` input
with:

```rust
NoiseParams {
    after_clifford_depolarization: noise,
    ..NoiseParams::none()
}
```

and then setting:

```rust
params.after_clifford_loss_probability = after_clifford_loss_probability;
```

Keep CSS generation on `NoiseParams::uniform(noise)` because the new CLI flag is
only part of the common surface-code path in this task.

- [ ] **Step 7: Verify GREEN for Task 1**

Run:

```sh
cargo test -p rstim --test gen_surface_code surface_code_after_clifford_atom_loss -q
cargo test -p rstim --test cli_gen gen_surface_code_atom_loss_is_opt_in_from_cli -q
cargo test -p rstim --test codegen_noise --test stim_codegen -q
```

Expected: PASS.

- [ ] **Step 8: Commit Task 1**

Run:

```sh
git add rstim/src/codegen/noise_params.rs rstim/src/codegen/surface_code.rs rstim/src/cli.rs rstim/tests/gen_surface_code.rs rstim/tests/cli_gen.rs rstim/tests/codegen_noise.rs rstim/tests/stim_codegen.rs
git commit -m "feat: add surface-code after-clifford atom loss"
```

### Task 2: Showcase Contract And Fixture Regeneration

**Files:**
- Modify: `rstim/src/showcase.rs`
- Modify: `rstim/tests/qp101_fixtures.rs`
- Modify: `rstim/tests/fixtures/qp101/surface_code_rotated_memory_x_d3_r3_mixed_noise.json`
- Modify: `rstim/tests/fixtures/qp101/surface_code_rotated_memory_x_d3_r3_mixed_noise_sample_seed7.json`
- Modify: `qp101-viz/examples/surface-code-rotated-memory-x-d3-r3-atom-loss.stim`
- Modify: `qp101-viz/examples/surface-code-rotated-memory-x-d3-r3-atom-loss.qp101.json`
- Modify: `qp101-viz/examples/surface-code-rotated-memory-x-d3-r3-atom-loss-sample.qp101.json`

**Interfaces:**
- Consumes: Task 1 `NoiseParams::after_clifford_loss_probability`.
- Produces: showcase artifacts generated from first-class surface-code atom-loss generation.

- [ ] **Step 1: Write failing showcase contract test**

Rename `mixed_noise_showcase_circuit_contains_sparse_loss_and_common_pauli_noise`
in `rstim/tests/qp101_fixtures.rs` to
`mixed_noise_showcase_circuit_uses_generated_after_clifford_loss_and_common_pauli_noise`.
Replace the sparse-loss assertions with:

```rust
let loss_target_count = count_target_tokens_for_op(&circuit_text, "LOSS(0.01)");
assert!(
    loss_target_count >= MIXED_NOISE_ROUNDS * 6,
    "showcase should get dense after-Clifford atom loss from generation, got {loss_target_count} loss targets"
);
assert!(
    circuit_text.contains("H 1\nLOSS(0.01) 1")
        || circuit_text.contains("H 3\nLOSS(0.01) 3"),
    "showcase should place LOSS immediately after H layers:\n{circuit_text}"
);
assert!(
    circuit_text.contains("CX") && circuit_text.contains("\nLOSS(0.01)"),
    "showcase should place LOSS after CX layers:\n{circuit_text}"
);
```

Keep the existing sparse Pauli assertions for `X_ERROR`, `Z_ERROR`,
`DEPOLARIZE1`, and `DEPOLARIZE2`.

- [ ] **Step 2: Run showcase test and verify RED**

Run:

```sh
cargo test -p rstim --test qp101_fixtures mixed_noise_showcase_circuit_uses_generated_after_clifford_loss_and_common_pauli_noise -q
```

Expected: FAIL because the helper still inserts only sparse final-tail loss.

- [ ] **Step 3: Update showcase helper**

In `rstim/src/showcase.rs`, change the base generation to:

```rust
let base = crate::codegen::surface_code::rotated_memory_x_with_params(
    3,
    3,
    crate::codegen::NoiseParams {
        after_clifford_loss_probability: 0.01,
        ..crate::codegen::NoiseParams::none()
    },
);
```

Remove the manual `out.push(noise_op("LOSS", ...))` insertion while keeping the
sparse Pauli/depolarizing decorations after the final pre-measurement `TICK`.

- [ ] **Step 4: Run showcase test and verify GREEN**

Run:

```sh
cargo test -p rstim --test qp101_fixtures mixed_noise_showcase_circuit_uses_generated_after_clifford_loss_and_common_pauli_noise -q
```

Expected: PASS.

- [ ] **Step 5: Regenerate showcase artifacts**

Run:

```sh
cargo run -p rstim --example mixed_noise_showcase
```

Expected: the committed `.stim`, QP101 examples, and Rust fixtures are updated.
There is no checked-in SVG preview in this checkout.

- [ ] **Step 6: Verify regenerated fixtures**

Run:

```sh
cargo test -p rstim --test qp101_fixtures -q
typst compile --root qp101-viz qp101-viz/examples/surface-code-rotated-memory-x-d3-r3-atom-loss.typ /tmp/surface-code-rotated-memory-x-d3-r3-atom-loss.pdf
```

Expected: PASS and Typst exits successfully.

- [ ] **Step 7: Commit Task 2**

Run:

```sh
git add rstim/src/showcase.rs rstim/tests/qp101_fixtures.rs rstim/tests/fixtures/qp101/surface_code_rotated_memory_x_d3_r3_mixed_noise.json rstim/tests/fixtures/qp101/surface_code_rotated_memory_x_d3_r3_mixed_noise_sample_seed7.json qp101-viz/examples/surface-code-rotated-memory-x-d3-r3-atom-loss.stim qp101-viz/examples/surface-code-rotated-memory-x-d3-r3-atom-loss.qp101.json qp101-viz/examples/surface-code-rotated-memory-x-d3-r3-atom-loss-sample.qp101.json
git commit -m "feat: regenerate atom-loss showcase from generator"
```

### Task 3: SVG Layer Packing And Decimal Probability Labels

**Files:**
- Modify: `rstim/src/qp101_svg.rs`
- Modify: `rstim/tests/qp101_svg.rs`

**Interfaces:**
- Consumes: `render_svg(&Qp101Document) -> Result<String, String>`.
- Produces: packed same-layer single-qubit gates, non-overlapping two-qubit/noise boxes, and per-item decimal labels for known noise operations.

- [ ] **Step 1: Add failing SVG regression**

Add `surface_code_atom_loss_svg_layout_regression` to `rstim/tests/qp101_svg.rs`.
Build the document from:

```rust
let instrs = parse_lines(
    "H 0\n\
     H 1\n\
     H 2\n\
     LOSS(0.01) 0 1 2\n\
     DEPOLARIZE1(0.01) 0 1 2\n\
     TICK\n\
     CX 0 1 2 3\n\
     LOSS(0.01) 0 1 2 3\n\
     TICK\n\
     M 0 1 2 3\n",
)
.expect("layout fixture should parse");
let doc = export_qp101(&instrs).expect("layout fixture should export");
let svg = render_svg(&doc).expect("layout fixture should render");
```

Assert these behaviors using helper functions in the same test file:

```rust
assert!(!svg.contains("p=0.01"), "known noise labels must be decimal-only: {svg}");
assert_eq!(svg.matches(">0.01</text>").count(), 10, "each LOSS/D1 box should have its own decimal label: {svg}");

let h_positions = text_positions(&svg, "H");
assert_eq!(h_positions.len(), 3);
assert_eq!(h_positions[0].0, h_positions[1].0, "same-layer H gates should share x");
assert_eq!(h_positions[1].0, h_positions[2].0, "same-layer H gates should share x");

let rects = element_rects(&svg, "noise-box");
assert!(!rects.is_empty());
assert_no_overlapping_rects(&rects, &svg);
```

Add local helpers:

```rust
#[derive(Debug, Clone, Copy)]
struct SvgRect {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

fn element_rects(svg: &str, class_name: &str) -> Vec<SvgRect> {
    let mut rects = Vec::new();
    let needle = format!("<rect class=\"{class_name}\"");
    let mut search_start = 0usize;
    while let Some(relative_start) = svg[search_start..].find(&needle) {
        let start = search_start + relative_start;
        let Some(end) = svg[start..].find("/>") else {
            break;
        };
        let attrs = &svg[start..start + end];
        if let (Some(x), Some(y), Some(width), Some(height)) = (
            svg_attr_i32(attrs, "x"),
            svg_attr_i32(attrs, "y"),
            svg_attr_i32(attrs, "width"),
            svg_attr_i32(attrs, "height"),
        ) {
            rects.push(SvgRect { x, y, width, height });
        }
        search_start = start + end;
    }
    rects
}

fn assert_no_overlapping_rects(rects: &[SvgRect], svg: &str) {
    for (left_index, left) in rects.iter().enumerate() {
        for (right_index, right) in rects.iter().enumerate().skip(left_index + 1) {
            let x_overlap = left.x < right.x + right.width && right.x < left.x + left.width;
            let y_overlap = left.y < right.y + right.height && right.y < left.y + left.height;
            assert!(
                !(x_overlap && y_overlap),
                "rect {left_index} {left:?} overlaps rect {right_index} {right:?}: {svg}"
            );
        }
    }
}
```

- [ ] **Step 2: Run SVG regression and verify RED**

Run:

```sh
cargo test -p rstim --test qp101_svg surface_code_atom_loss_svg_layout_regression -q
```

Expected: FAIL because current output uses `p=0.01` and same-layer H gates are
not packed.

- [ ] **Step 3: Implement layer packing**

In `rstim/src/qp101_svg.rs`, replace `count_visible_columns` and
`render_operations` column advancement with a layer-aware renderer:

- Accumulate non-`TICK` visible operations into a `LayerItem` buffer.
- Flush the buffer before rendering a `TICK`, before entering/leaving repeat
  bodies, and at the end of the operation list.
- For each flush, assign every item to the first column in the layer whose
  occupied lane interval does not intersect the item interval.
- Render all items with `x_for_column(layer_start + assigned_column)`.
- Advance the global column by the number of assigned columns.
- Render `TICK` in its own column and advance by one.

Use a lane interval per rendered item:

```rust
#[derive(Debug, Clone, Copy)]
struct LaneSpan {
    min: usize,
    max: usize,
}

impl LaneSpan {
    fn conflicts(self, other: LaneSpan) -> bool {
        self.min <= other.max && other.min <= self.max
    }
}
```

For gates/noise that render multiple independent boxes from one QP101 operation,
split into separate `LayerItem`s for packing:

- one item per single-qubit simple gate target;
- one item per controlled pair for `CX`/`CZ`;
- one item per target pair for `SWAP`;
- one item per known single-target noise box;
- one item per known paired `DEPOLARIZE2` pair;
- one fallback item spanning all lanes for generic or malformed operations.

Keep measurement-anchor state in operation order. If an operation produces
measurement anchors, record/render them on the first item for that operation.

- [ ] **Step 4: Implement decimal-only per-item known-noise labels**

Change `noise_param_note(params)` to return decimal values only:

```rust
Some(values)
```

For known `NoisePolicy::Single` and `NoisePolicy::Pair`, render the parameter
note once per rendered box or pair item, using the lane span for that item, not
once per whole operation. Fallback unknown noise may keep the generic joined
parameter text without the `p=` prefix.

- [ ] **Step 5: Verify SVG regression GREEN**

Run:

```sh
cargo test -p rstim --test qp101_svg surface_code_atom_loss_svg_layout_regression -q
cargo test -p rstim --test qp101_svg -q
```

Expected: PASS.

- [ ] **Step 6: Commit Task 3**

Run:

```sh
git add rstim/src/qp101_svg.rs rstim/tests/qp101_svg.rs
git commit -m "fix: pack qp101 svg atom-loss timelines"
```

### Task 4: End-To-End Verification And PR Prep

**Files:**
- Modify only if verification reveals a real issue in files touched by Tasks 1-3.

**Interfaces:**
- Consumes: all previous tasks.
- Produces: verified branch ready for PR.

- [ ] **Step 1: Run issue positive control**

Run:

```sh
cargo run -q -p rstim --bin rstim -- gen --code surface_code --task rotated_memory_x --distance 3 --rounds 3 --after_clifford_loss_probability 0.01 --out /tmp/rstim-surface-d3-r3-atom-loss.stim
```

Expected: exit 0; `/tmp/rstim-surface-d3-r3-atom-loss.stim` contains
`LOSS(0.01)` immediately after H and CX layers.

- [ ] **Step 2: Run issue negative control**

Run:

```sh
cargo run -q -p rstim --bin rstim -- gen --code surface_code --task rotated_memory_x --distance 3 --rounds 3 --after_clifford_depolarization 0.01 --out /tmp/rstim-surface-d3-r3-depol-only.stim
```

Expected: exit 0; output contains `DEPOLARIZE1(0.01)` and
`DEPOLARIZE2(0.01)` and does not contain `LOSS(0.01)`.

- [ ] **Step 3: Run required focused tests**

Run:

```sh
cargo test -p rstim --test gen_surface_code surface_code_after_clifford_atom_loss -q
cargo test -p rstim --test qp101_fixtures mixed_noise_showcase_circuit_uses_generated_after_clifford_loss_and_common_pauli_noise -q
cargo test -p rstim --test qp101_svg surface_code_atom_loss_svg_layout_regression -q
```

Expected: PASS.

- [ ] **Step 4: Run visualization sync checks**

Run:

```sh
cargo test -p rstim --test qp101_export --test qp101_fixtures --test cli_export_json
typst compile --root qp101-viz qp101-viz/examples/surface-code-rotated-memory-x-d3-r3-atom-loss.typ /tmp/surface-code-rotated-memory-x-d3-r3-atom-loss.pdf
```

Expected: PASS and Typst exits successfully.

- [ ] **Step 5: Run required broad verification**

Run:

```sh
cargo test
```

Expected: PASS.

- [ ] **Step 6: Commit any verification fixes**

If Step 1-5 required fixes, commit them with a scoped message such as:

```sh
git add rstim/src/codegen/noise_params.rs rstim/src/codegen/surface_code.rs rstim/src/cli.rs rstim/src/showcase.rs rstim/src/qp101_svg.rs rstim/tests/gen_surface_code.rs rstim/tests/cli_gen.rs rstim/tests/codegen_noise.rs rstim/tests/stim_codegen.rs rstim/tests/qp101_fixtures.rs rstim/tests/qp101_svg.rs rstim/tests/fixtures/qp101/surface_code_rotated_memory_x_d3_r3_mixed_noise.json rstim/tests/fixtures/qp101/surface_code_rotated_memory_x_d3_r3_mixed_noise_sample_seed7.json qp101-viz/examples/surface-code-rotated-memory-x-d3-r3-atom-loss.stim qp101-viz/examples/surface-code-rotated-memory-x-d3-r3-atom-loss.qp101.json qp101-viz/examples/surface-code-rotated-memory-x-d3-r3-atom-loss-sample.qp101.json
git commit -m "fix: complete surface-code atom-loss verification"
```

If no files changed, do not create an empty commit.

- [ ] **Step 7: Use finishing workflow**

Invoke `superpowers:finishing-a-development-branch`. Choose
`Push and create a Pull Request`, then push the worker branch and create a PR
against `master`.

## Plan Self-Review

- Coverage: Tasks cover generator/API, CLI, showcase regeneration, SVG renderer,
  fixture sync, and final verification.
- Placeholder scan: no unresolved marker text or unspecified file paths remain.
- Type consistency: `after_clifford_loss_probability` is the single new field
  name used across tests, API, CLI, and showcase.
- Scope: the plan does not change QP101 JSON schema or depolarization semantics.
