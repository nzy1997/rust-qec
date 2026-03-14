# rstim / Stim Parity Showcase Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a reusable correctness harness and a timing/report tool that demonstrate `rstim` and `stim` are closely aligned on representative `gen` and `analyze_errors` workflows.

**Architecture:** Put the stable comparison logic in a small reusable library module so both tests and the reporting tool can share one case matrix and one semantic comparison implementation. Use one integration test for deterministic correctness and one example binary for markdown timing output; keep timing outside pass/fail tests.

**Tech Stack:** Rust, existing `std::process::Command` test style, `tempfile`, `cargo test`, `cargo run --example`, local `stim` CLI via `RSTIM_TEST_STIM`

---

## Preflight

- Worktree setup was attempted but blocked by sandbox permissions when creating `.git/worktrees/...`. Implement this plan either:
  - in a fresh unsandboxed session with `using-git-worktrees`, or
  - in the current workspace if isolation is not available.
- Use `/Users/nzy/rcode/rstim/docs/plans/2026-03-14-rstim-stim-parity-showcase-design.md` as the design source of truth.
- Reuse the existing `RSTIM_TEST_STIM` override pattern from `/Users/nzy/rcode/rstim/rstim/tests/cross_validate_dem.rs`.
- Keep `gen` parity noiseless and `DEM` parity noisy with `stim gen --after_clifford_depolarization 0.001`.

### Task 1: Add Reusable Showcase Comparison Helpers

**Files:**
- Create: `/Users/nzy/rcode/rstim/rstim/src/showcase.rs`
- Modify: `/Users/nzy/rcode/rstim/rstim/src/lib.rs`
- Test: `/Users/nzy/rcode/rstim/rstim/tests/showcase_helpers.rs`

**Step 1: Write the failing test**

```rust
use rstim::dem::{DemInstruction, DetectorErrorModel};
use rstim::parser::parse_lines;
use rstim::showcase::{
    dem_semantic_summary,
    showcase_cases,
    strip_comment_preamble,
    structural_circuit_summary,
};

#[test]
fn showcase_cases_cover_expected_matrix() {
    let labels: Vec<String> = showcase_cases().into_iter().map(|c| c.label()).collect();
    assert_eq!(labels.len(), 6);
    assert!(labels.contains(&"repetition_code/memory d=5 r=5".to_string()));
    assert!(labels.contains(&"repetition_code/memory d=13 r=13".to_string()));
    assert!(labels.contains(&"surface_code/rotated_memory_x d=5 r=5".to_string()));
    assert!(labels.contains(&"surface_code/rotated_memory_x d=13 r=13".to_string()));
    assert!(labels.contains(&"surface_code/rotated_memory_z d=5 r=5".to_string()));
    assert!(labels.contains(&"surface_code/rotated_memory_z d=13 r=13".to_string()));
}

#[test]
fn strip_comment_preamble_drops_leading_stim_header_only() {
    let text = "# header\n# header\nR 0\n# inline stays comment to parser\nM 0\n";
    assert_eq!(strip_comment_preamble(text), "R 0\n# inline stays comment to parser\nM 0\n");
}

#[test]
fn structural_circuit_summary_counts_repeat_and_annotations() {
    let instrs = parse_lines(
        "QUBIT_COORDS(1, 2) 0\nR 0\nREPEAT 2 {\n    M 0\n    DETECTOR(1, 0) rec[-1]\n}\nOBSERVABLE_INCLUDE(0) rec[-1]\n",
    )
    .unwrap();
    let summary = structural_circuit_summary(&instrs);
    assert_eq!(summary.measurements, 2);
    assert_eq!(summary.detectors, 2);
    assert_eq!(summary.observables, 1);
    assert_eq!(summary.opcode_counts["M"], 2);
    assert!(summary.qubit_coords.contains(&"QUBIT_COORDS(1,2) 0".to_string()));
}

#[test]
fn dem_semantic_summary_flattens_repeat_blocks_and_shifted_detectors() {
    let dem = DetectorErrorModel::parse(
        "error(0.125) D0\nrepeat 2 {\n    error(0.25) D0 D1\n    shift_detectors 2\n    detector(5, 0) D0\n}\n",
    )
    .unwrap();
    let summary = dem_semantic_summary(&dem);
    assert!(summary.error_probabilities.contains_key("D0"));
    assert!(summary.error_probabilities.contains_key("D0 D1"));
    assert!(summary.annotation_lines.iter().any(|line| line.starts_with("detector(5,0) D2")));
}
```

**Step 2: Run test to verify it fails**

Run:

```bash
CARGO_HOME=/tmp/rstim-cargo-home cargo test -p rstim --test showcase_helpers
```

Expected: FAIL with unresolved import errors for `rstim::showcase`.

**Step 3: Write minimal implementation**

```rust
// /Users/nzy/rcode/rstim/rstim/src/showcase.rs
use std::collections::{BTreeMap, BTreeSet};

use crate::dem::{DemInstruction, DemTarget, DetectorErrorModel};
use crate::ir::{StimInstr, StimTarget};
use crate::stats;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShowcaseCase {
    pub code: &'static str,
    pub task: &'static str,
    pub distance: usize,
    pub rounds: usize,
}

impl ShowcaseCase {
    pub fn label(&self) -> String {
        format!("{}/{} d={} r={}", self.code, self.task, self.distance, self.rounds)
    }
}

pub fn showcase_cases() -> Vec<ShowcaseCase> {
    vec![
        ShowcaseCase { code: "repetition_code", task: "memory", distance: 5, rounds: 5 },
        ShowcaseCase { code: "repetition_code", task: "memory", distance: 13, rounds: 13 },
        ShowcaseCase { code: "surface_code", task: "rotated_memory_x", distance: 5, rounds: 5 },
        ShowcaseCase { code: "surface_code", task: "rotated_memory_x", distance: 13, rounds: 13 },
        ShowcaseCase { code: "surface_code", task: "rotated_memory_z", distance: 5, rounds: 5 },
        ShowcaseCase { code: "surface_code", task: "rotated_memory_z", distance: 13, rounds: 13 },
    ]
}

pub fn strip_comment_preamble(text: &str) -> &str {
    let mut offset = 0usize;
    for line in text.split_inclusive('\n') {
        if line.trim_start().starts_with('#') || line.trim().is_empty() {
            offset += line.len();
            continue;
        }
        break;
    }
    &text[offset..]
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CircuitSummary {
    pub opcode_counts: BTreeMap<String, usize>,
    pub measurements: usize,
    pub detectors: usize,
    pub observables: usize,
    pub qubit_coords: BTreeSet<String>,
    pub detector_annotations: BTreeSet<String>,
    pub observable_includes: BTreeSet<String>,
}

pub fn structural_circuit_summary(instrs: &[StimInstr]) -> CircuitSummary {
    let mut summary = CircuitSummary {
        opcode_counts: BTreeMap::new(),
        measurements: stats::num_measurements(instrs),
        detectors: stats::num_detectors(instrs),
        observables: stats::num_observables(instrs),
        qubit_coords: BTreeSet::new(),
        detector_annotations: BTreeSet::new(),
        observable_includes: BTreeSet::new(),
    };
    accumulate_instrs(instrs, &mut summary);
    summary
}

fn accumulate_instrs(instrs: &[StimInstr], summary: &mut CircuitSummary) {
    for instr in instrs {
        match instr {
            StimInstr::Op { name, args, targets, .. } => {
                *summary.opcode_counts.entry(name.clone()).or_default() += 1;
                match name.as_str() {
                    "QUBIT_COORDS" => {
                        let q = targets[0].qubit_index().unwrap();
                        summary.qubit_coords.insert(format!("QUBIT_COORDS({}) {}", format_args(args), q));
                    }
                    "DETECTOR" => {
                        summary.detector_annotations.insert(format!("DETECTOR({}) {}", format_args(args), format_targets(targets)));
                    }
                    "OBSERVABLE_INCLUDE" => {
                        summary.observable_includes.insert(format!("OBSERVABLE_INCLUDE({}) {}", format_args(args), format_targets(targets)));
                    }
                    _ => {}
                }
            }
            StimInstr::Repeat { count, body } => {
                *summary.opcode_counts.entry("REPEAT".to_string()).or_default() += 1;
                for _ in 0..*count {
                    accumulate_instrs(body, summary);
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DemSummary {
    pub error_probabilities: BTreeMap<String, f64>,
    pub annotation_lines: Vec<String>,
}

pub fn dem_semantic_summary(dem: &DetectorErrorModel) -> DemSummary {
    let mut summary = DemSummary {
        error_probabilities: BTreeMap::new(),
        annotation_lines: Vec::new(),
    };
    accumulate_dem(dem.instructions(), 0, &mut summary);
    summary.annotation_lines.sort();
    summary
}

fn accumulate_dem(instrs: &[DemInstruction], detector_offset: usize, summary: &mut DemSummary) -> usize {
    let mut offset = detector_offset;
    for instr in instrs {
        match instr {
            DemInstruction::Error { probability, targets } => {
                summary.error_probabilities.insert(format_dem_targets(targets, offset), *probability);
            }
            DemInstruction::Detector { index, coords } => {
                summary.annotation_lines.push(format!("detector({}) D{}", format_args(coords), index + offset));
            }
            DemInstruction::ShiftDetectors { detector_offset, coord_offsets } => {
                summary.annotation_lines.push(format!("shift_detectors({}) {}", format_args(coord_offsets), detector_offset));
                offset += detector_offset;
            }
            DemInstruction::LogicalObservable { index } => {
                summary.annotation_lines.push(format!("logical_observable L{}", index));
            }
            DemInstruction::Repeat { count, body } => {
                for _ in 0..*count {
                    offset = accumulate_dem(body.instructions(), offset, summary);
                }
            }
        }
    }
    offset
}
```

```rust
// /Users/nzy/rcode/rstim/rstim/src/lib.rs
pub mod showcase;
```

**Step 4: Run test to verify it passes**

Run:

```bash
CARGO_HOME=/tmp/rstim-cargo-home cargo test -p rstim --test showcase_helpers
```

Expected: PASS.

**Step 5: Commit**

```bash
git add /Users/nzy/rcode/rstim/rstim/src/showcase.rs /Users/nzy/rcode/rstim/rstim/src/lib.rs /Users/nzy/rcode/rstim/rstim/tests/showcase_helpers.rs
git commit -m "feat: add showcase comparison helpers"
```

### Task 2: Add Deterministic Gen Parity Integration Test

**Files:**
- Create: `/Users/nzy/rcode/rstim/rstim/tests/stim_parity_showcase.rs`

**Step 1: Write the failing test**

```rust
use rstim::parser::parse_lines;
use rstim::showcase::{showcase_cases, strip_comment_preamble, structural_circuit_summary};
use std::process::{Command, Stdio};

fn stim_cmd() -> String {
    std::env::var("RSTIM_TEST_STIM").unwrap_or_else(|_| "stim".to_string())
}

fn run_capture(cmd: &str, args: &[&str]) -> String {
    let output = Command::new(cmd).args(args).output().unwrap();
    assert!(output.status.success(), "command failed: {cmd} {args:?}\n{}", String::from_utf8_lossy(&output.stderr));
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn showcase_gen_parity_matches_structurally() {
    for case in showcase_cases() {
        let stim_text = run_capture(
            &stim_cmd(),
            &[
                "gen",
                "--code", case.code,
                "--task", case.task,
                "--distance", &case.distance.to_string(),
                "--rounds", &case.rounds.to_string(),
            ],
        );
        let rstim_text = run_capture(
            env!("CARGO_BIN_EXE_rstim"),
            &[
                "gen",
                "--code", case.code,
                "--task", case.task,
                "--distance", &case.distance.to_string(),
                "--rounds", &case.rounds.to_string(),
            ],
        );

        let stim_norm = strip_comment_preamble(&stim_text);
        let stim_instrs = parse_lines(stim_norm).unwrap();
        let rstim_instrs = parse_lines(&rstim_text).unwrap();
        assert_eq!(
            structural_circuit_summary(&stim_instrs),
            structural_circuit_summary(&rstim_instrs),
            "gen mismatch for {}",
            case.label(),
        );
    }
}
```

**Step 2: Run test to verify it fails**

Run:

```bash
CARGO_HOME=/tmp/rstim-cargo-home cargo test -p rstim --test stim_parity_showcase showcase_gen_parity_matches_structurally -- --nocapture
```

Expected: FAIL on the first structural mismatch, likely in `repetition_code` or rotated surface code annotation formatting.

**Step 3: Write minimal implementation**

Use the test failure to tighten `/Users/nzy/rcode/rstim/rstim/src/showcase.rs` until the comparison matches intended semantics:

- flatten `REPEAT` bodies when counting opcodes
- preserve `QUBIT_COORDS` numeric formatting without spaces
- compare `DETECTOR` and `OBSERVABLE_INCLUDE` annotations using canonical target rendering
- include `SHIFT_COORDS` in opcode counts but do not treat it as a parity failure on its own

If the raw stripped texts match exactly for a case, keep that as a fast path:

```rust
if strip_comment_preamble(&stim_text) == rstim_text {
    continue;
}
```

**Step 4: Run test to verify it passes**

Run:

```bash
CARGO_HOME=/tmp/rstim-cargo-home cargo test -p rstim --test stim_parity_showcase showcase_gen_parity_matches_structurally
```

Expected: PASS.

**Step 5: Commit**

```bash
git add /Users/nzy/rcode/rstim/rstim/tests/stim_parity_showcase.rs /Users/nzy/rcode/rstim/rstim/src/showcase.rs
git commit -m "test: add showcase circuit generation parity checks"
```

### Task 3: Add Deterministic DEM Parity Integration Test

**Files:**
- Modify: `/Users/nzy/rcode/rstim/rstim/tests/stim_parity_showcase.rs`
- Modify: `/Users/nzy/rcode/rstim/rstim/src/showcase.rs`

**Step 1: Write the failing test**

```rust
use rstim::dem::DetectorErrorModel;
use rstim::showcase::{dem_semantic_summary, showcase_cases};
use std::io::Write;

fn run_with_stdin(cmd: &str, args: &[&str], stdin_data: &str) -> String {
    let mut child = Command::new(cmd)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(stdin_data.as_bytes()).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "command failed: {cmd} {args:?}\n{}", String::from_utf8_lossy(&output.stderr));
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn showcase_dem_parity_matches_semantically() {
    for case in showcase_cases() {
        let noisy_circuit = run_capture(
            &stim_cmd(),
            &[
                "gen",
                "--code", case.code,
                "--task", case.task,
                "--distance", &case.distance.to_string(),
                "--rounds", &case.rounds.to_string(),
                "--after_clifford_depolarization", "0.001",
            ],
        );
        let stim_dem = run_with_stdin(&stim_cmd(), &["analyze_errors"], &noisy_circuit);
        let rstim_dem = run_with_stdin(env!("CARGO_BIN_EXE_rstim"), &["analyze_errors"], &noisy_circuit);
        let stim_summary = dem_semantic_summary(&DetectorErrorModel::parse(&stim_dem).unwrap());
        let rstim_summary = dem_semantic_summary(&DetectorErrorModel::parse(&rstim_dem).unwrap());
        assert_eq!(stim_summary.annotation_lines, rstim_summary.annotation_lines, "annotation mismatch for {}", case.label());
        assert_eq!(stim_summary.error_probabilities.keys().collect::<Vec<_>>(), rstim_summary.error_probabilities.keys().collect::<Vec<_>>(), "target mismatch for {}", case.label());
        for (targets, stim_p) in &stim_summary.error_probabilities {
            let rstim_p = rstim_summary.error_probabilities[targets];
            let rel = (stim_p - rstim_p).abs() / stim_p.max(1e-20);
            assert!(rel <= 1e-12, "probability mismatch for {} in {}: stim={} rstim={} rel={}", targets, case.label(), stim_p, rstim_p, rel);
        }
    }
}
```

**Step 2: Run test to verify it fails**

Run:

```bash
CARGO_HOME=/tmp/rstim-cargo-home cargo test -p rstim --test stim_parity_showcase showcase_dem_parity_matches_semantically -- --nocapture
```

Expected: FAIL on annotation canonicalization or flattened detector offset handling.

**Step 3: Write minimal implementation**

Extend `/Users/nzy/rcode/rstim/rstim/src/showcase.rs` with exact canonicalization helpers:

```rust
fn format_dem_targets(targets: &[DemTarget], detector_offset: usize) -> String {
    targets
        .iter()
        .map(|target| match target {
            DemTarget::Detector(index) => format!("D{}", index + detector_offset),
            DemTarget::Observable(index) => format!("L{}", index),
            DemTarget::Separator => "^".to_string(),
        })
        .collect::<Vec<_>>()
        .join(" ")
}
```

Also ensure flattened annotation handling is stable across repeats:

- repeat bodies must be expanded in order
- `shift_detectors` must increment the running detector offset
- detector annotations must be emitted with shifted detector indices
- annotation output should use lowercase `detector` and `shift_detectors`, matching DEM text rendering

**Step 4: Run test to verify it passes**

Run:

```bash
CARGO_HOME=/tmp/rstim-cargo-home cargo test -p rstim --test stim_parity_showcase showcase_dem_parity_matches_semantically
```

Expected: PASS.

**Step 5: Commit**

```bash
git add /Users/nzy/rcode/rstim/rstim/tests/stim_parity_showcase.rs /Users/nzy/rcode/rstim/rstim/src/showcase.rs
git commit -m "test: add showcase dem parity checks"
```

### Task 4: Add Markdown Timing Report Example

**Files:**
- Create: `/Users/nzy/rcode/rstim/rstim/examples/stim_parity_showcase.rs`
- Modify: `/Users/nzy/rcode/rstim/rstim/src/showcase.rs`

**Step 1: Write the failing test**

Add a small helper-focused regression in `/Users/nzy/rcode/rstim/rstim/tests/showcase_helpers.rs`:

```rust
use rstim::showcase::{median_duration_ns, render_markdown_table};
use std::time::Duration;

#[test]
fn median_duration_ns_picks_middle_value() {
    let values = vec![
        Duration::from_millis(30),
        Duration::from_millis(10),
        Duration::from_millis(20),
    ];
    assert_eq!(median_duration_ns(&values), 20_000_000);
}

#[test]
fn render_markdown_table_includes_expected_headers() {
    let table = render_markdown_table(&[
        vec![
            "repetition_code/memory d=5 r=5".to_string(),
            "exact".to_string(),
            "match".to_string(),
            "0".to_string(),
            "1.0".to_string(),
            "1.1".to_string(),
            "2.0".to_string(),
            "2.4".to_string(),
            "1.10x".to_string(),
            "1.20x".to_string(),
        ],
    ]);
    assert!(table.contains("| Case | Gen | DEM |"));
    assert!(table.contains("repetition_code/memory d=5 r=5"));
}
```

**Step 2: Run test to verify it fails**

Run:

```bash
CARGO_HOME=/tmp/rstim-cargo-home cargo test -p rstim --test showcase_helpers median_duration_ns_picks_middle_value render_markdown_table_includes_expected_headers
```

Expected: FAIL with unresolved helper names.

**Step 3: Write minimal implementation**

Add reporting helpers to `/Users/nzy/rcode/rstim/rstim/src/showcase.rs`:

```rust
pub fn median_duration_ns(values: &[std::time::Duration]) -> u128 {
    let mut nanos: Vec<u128> = values.iter().map(|d| d.as_nanos()).collect();
    nanos.sort();
    nanos[nanos.len() / 2]
}

pub fn render_markdown_table(rows: &[Vec<String>]) -> String {
    let mut out = String::from("| Case | Gen | DEM | Max Rel Error | Stim Gen ms | rstim Gen ms | Stim DEM ms | rstim DEM ms | Gen Ratio | DEM Ratio |\n");
    out.push_str("| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |\n");
    for row in rows {
        out.push('|');
        out.push(' ');
        out.push_str(&row.join(" | "));
        out.push_str(" |\n");
    }
    out
}
```

Create `/Users/nzy/rcode/rstim/rstim/examples/stim_parity_showcase.rs` using those helpers:

```rust
use rstim::dem::DetectorErrorModel;
use rstim::parser::parse_lines;
use rstim::showcase::{
    dem_semantic_summary,
    median_duration_ns,
    render_markdown_table,
    showcase_cases,
    strip_comment_preamble,
    structural_circuit_summary,
};
use std::io::Write;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn timed_capture(cmd: &str, args: &[&str], stdin_data: Option<&str>) -> (String, Duration) {
    let start = Instant::now();
    let output = if let Some(stdin_data) = stdin_data {
        let mut child = Command::new(cmd)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(stdin_data.as_bytes()).unwrap();
        child.wait_with_output().unwrap()
    } else {
        Command::new(cmd).args(args).output().unwrap()
    };
    assert!(output.status.success(), "command failed: {cmd} {args:?}\n{}", String::from_utf8_lossy(&output.stderr));
    (String::from_utf8(output.stdout).unwrap(), start.elapsed())
}

fn main() {
    let stim_cmd = std::env::var("RSTIM_TEST_STIM").unwrap_or_else(|_| "stim".to_string());
    let rstim_cmd = env!("CARGO_BIN_EXE_rstim");
    let mut rows = Vec::new();

    for case in showcase_cases() {
        // warmup
        let (noisy_circuit, _) = timed_capture(
            &stim_cmd,
            &[
                "gen",
                "--code", case.code,
                "--task", case.task,
                "--distance", &case.distance.to_string(),
                "--rounds", &case.rounds.to_string(),
                "--after_clifford_depolarization", "0.001",
            ],
            None,
        );
        let _ = timed_capture(&stim_cmd, &["analyze_errors"], Some(&noisy_circuit));
        let _ = timed_capture(rstim_cmd, &["analyze_errors"], Some(&noisy_circuit));

        let mut stim_gen_times = Vec::new();
        let mut rstim_gen_times = Vec::new();
        let mut stim_dem_times = Vec::new();
        let mut rstim_dem_times = Vec::new();
        let mut last_stim_text = String::new();
        let mut last_rstim_text = String::new();
        let mut last_stim_dem = String::new();
        let mut last_rstim_dem = String::new();

        for _ in 0..5 {
            let (stim_text, stim_gen_time) = timed_capture(
                &stim_cmd,
                &[
                    "gen",
                    "--code", case.code,
                    "--task", case.task,
                    "--distance", &case.distance.to_string(),
                    "--rounds", &case.rounds.to_string(),
                ],
                None,
            );
            let (rstim_text, rstim_gen_time) = timed_capture(
                rstim_cmd,
                &[
                    "gen",
                    "--code", case.code,
                    "--task", case.task,
                    "--distance", &case.distance.to_string(),
                    "--rounds", &case.rounds.to_string(),
                ],
                None,
            );
            let (stim_noisy_text, _) = timed_capture(
                &stim_cmd,
                &[
                    "gen",
                    "--code", case.code,
                    "--task", case.task,
                    "--distance", &case.distance.to_string(),
                    "--rounds", &case.rounds.to_string(),
                    "--after_clifford_depolarization", "0.001",
                ],
                None,
            );
            let (stim_dem, stim_dem_time) = timed_capture(&stim_cmd, &["analyze_errors"], Some(&stim_noisy_text));
            let (rstim_dem, rstim_dem_time) = timed_capture(rstim_cmd, &["analyze_errors"], Some(&stim_noisy_text));

            stim_gen_times.push(stim_gen_time);
            rstim_gen_times.push(rstim_gen_time);
            stim_dem_times.push(stim_dem_time);
            rstim_dem_times.push(rstim_dem_time);
            last_stim_text = stim_text;
            last_rstim_text = rstim_text;
            last_stim_dem = stim_dem;
            last_rstim_dem = rstim_dem;
        }

        let gen_status = if strip_comment_preamble(&last_stim_text) == last_rstim_text {
            "exact".to_string()
        } else {
            let stim_instrs = parse_lines(strip_comment_preamble(&last_stim_text)).unwrap();
            let rstim_instrs = parse_lines(&last_rstim_text).unwrap();
            if structural_circuit_summary(&stim_instrs) == structural_circuit_summary(&rstim_instrs) {
                "normalized".to_string()
            } else {
                "mismatch".to_string()
            }
        };

        let stim_summary = dem_semantic_summary(&DetectorErrorModel::parse(&last_stim_dem).unwrap());
        let rstim_summary = dem_semantic_summary(&DetectorErrorModel::parse(&last_rstim_dem).unwrap());
        let mut max_rel = 0.0f64;
        for (targets, stim_p) in &stim_summary.error_probabilities {
            let rstim_p = rstim_summary.error_probabilities[targets];
            let rel = (stim_p - rstim_p).abs() / stim_p.max(1e-20);
            max_rel = max_rel.max(rel);
        }
        let dem_status = if stim_summary == rstim_summary { "match" } else { "mismatch" };

        let stim_gen_ns = median_duration_ns(&stim_gen_times);
        let rstim_gen_ns = median_duration_ns(&rstim_gen_times);
        let stim_dem_ns = median_duration_ns(&stim_dem_times);
        let rstim_dem_ns = median_duration_ns(&rstim_dem_times);

        rows.push(vec![
            case.label(),
            gen_status,
            dem_status.to_string(),
            format!("{max_rel:.3e}"),
            format!("{:.3}", stim_gen_ns as f64 / 1_000_000.0),
            format!("{:.3}", rstim_gen_ns as f64 / 1_000_000.0),
            format!("{:.3}", stim_dem_ns as f64 / 1_000_000.0),
            format!("{:.3}", rstim_dem_ns as f64 / 1_000_000.0),
            format!("{:.2}x", rstim_gen_ns as f64 / stim_gen_ns as f64),
            format!("{:.2}x", rstim_dem_ns as f64 / stim_dem_ns as f64),
        ]);
    }

    print!("{}", render_markdown_table(&rows));
}
```

**Step 4: Run tests and the example**

Run:

```bash
CARGO_HOME=/tmp/rstim-cargo-home cargo test -p rstim --test showcase_helpers
```

Expected: PASS.

Run:

```bash
CARGO_HOME=/tmp/rstim-cargo-home cargo run -p rstim --example stim_parity_showcase
```

Expected: PASS and prints a markdown table with six data rows.

**Step 5: Commit**

```bash
git add /Users/nzy/rcode/rstim/rstim/examples/stim_parity_showcase.rs /Users/nzy/rcode/rstim/rstim/src/showcase.rs /Users/nzy/rcode/rstim/rstim/tests/showcase_helpers.rs
git commit -m "feat: add showcase timing report example"
```

### Task 5: Run Full Verification and Capture Fresh Output

**Files:**
- Modify: `/Users/nzy/rcode/rstim/README.md` only if you want to add the final summary after measurements

**Step 1: Run the full correctness suite**

Run:

```bash
CARGO_HOME=/tmp/rstim-cargo-home cargo test -p rstim --test showcase_helpers --test stim_parity_showcase
```

Expected: PASS.

**Step 2: Run the reporting example and save the table**

Run:

```bash
CARGO_HOME=/tmp/rstim-cargo-home cargo run -p rstim --example stim_parity_showcase > /tmp/rstim-stim-showcase-table.md
```

Expected: PASS and `/tmp/rstim-stim-showcase-table.md` contains a markdown table.

**Step 3: Inspect the output before writing summary text**

Run:

```bash
sed -n '1,40p' /tmp/rstim-stim-showcase-table.md
```

Expected: header row plus six data rows.

**Step 4: Optionally add a short summary paragraph**

If updating the README or a PR description, use the fresh table output and keep the claim narrow:

```markdown
On six representative `repetition_code` and rotated `surface_code` cases (`d=5` and `d=13`), `rstim` matches `stim` on noiseless circuit generation structure and on noisy `analyze_errors` DEM semantics. The included showcase script reproduces the comparison and timing table locally.
```

**Step 5: Commit**

```bash
git add /Users/nzy/rcode/rstim/README.md
git commit -m "docs: summarize parity showcase results"
```
