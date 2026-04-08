# DEM Provenance Tracking Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an opt-in tracked DEM analysis path that preserves exact circuit-source provenance through merge and decomposition, and export DEM-origin query results as QP101 JSON highlights.

**Architecture:** Keep the existing `DetectorErrorModel` and default `circuit_to_dem` path unchanged. Introduce a tracked analysis result with its own source table and bidirectional indices, teach the analyzer and decomposition pipeline to carry provenance in opt-in mode, then expose the queried origins through a QP101 `extensions.rstim_query_highlights` payload and an explicit CLI flag on `export_json`.

**Tech Stack:** Rust, `serde` / `serde_json`, existing `clap` CLI, existing `rstim` DEM analyzer and QP101 exporter, Rust integration tests via `cargo test`

---

## File Structure

- Create: `rstim/src/dem_provenance.rs`
  - Own tracked data structures shared by analysis and export: `SourceId`, `DemErrorId`, `SourceBranch`, `TrackedSource`, `TrackedErrorTerm`, `TrackedDemResult`, highlight-export structs, and small helper methods for reverse-index construction.
- Modify: `rstim/src/lib.rs`
  - Re-export the new provenance module.
- Modify: `rstim/src/error_analyzer.rs`
  - Add tracked analyzer entry points, recursive repeat-aware traversal, tracked source emission helpers, tracked merge support, and tracked decomposition support.
- Modify: `rstim/src/qp101.rs`
  - Add a helper that exports a base QP101 document plus `extensions.rstim_query_highlights`.
- Modify: `rstim/src/cli.rs`
  - Add an explicit `--highlight_dem_error <INDEX>` flag to `export_json` and route that path through tracked analysis.
- Create: `rstim/tests/tracked_dem.rs`
  - Focused provenance tests for source emission, repeat tracking, merge behavior, and reverse indices.
- Create: `rstim/tests/qp101_highlights.rs`
  - JSON-shape tests for highlight extension payloads.
- Modify: `rstim/tests/cli_export_json.rs`
  - CLI tests for the explicit export flag and invalid query handling.
- Modify: `rstim/doc/QP101-ZY.md`
  - Document the `extensions.rstim_query_highlights` extension after the implementation stabilizes.

### Task 1: Add Tracked Provenance Core Types

**Files:**
- Create: `rstim/src/dem_provenance.rs`
- Modify: `rstim/src/lib.rs`
- Test: `rstim/tests/tracked_dem.rs`

- [ ] **Step 1: Write the failing test**

Add a focused serialization and index-construction test to `rstim/tests/tracked_dem.rs`:

```rust
use rstim::dem::DemTarget;
use rstim::dem_provenance::{
    SourceBranch, TrackedDemResult, TrackedErrorTerm, TrackedSource,
};

#[test]
fn tracked_result_builds_reverse_indices() {
    let sources = vec![
        TrackedSource {
            source_id: 0,
            op_path: vec![3, 1],
            repeat_iterations: vec![2],
            instr_name: "DEPOLARIZE1".to_string(),
            target_slots: vec![0],
            target_qubits: vec![5],
            branch: SourceBranch::Y,
            probability_fragment: 0.125,
        },
        TrackedSource {
            source_id: 1,
            op_path: vec![3, 1],
            repeat_iterations: vec![2],
            instr_name: "DEPOLARIZE1".to_string(),
            target_slots: vec![1],
            target_qubits: vec![7],
            branch: SourceBranch::X,
            probability_fragment: 0.125,
        },
    ];
    let dem_terms = vec![
        TrackedErrorTerm {
            probability: 0.2,
            targets: vec![DemTarget::Detector(0)],
            source_ids: vec![0],
        },
        TrackedErrorTerm {
            probability: 0.3,
            targets: vec![DemTarget::Detector(1)],
            source_ids: vec![0, 1],
        },
    ];

    let result = TrackedDemResult::from_terms_and_sources(sources, dem_terms);

    assert_eq!(result.dem_error_to_sources[0], vec![0]);
    assert_eq!(result.dem_error_to_sources[1], vec![0, 1]);
    assert_eq!(result.source_to_dem_errors[0], vec![0, 1]);
    assert_eq!(result.source_to_dem_errors[1], vec![1]);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p rstim tracked_result_builds_reverse_indices --test tracked_dem`

Expected: FAIL with an unresolved import for `rstim::dem_provenance` or missing types such as `TrackedDemResult`.

- [ ] **Step 3: Write minimal implementation**

Create `rstim/src/dem_provenance.rs` with the initial tracked types and reverse-index builder:

```rust
use crate::dem::{DemTarget, DetectorErrorModel};
use serde::{Deserialize, Serialize};

pub type SourceId = usize;
pub type DemErrorId = usize;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SourceBranch {
    X,
    Y,
    Z,
    XX,
    XY,
    XZ,
    YX,
    YY,
    YZ,
    ZX,
    ZY,
    ZZ,
    MeasurementFlip,
    CorrelatedBranch { index: usize },
    Custom { label: String },
}

impl SourceBranch {
    pub fn label(&self) -> String {
        match self {
            SourceBranch::X => "X".to_string(),
            SourceBranch::Y => "Y".to_string(),
            SourceBranch::Z => "Z".to_string(),
            SourceBranch::XX => "XX".to_string(),
            SourceBranch::XY => "XY".to_string(),
            SourceBranch::XZ => "XZ".to_string(),
            SourceBranch::YX => "YX".to_string(),
            SourceBranch::YY => "YY".to_string(),
            SourceBranch::YZ => "YZ".to_string(),
            SourceBranch::ZX => "ZX".to_string(),
            SourceBranch::ZY => "ZY".to_string(),
            SourceBranch::ZZ => "ZZ".to_string(),
            SourceBranch::MeasurementFlip => "M".to_string(),
            SourceBranch::CorrelatedBranch { index } => format!("E{index}"),
            SourceBranch::Custom { label } => label.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TrackedSource {
    pub source_id: SourceId,
    pub op_path: Vec<usize>,
    pub repeat_iterations: Vec<u64>,
    pub instr_name: String,
    pub target_slots: Vec<usize>,
    pub target_qubits: Vec<u32>,
    pub branch: SourceBranch,
    pub probability_fragment: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TrackedErrorTerm {
    pub probability: f64,
    pub targets: Vec<DemTarget>,
    pub source_ids: Vec<SourceId>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TrackedDemResult {
    pub dem: DetectorErrorModel,
    pub sources: Vec<TrackedSource>,
    pub dem_error_to_sources: Vec<Vec<SourceId>>,
    pub source_to_dem_errors: Vec<Vec<DemErrorId>>,
}

impl TrackedDemResult {
    pub fn from_terms_and_sources(
        sources: Vec<TrackedSource>,
        terms: Vec<TrackedErrorTerm>,
    ) -> Self {
        let mut dem = DetectorErrorModel::new();
        let mut dem_error_to_sources = Vec::with_capacity(terms.len());
        for term in terms {
            dem.add_error(term.probability, term.targets);
            dem_error_to_sources.push(term.source_ids);
        }
        let mut source_to_dem_errors = vec![Vec::new(); sources.len()];
        for (dem_error_id, source_ids) in dem_error_to_sources.iter().enumerate() {
            for &source_id in source_ids {
                source_to_dem_errors[source_id].push(dem_error_id);
            }
        }
        Self {
            dem,
            sources,
            dem_error_to_sources,
            source_to_dem_errors,
        }
    }
}
```

Expose the module from `rstim/src/lib.rs`:

```rust
pub mod dem_provenance;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p rstim tracked_result_builds_reverse_indices --test tracked_dem`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add rstim/src/dem_provenance.rs rstim/src/lib.rs rstim/tests/tracked_dem.rs
git commit -m "feat: add tracked DEM provenance core types"
```

### Task 2: Add Repeat-Aware Tracked Traversal And Source Emission

**Files:**
- Modify: `rstim/src/error_analyzer.rs`
- Test: `rstim/tests/tracked_dem.rs`

- [ ] **Step 1: Write the failing test**

Extend `rstim/tests/tracked_dem.rs` with a repeat-aware single-source test:

```rust
use rstim::error_analyzer::ErrorAnalyzer;
use rstim::parser::parse_lines;

#[test]
fn tracked_dem_records_repeat_iteration_target_slot_and_branch() {
    let circuit = parse_lines(
        "REPEAT 2 {\n  DEPOLARIZE1(0.3) 5 7\n  TICK\n}\nM 5 7\nDETECTOR rec[-2]\nDETECTOR rec[-1]\n",
    )
    .unwrap();

    let tracked = ErrorAnalyzer::circuit_to_tracked_dem(&circuit).unwrap();
    let source = tracked
        .sources
        .iter()
        .find(|source| {
            source.repeat_iterations == vec![1]
                && source.target_slots == vec![1]
                && source.branch.label() == "Y"
        })
        .unwrap();

    assert_eq!(source.op_path, vec![0, 0]);
    assert_eq!(source.target_qubits, vec![7]);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p rstim tracked_dem_records_repeat_iteration_target_slot_and_branch --test tracked_dem`

Expected: FAIL because `ErrorAnalyzer::circuit_to_tracked_dem` does not exist.

- [ ] **Step 3: Write minimal implementation**

In `rstim/src/error_analyzer.rs`, add tracked entry points and a repeat-aware recursive traversal context instead of using the flatten-first path:

```rust
use crate::dem_provenance::{SourceBranch, SourceId, TrackedDemResult, TrackedErrorTerm, TrackedSource};

#[derive(Debug, Clone, Default)]
struct TrackingContext {
    op_path: Vec<usize>,
    repeat_iterations: Vec<u64>,
}

impl ErrorAnalyzer {
    pub fn circuit_to_tracked_dem(instrs: &[StimInstr]) -> Result<TrackedDemResult, String> {
        Self::circuit_to_tracked_dem_with_options(instrs, AnalyzeOptions::default())
    }

    pub fn circuit_to_tracked_dem_with_options(
        instrs: &[StimInstr],
        options: AnalyzeOptions,
    ) -> Result<TrackedDemResult, String> {
        let mut analyzer = Self::new_tracked(instrs, options)?;
        analyzer.undo_circuit_tracked(instrs, &mut TrackingContext::default())?;
        analyzer.finish_tracked_result()
    }

    fn undo_circuit_tracked(
        &mut self,
        instrs: &[StimInstr],
        ctx: &mut TrackingContext,
    ) -> Result<(), String> {
        for (op_index, instr) in instrs.iter().enumerate().rev() {
            ctx.op_path.push(op_index);
            match instr {
                StimInstr::Repeat { count, body } => {
                    for iter in (0..*count).rev() {
                        ctx.repeat_iterations.push(iter);
                        self.undo_circuit_tracked(body, ctx)?;
                        ctx.repeat_iterations.pop();
                    }
                }
                StimInstr::Op { name, args, targets, .. } => {
                    self.undo_op_tracked(name, args, targets, ctx)?;
                }
            }
            ctx.op_path.pop();
        }
        Ok(())
    }

    fn emit_tracked_source(
        &mut self,
        ctx: &TrackingContext,
        instr_name: &str,
        target_slots: Vec<usize>,
        target_qubits: Vec<u32>,
        branch: SourceBranch,
        probability_fragment: f64,
        targets: Vec<DemTarget>,
    ) {
        let source_id = self.tracked_sources.len();
        self.tracked_sources.push(TrackedSource {
            source_id,
            op_path: ctx.op_path.iter().rev().copied().collect(),
            repeat_iterations: ctx.repeat_iterations.iter().rev().copied().collect(),
            instr_name: instr_name.to_string(),
            target_slots,
            target_qubits,
            branch,
            probability_fragment,
        });
        self.tracked_terms.push(TrackedErrorTerm {
            probability: probability_fragment,
            targets,
            source_ids: vec![source_id],
        });
    }
}
```

Wire `DEPOLARIZE1` and measurement noise through this helper first. Keep the old non-tracked emission path intact.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p rstim tracked_dem_records_repeat_iteration_target_slot_and_branch --test tracked_dem`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add rstim/src/error_analyzer.rs rstim/tests/tracked_dem.rs
git commit -m "feat: add repeat-aware tracked DEM traversal"
```

### Task 3: Preserve Provenance Through Merge And Canonicalization

**Files:**
- Modify: `rstim/src/error_analyzer.rs`
- Test: `rstim/tests/tracked_dem.rs`

- [ ] **Step 1: Write the failing test**

Add a merge-precision test:

```rust
#[test]
fn tracked_dem_merge_keeps_exact_source_union() {
    let circuit = parse_lines(
        "R 0\nX_ERROR(0.1) 0\nX_ERROR(0.2) 0\nM 0\nDETECTOR rec[-1]\n",
    )
    .unwrap();

    let tracked = ErrorAnalyzer::circuit_to_tracked_dem(&circuit).unwrap();

    assert_eq!(tracked.dem.instructions().len(), 1);
    assert_eq!(tracked.dem_error_to_sources.len(), 1);
    assert_eq!(tracked.dem_error_to_sources[0].len(), 2);
    assert_eq!(tracked.source_to_dem_errors[0], vec![0]);
    assert_eq!(tracked.source_to_dem_errors[1], vec![0]);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p rstim tracked_dem_merge_keeps_exact_source_union --test tracked_dem`

Expected: FAIL because the tracked path still emits duplicate final terms instead of a merged provenance set.

- [ ] **Step 3: Write minimal implementation**

Teach the tracked finalizer in `rstim/src/error_analyzer.rs` to merge terms by canonicalized target set while unioning source ids:

```rust
use std::collections::BTreeMap;

fn merge_tracked_terms(terms: Vec<TrackedErrorTerm>) -> Vec<TrackedErrorTerm> {
    let mut merged: BTreeMap<Vec<DemTarget>, (f64, Vec<SourceId>)> = BTreeMap::new();
    for term in terms.into_iter().rev() {
        if term.probability <= 0.0 || term.targets.is_empty() {
            continue;
        }
        let key = canonicalize_error_targets(&term.targets);
        merged
            .entry(key)
            .and_modify(|(existing_prob, existing_sources)| {
                *existing_prob = *existing_prob + term.probability
                    - 2.0 * *existing_prob * term.probability;
                for source_id in &term.source_ids {
                    if !existing_sources.contains(source_id) {
                        existing_sources.push(*source_id);
                    }
                }
                existing_sources.sort_unstable();
            })
            .or_insert_with(|| {
                let mut source_ids = term.source_ids;
                source_ids.sort_unstable();
                source_ids.dedup();
                (term.probability, source_ids)
            });
    }

    merged
        .into_iter()
        .map(|(targets, (probability, source_ids))| TrackedErrorTerm {
            probability,
            targets,
            source_ids,
        })
        .collect()
}
```

Use this helper inside `finish_tracked_result()` before constructing `TrackedDemResult`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p rstim tracked_dem_merge_keeps_exact_source_union --test tracked_dem`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add rstim/src/error_analyzer.rs rstim/tests/tracked_dem.rs
git commit -m "feat: preserve provenance through DEM merge"
```

### Task 4: Preserve Provenance Through Decomposition

**Files:**
- Modify: `rstim/src/error_analyzer.rs`
- Test: `rstim/tests/tracked_dem.rs`

- [ ] **Step 1: Write the failing test**

Add a decomposition-specific provenance test:

```rust
#[test]
fn tracked_dem_decomposition_keeps_reverse_links() {
    let circuit = parse_lines(
        "R 0 1 2\nX_ERROR(0.1) 0\nCX 0 1\nCX 1 2\nM 0 1 2\nDETECTOR rec[-3]\nDETECTOR rec[-2]\nDETECTOR rec[-1]\n",
    )
    .unwrap();

    let tracked = ErrorAnalyzer::circuit_to_tracked_dem_decomposed(&circuit).unwrap();

    let dem_ids = &tracked.source_to_dem_errors[0];
    assert!(!dem_ids.is_empty());
    for &dem_id in dem_ids {
        assert!(tracked.dem_error_to_sources[dem_id].contains(&0));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p rstim tracked_dem_decomposition_keeps_reverse_links --test tracked_dem`

Expected: FAIL because the tracked decomposed API does not exist or decomposition drops provenance.

- [ ] **Step 3: Write minimal implementation**

Add tracked decomposed entry points and a tracked decomposition helper in `rstim/src/error_analyzer.rs`:

```rust
impl ErrorAnalyzer {
    pub fn circuit_to_tracked_dem_decomposed(
        instrs: &[StimInstr],
    ) -> Result<TrackedDemResult, String> {
        let mut tracked = Self::circuit_to_tracked_dem(instrs)?;
        decompose_tracked_terms(&mut tracked)?;
        Ok(tracked)
    }
}

fn decompose_tracked_terms(tracked: &mut TrackedDemResult) -> Result<(), String> {
    let mut rewritten_terms = Vec::new();
    for (dem_error_id, instr) in tracked.dem.instructions().iter().enumerate() {
        let DemInstruction::Error { probability, targets } = instr else {
            continue;
        };
        let rewritten_targets = rewrite_targets_to_graphlike(targets)?;
        rewritten_terms.push(TrackedErrorTerm {
            probability: *probability,
            targets: rewritten_targets,
            source_ids: tracked.dem_error_to_sources[dem_error_id].clone(),
        });
    }
    let merged_terms = merge_tracked_terms(rewritten_terms);
    let sources = tracked.sources.clone();
    *tracked = TrackedDemResult::from_terms_and_sources(sources, merged_terms);
    Ok(())
}
```

Reuse the same target-rewrite logic as the existing `decompose_errors`, but keep `source_ids` attached to each term throughout the rewrite and re-merge.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p rstim tracked_dem_decomposition_keeps_reverse_links --test tracked_dem`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add rstim/src/error_analyzer.rs rstim/tests/tracked_dem.rs
git commit -m "feat: preserve provenance through DEM decomposition"
```

### Task 5: Export Highlighted QP101 Documents

**Files:**
- Modify: `rstim/src/dem_provenance.rs`
- Modify: `rstim/src/qp101.rs`
- Test: `rstim/tests/qp101_highlights.rs`

- [ ] **Step 1: Write the failing test**

Create `rstim/tests/qp101_highlights.rs` with an export-shape test:

```rust
use rstim::error_analyzer::ErrorAnalyzer;
use rstim::parser::parse_lines;
use rstim::qp101::export_qp101_with_highlighted_dem_error;

#[test]
fn qp101_export_includes_dem_origin_highlights() {
    let circuit = parse_lines(
        "REPEAT 2 {\n  DEPOLARIZE1(0.3) 5 7\n}\nM 5 7\nDETECTOR rec[-2]\nDETECTOR rec[-1]\n",
    )
    .unwrap();
    let tracked = ErrorAnalyzer::circuit_to_tracked_dem(&circuit).unwrap();

    let doc = export_qp101_with_highlighted_dem_error(&circuit, &tracked, 0).unwrap();
    let value = serde_json::to_value(doc).unwrap();

    assert_eq!(value["extensions"]["rstim_query_highlights"]["query"]["kind"], "dem_error_origin");
    assert_eq!(value["extensions"]["rstim_query_highlights"]["highlights"][0]["target_slots"], serde_json::json!([0]));
    assert!(value["extensions"]["rstim_query_highlights"]["highlights"][0]["repeat_iterations"].is_array());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p rstim qp101_export_includes_dem_origin_highlights --test qp101_highlights`

Expected: FAIL with missing `export_qp101_with_highlighted_dem_error`.

- [ ] **Step 3: Write minimal implementation**

Add highlight-export structs to `rstim/src/dem_provenance.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HighlightRecord {
    pub op_path: Vec<usize>,
    pub repeat_iterations: Vec<u64>,
    pub target_slots: Vec<usize>,
    pub target_qubits: Vec<u32>,
    pub branch: String,
    pub label: String,
}

impl HighlightRecord {
    pub fn from_source(source: &TrackedSource) -> Self {
        let label = source.branch.label();
        Self {
            op_path: source.op_path.clone(),
            repeat_iterations: source.repeat_iterations.clone(),
            target_slots: source.target_slots.clone(),
            target_qubits: source.target_qubits.clone(),
            branch: label.clone(),
            label,
        }
    }
}
```

Add the export helper to `rstim/src/qp101.rs`:

```rust
use crate::dem_provenance::{HighlightRecord, TrackedDemResult};
use serde_json::json;

pub fn export_qp101_with_highlighted_dem_error(
    instrs: &[StimInstr],
    tracked: &TrackedDemResult,
    dem_error_index: usize,
) -> Result<Qp101Document, String> {
    let source_ids = tracked
        .dem_error_to_sources
        .get(dem_error_index)
        .ok_or_else(|| format!("DEM error index out of range: {dem_error_index}"))?;

    let mut highlights = Vec::new();
    for &source_id in source_ids {
        highlights.push(HighlightRecord::from_source(&tracked.sources[source_id]));
    }

    let mut doc = export_qp101(instrs)?;
    doc.extensions = Some(json!({
        "rstim_query_highlights": {
            "version": "1",
            "query": {
                "kind": "dem_error_origin",
                "dem_error_index": dem_error_index,
            },
            "highlights": highlights,
        }
    }));
    Ok(doc)
}
```

Add a small dedup pass over `highlights` keyed by `op_path`, `repeat_iterations`, `target_slots`, and `branch` before writing them into the document.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p rstim qp101_export_includes_dem_origin_highlights --test qp101_highlights`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add rstim/src/dem_provenance.rs rstim/src/qp101.rs rstim/tests/qp101_highlights.rs
git commit -m "feat: export DEM-origin highlights in QP101"
```

### Task 6: Add Explicit CLI Query Export

**Files:**
- Modify: `rstim/src/cli.rs`
- Modify: `rstim/tests/cli_export_json.rs`

- [ ] **Step 1: Write the failing test**

Extend `rstim/tests/cli_export_json.rs`:

```rust
#[test]
fn export_json_can_highlight_dem_error_origins() {
    let output = run_export_json_with_stdin(
        &["--highlight_dem_error", "0"],
        "REPEAT 2 {\n  DEPOLARIZE1(0.3) 5 7\n}\nM 5 7\nDETECTOR rec[-2]\nDETECTOR rec[-1]\n",
    );
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        value["extensions"]["rstim_query_highlights"]["query"]["dem_error_index"],
        0
    );
}

#[test]
fn export_json_rejects_invalid_highlight_dem_error_index() {
    let output = run_export_json_with_stdin(
        &["--highlight_dem_error", "99"],
        "X_ERROR(0.1) 0\nM 0\nDETECTOR rec[-1]\n",
    );
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("DEM error index out of range"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p rstim --test cli_export_json export_json_can_highlight_dem_error_origins`

Expected: FAIL because `export_json` does not accept `--highlight_dem_error`.

- [ ] **Step 3: Write minimal implementation**

In `rstim/src/cli.rs`, extend the command shape and export path:

```rust
#[command(name = "export_json")]
ExportJson {
    #[arg(long = "in")]
    r#in: Option<String>,
    #[arg(long)]
    out: Option<String>,
    #[arg(long, default_value = "pretty")]
    format: String,
    #[arg(long = "highlight_dem_error")]
    highlight_dem_error: Option<usize>,
},
```

Update the runner:

```rust
fn run_export_json(
    text: &str,
    format: JsonOutputFormat,
    highlight_dem_error: Option<usize>,
    w: &mut dyn Write,
) -> Result<(), String> {
    let instrs = parse_lines(text)?;
    let doc = if let Some(dem_error_index) = highlight_dem_error {
        let tracked = crate::error_analyzer::ErrorAnalyzer::circuit_to_tracked_dem(&instrs)?;
        crate::qp101::export_qp101_with_highlighted_dem_error(
            &instrs,
            &tracked,
            dem_error_index,
        )?
    } else {
        crate::qp101::export_qp101(&instrs)?
    };

    match format {
        JsonOutputFormat::Pretty => serde_json::to_writer_pretty(&mut *w, &doc)
            .map_err(|e| format!("write error: {e}"))?,
        JsonOutputFormat::Compact => serde_json::to_writer(&mut *w, &doc)
            .map_err(|e| format!("write error: {e}"))?,
    }
    w.write_all(b"\n").map_err(|e| format!("write error: {e}"))?;
    Ok(())
}
```

Keep the default path byte-for-byte equivalent when `highlight_dem_error` is `None`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p rstim --test cli_export_json export_json_can_highlight_dem_error_origins export_json_rejects_invalid_highlight_dem_error_index`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add rstim/src/cli.rs rstim/tests/cli_export_json.rs
git commit -m "feat: add CLI export for DEM-origin highlights"
```

### Task 7: Update The QP101 Extension Documentation

**Files:**
- Modify: `rstim/doc/QP101-ZY.md`
- Test: none

- [ ] **Step 1: Write the doc diff**

Add a new subsection under the top-level `extensions` discussion in `rstim/doc/QP101-ZY.md`:

```markdown
## rstim Query Highlight Extension

`rstim` may attach query-specific visualization metadata under
`extensions.rstim_query_highlights`.

This extension is not part of the generic QP101 core schema. It preserves
query results such as "which circuit-side noise sources produced DEM error
line N" without mutating the base circuit structure.

Example:

~~~json
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
~~~
```

- [ ] **Step 2: Review the rendered section for consistency**

Run: `sed -n '1,260p' rstim/doc/QP101-ZY.md`

Expected: the new extension section uses `target_slots`, matches the implementation, and does not redefine the QP101 core schema.

- [ ] **Step 3: Commit**

```bash
git add rstim/doc/QP101-ZY.md
git commit -m "docs: describe rstim query highlight extension"
```

## Self-Review Checklist

- Spec coverage:
  - tracked core types: Tasks 1-2
  - merge and decomposition preservation: Tasks 3-4
  - QP101 highlight export: Task 5
  - explicit opt-in CLI path: Task 6
  - extension documentation: Task 7
- Placeholder scan:
  - no placeholder markers remain
  - every code-writing step includes concrete code blocks
  - every verification step includes an exact command and expected result
- Type consistency:
  - `target_slots` is used consistently in tracked types, JSON export, and docs
  - `SourceBranch::label()` is the single display-label source
  - `export_qp101_with_highlighted_dem_error(...)` remains the only QP101 helper that injects query metadata
