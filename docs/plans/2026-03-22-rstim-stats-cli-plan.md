# rstim Stats CLI Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add `rstim stats` with human-readable and JSON output, then document the existing and new CLI surface clearly.

**Architecture:** Extend `rstim::stats` with a serializable summary struct and a `summarize` entry point, then expose that through a new `stats` CLI subcommand in `rstim/src/cli.rs`. Keep output logic thin and stable: one flat summary struct, one plain-text formatter, one JSON serializer. Update docs in three layers: CLI reference, getting started workflow, and README discovery text.

**Tech Stack:** Rust, `clap`, `serde`, existing `rstim` parser/stats modules, workspace integration tests

---

### Task 1: Add summary API to `rstim::stats`

**Files:**
- Modify: `rstim/src/stats.rs`
- Modify: `rstim/tests/stats.rs`

**Step 1: Write the failing tests**

Add summary-focused tests to `rstim/tests/stats.rs`:

```rust
#[test]
fn summarize_empty_circuit() {
    let instrs = parse_lines("").unwrap();
    let summary = stats::summarize(&instrs);
    assert_eq!(summary.instruction_count, 0);
    assert_eq!(summary.repeat_blocks, 0);
    assert_eq!(summary.max_repeat_depth, 0);
    assert_eq!(summary.num_qubits, 0);
    assert_eq!(summary.num_measurements, 0);
    assert_eq!(summary.num_detectors, 0);
    assert_eq!(summary.num_observables, 0);
    assert_eq!(summary.num_ticks, 0);
    assert_eq!(summary.num_sweep_bits, 0);
}

#[test]
fn summarize_repeat_distinguishes_structure_from_expanded_counts() {
    let instrs = parse_lines("H 0\nREPEAT 3 {\n  M 0\n  DETECTOR rec[-1]\n  TICK\n}\n").unwrap();
    let summary = stats::summarize(&instrs);
    assert_eq!(summary.instruction_count, 4);
    assert_eq!(summary.repeat_blocks, 1);
    assert_eq!(summary.max_repeat_depth, 1);
    assert_eq!(summary.num_measurements, 3);
    assert_eq!(summary.num_detectors, 3);
    assert_eq!(summary.num_ticks, 3);
}

#[test]
fn summarize_nested_repeat_tracks_max_depth() {
    let instrs = parse_lines("REPEAT 2 {\n  REPEAT 5 {\n    M 0\n  }\n}\n").unwrap();
    let summary = stats::summarize(&instrs);
    assert_eq!(summary.repeat_blocks, 2);
    assert_eq!(summary.max_repeat_depth, 2);
    assert_eq!(summary.instruction_count, 3);
    assert_eq!(summary.num_measurements, 10);
}
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test -p rstim --test stats summarize_
```

Expected: FAIL because `summarize` and the summary type do not exist yet.

**Step 3: Write the minimal implementation**

In `rstim/src/stats.rs`:

- add `use serde::Serialize;`
- add:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CircuitStatsSummary {
    pub instruction_count: usize,
    pub repeat_blocks: usize,
    pub max_repeat_depth: usize,
    pub num_qubits: usize,
    pub num_measurements: usize,
    pub num_detectors: usize,
    pub num_observables: usize,
    pub num_ticks: usize,
    pub num_sweep_bits: usize,
}
```

- add:

```rust
pub fn summarize(instrs: &[StimInstr]) -> CircuitStatsSummary {
    let (instruction_count, repeat_blocks, max_repeat_depth) = summarize_structure(instrs, 0);
    CircuitStatsSummary {
        instruction_count,
        repeat_blocks,
        max_repeat_depth,
        num_qubits: num_qubits(instrs),
        num_measurements: num_measurements(instrs),
        num_detectors: num_detectors(instrs),
        num_observables: num_observables(instrs),
        num_ticks: num_ticks(instrs),
        num_sweep_bits: num_sweep_bits(instrs),
    }
}
```

- add a private helper:

```rust
fn summarize_structure(instrs: &[StimInstr], depth: usize) -> (usize, usize, usize) {
    let mut instruction_count = 0;
    let mut repeat_blocks = 0;
    let mut max_repeat_depth = depth;
    for instr in instrs {
        instruction_count += 1;
        if let StimInstr::Repeat { body, .. } = instr {
            repeat_blocks += 1;
            let (inner_instrs, inner_repeats, inner_depth) = summarize_structure(body, depth + 1);
            instruction_count += inner_instrs;
            repeat_blocks += inner_repeats;
            max_repeat_depth = max_repeat_depth.max(inner_depth);
        }
    }
    (instruction_count, repeat_blocks, max_repeat_depth)
}
```

Keep all existing `num_*` helpers unchanged.

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test -p rstim --test stats summarize_
```

Expected: PASS.

**Step 5: Commit**

```bash
git add rstim/src/stats.rs rstim/tests/stats.rs
git commit -m "feat: add rstim circuit stats summary"
```

### Task 2: Add `rstim stats` CLI with text and JSON output

**Files:**
- Modify: `rstim/src/cli.rs`
- Create: `rstim/tests/cli_stats.rs`

**Step 1: Write the failing tests**

Create `rstim/tests/cli_stats.rs` with focused CLI tests:

```rust
use std::io::Write;
use std::process::{Command, Stdio};

fn rstim_cmd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_rstim"))
}

fn run_with_stdin(args: &[&str], stdin_data: &str) -> std::process::Output {
    let mut child = rstim_cmd()
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(stdin_data.as_bytes()).unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn stats_text_output_from_stdin() {
    let output = run_with_stdin(
        &["stats"],
        "H 0\nREPEAT 2 {\n  M 0\n  DETECTOR rec[-1]\n  TICK\n}\n",
    );
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("instruction_count: 4"));
    assert!(text.contains("repeat_blocks: 1"));
    assert!(text.contains("max_repeat_depth: 1"));
    assert!(text.contains("num_measurements: 2"));
    assert!(text.contains("num_detectors: 2"));
    assert!(text.contains("num_ticks: 2"));
}

#[test]
fn stats_json_output_from_stdin() {
    let output = run_with_stdin(
        &["stats", "--json"],
        "CX sweep[3] 0\nOBSERVABLE_INCLUDE(2) rec[-1]\n",
    );
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["instruction_count"], 2);
    assert_eq!(value["repeat_blocks"], 0);
    assert_eq!(value["max_repeat_depth"], 0);
    assert_eq!(value["num_qubits"], 1);
    assert_eq!(value["num_measurements"], 0);
    assert_eq!(value["num_observables"], 3);
    assert_eq!(value["num_sweep_bits"], 4);
}

#[test]
fn stats_reads_from_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("input.stim");
    std::fs::write(&path, "M 0\nDETECTOR rec[-1]\n").unwrap();
    let output = rstim_cmd().args(["stats", "--in", path.to_str().unwrap()]).output().unwrap();
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("num_measurements: 1"));
    assert!(text.contains("num_detectors: 1"));
}

#[test]
fn stats_invalid_input_fails_cleanly() {
    let output = run_with_stdin(&["stats"], "DETECTOR rec[-1]");
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("rec"));
    assert!(!stderr.contains("panicked"));
}
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test -p rstim --test cli_stats
```

Expected: FAIL because the `stats` subcommand does not exist yet.

**Step 3: Write the minimal implementation**

In `rstim/src/cli.rs`:

- add a new subcommand:

```rust
/// Summarize circuit structure and counts
Stats {
    #[arg(long = "in")]
    r#in: Option<String>,
    #[arg(long)]
    out: Option<String>,
    #[arg(long)]
    json: bool,
},
```

- route it in `run(cli)`:

```rust
Some(Commands::Stats { r#in, out, json }) => {
    let text = read_input(r#in.as_deref())?;
    let mut w = open_output(out.as_deref())?;
    run_stats(&text, json, &mut w)
}
```

- add:

```rust
pub fn run_stats(text: &str, json: bool, out: &mut dyn Write) -> Result<(), String> {
    let instrs = parse_lines(text)?;
    let summary = crate::stats::summarize(&instrs);
    if json {
        serde_json::to_writer_pretty(&mut *out, &summary).map_err(|e| format!("write error: {e}"))?;
        out.write_all(b"\n").map_err(|e| format!("write error: {e}"))?;
        return Ok(());
    }
    writeln!(out, "instruction_count: {}", summary.instruction_count).map_err(|e| e.to_string())?;
    writeln!(out, "repeat_blocks: {}", summary.repeat_blocks).map_err(|e| e.to_string())?;
    writeln!(out, "max_repeat_depth: {}", summary.max_repeat_depth).map_err(|e| e.to_string())?;
    writeln!(out, "num_qubits: {}", summary.num_qubits).map_err(|e| e.to_string())?;
    writeln!(out, "num_measurements: {}", summary.num_measurements).map_err(|e| e.to_string())?;
    writeln!(out, "num_detectors: {}", summary.num_detectors).map_err(|e| e.to_string())?;
    writeln!(out, "num_observables: {}", summary.num_observables).map_err(|e| e.to_string())?;
    writeln!(out, "num_ticks: {}", summary.num_ticks).map_err(|e| e.to_string())?;
    writeln!(out, "num_sweep_bits: {}", summary.num_sweep_bits).map_err(|e| e.to_string())?;
    Ok(())
}
```

Keep JSON output flat and pretty-printed.

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test -p rstim --test cli_stats
```

Expected: PASS.

**Step 5: Commit**

```bash
git add rstim/src/cli.rs rstim/tests/cli_stats.rs
git commit -m "feat: add rstim stats command"
```

### Task 3: Document the CLI clearly

**Files:**
- Create: `rstim/doc/cli.md`
- Modify: `rstim/doc/getting_started.md`
- Modify: `README.md`

**Step 1: Write the failing doc checks**

There is no automated doc checker in the repo, so the "failing test" here is a
manual gap check. Confirm these cases are currently undocumented:

- no CLI reference file exists at `rstim/doc/cli.md`
- `README.md` does not mention `rstim stats`
- `rstim/doc/getting_started.md` does not show a CLI inspection workflow

Run:

```bash
test -f rstim/doc/cli.md && exit 1 || exit 0
rg -n "rstim stats" README.md rstim/doc/getting_started.md
```

Expected: `rstim/doc/cli.md` missing, and no relevant `rstim stats` references.

**Step 2: Write the documentation**

Create `rstim/doc/cli.md` with:

- one short section per command family
- a full `rstim stats` section
- examples for text and JSON output
- an explanation that structural metrics do not expand repeats, while execution
  counts do

Update `rstim/doc/getting_started.md` with a short CLI workflow:

1. inspect with `rstim stats`
2. then sample or analyze

Update `README.md` with a short discoverability section mentioning `rstim stats`
and linking readers toward the CLI/reference docs.

**Step 3: Verify the docs**

Run:

```bash
rg -n "rstim stats|CLI Reference|instruction_count|--json" README.md rstim/doc/getting_started.md rstim/doc/cli.md
```

Expected: matches in all three files, with `rstim/doc/cli.md` containing the
full reference.

**Step 4: Commit**

```bash
git add README.md rstim/doc/getting_started.md rstim/doc/cli.md
git commit -m "docs: document rstim cli and stats command"
```

### Task 4: Final verification

**Files:**
- Verify only

**Step 1: Run targeted verification**

Run:

```bash
cargo test -p rstim --test stats summarize_
cargo test -p rstim --test cli_stats
cargo test -p rstim --test cli_integration
```

Expected: PASS.

**Step 2: Run one manual CLI smoke test**

Run:

```bash
printf 'H 0\nREPEAT 2 {\n  M 0\n  DETECTOR rec[-1]\n  TICK\n}\n' | cargo run -p rstim -- stats
printf 'M 0\nDETECTOR rec[-1]\n' | cargo run -p rstim -- stats --json
```

Expected:

- text output contains the nine summary fields
- JSON output parses cleanly and includes `num_measurements` and `num_detectors`

**Step 3: Summarize**

Record:

- which tests ran
- whether worktree setup was blocked by environment permissions
- which files changed

No final success claim should be made without the test commands completing
cleanly.
