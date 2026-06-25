# Quantum Tanner CLI Workflow Documentation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add user-facing `qec-code` quantum Tanner CLI workflow documentation and keep its commands current with a regression test.

**Architecture:** Put the workflow in a short Markdown document next to the existing quantum Tanner construction contract. Add one integration test that extracts marked documented commands, translates the documented `cargo run -q -p qec-code --` prefix to the compiled test binary, and verifies the export, exact-distance, and invalid-spec paths.

**Tech Stack:** Rust 2024 integration tests, `serde_json`, `tempfile`, Markdown documentation, existing `qec-code` CLI binary.

## Global Constraints

- Workflow document path is exactly `qec-code/doc/quantum_tanner_cli.md`.
- Regression test name is exactly `quantum_tanner_cli_doc_commands_stay_current`.
- Verification command from issue #186 is `cargo test -p qec-code quantum_tanner_cli_doc_commands_stay_current -q`.
- The documented exact-distance command for `qec-code/tests/fixtures/quantum_tanner/toric_d4.json` must return distance `4`.
- The documented invalid-spec command must exit non-zero and must not produce valid `sparse_rows` or distance JSON.
- The document must state that Rust consumes explicit finite-group specs and does not search groups or call GAP/Oscar.
- Include the required reference paths and repositories: `drafts/qLDPC/src/qldpc/codes/quantum.py`, `drafts/qLDPC/src/qldpc/objects.py`, `drafts/qLDPC/src/qldpc/codes/quantum_test.py`, `https://github.com/qLDPCOrg/qLDPC`, `https://github.com/QuantumSavory/QuantumExpanders.jl`, and `https://github.com/RebKatRad/qTanner`.
- State that qLDPC is Apache-2.0 in the local clone, while the other repositories should be used according to their own licenses and may be reference-only unless a compatible license is confirmed.
- Do not add code-generation functionality, importer tooling, benchmark results, or `rsinter` integration.

---

## File Structure

- Create `qec-code/doc/quantum_tanner_cli.md`: user-facing CLI workflow with marked command blocks and provenance notes.
- Modify `qec-code/tests/quantum_tanner_cli.rs`: add the doc-backed command parser and regression test.

---

### Task 1: Quantum Tanner CLI Workflow Doc And Regression Test

**Files:**
- Create: `qec-code/doc/quantum_tanner_cli.md`
- Modify: `qec-code/tests/quantum_tanner_cli.rs`

**Interfaces:**
- Consumes: `CARGO_BIN_EXE_qec-code`, `workspace_root()`, `assert_quantum_tanner_sparse_rows_output`.
- Produces: Markdown markers `quantum_tanner_cli:toric_d4_commands` and `quantum_tanner_cli:invalid_spec_command`.
- Produces: test `quantum_tanner_cli_doc_commands_stay_current`.

- [ ] **Step 1: Write the failing doc-backed test**

Add these imports at the top of `qec-code/tests/quantum_tanner_cli.rs`:

```rust
use std::collections::HashMap;
use std::fs;
use tempfile::tempdir;
```

Replace the existing `use std::path::PathBuf;` and `use std::process::{Command, Output};`
block with the combined imports:

```rust
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};

use tempfile::tempdir;
```

Add the helper code below after `run_quantum_tanner`:

```rust
const QUANTUM_TANNER_CLI_DOC: &str = include_str!("../doc/quantum_tanner_cli.md");
const DOC_COMMAND_PREFIX: &str = "cargo run -q -p qec-code -- ";

#[derive(Debug)]
struct DocumentedCommand {
    args: Vec<String>,
    redirect: Option<String>,
}

fn documented_command_block(marker: &str) -> &str {
    let marker_text = format!("<!-- {marker} -->");
    let after_marker = QUANTUM_TANNER_CLI_DOC
        .split_once(&marker_text)
        .map(|(_, after)| after)
        .unwrap_or_else(|| panic!("missing doc marker {marker_text}"));
    let fence_start = after_marker
        .find("```bash")
        .unwrap_or_else(|| panic!("missing bash fence after {marker_text}"));
    let command_start = fence_start + "```bash".len();
    let command_tail = &after_marker[command_start..];
    let fence_end = command_tail
        .find("```")
        .unwrap_or_else(|| panic!("missing closing bash fence after {marker_text}"));
    &command_tail[..fence_end]
}

fn documented_qec_code_commands(marker: &str) -> Vec<DocumentedCommand> {
    documented_command_block(marker)
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with(DOC_COMMAND_PREFIX))
        .map(parse_documented_command)
        .collect()
}

fn parse_documented_command(line: &str) -> DocumentedCommand {
    let command = line
        .strip_prefix(DOC_COMMAND_PREFIX)
        .unwrap_or_else(|| panic!("documented command must start with {DOC_COMMAND_PREFIX}: {line}"));
    let (args_text, redirect) = match command.split_once(" > ") {
        Some((before_redirect, redirect)) => (before_redirect, Some(redirect.to_owned())),
        None => (command, None),
    };

    DocumentedCommand {
        args: args_text.split_whitespace().map(str::to_owned).collect(),
        redirect,
    }
}

fn workspace_path_arg(arg: &str, output_map: &HashMap<String, PathBuf>) -> String {
    output_map
        .get(arg)
        .cloned()
        .unwrap_or_else(|| workspace_root().join(arg))
        .display()
        .to_string()
}

fn materialize_doc_args(args: &[String], output_map: &HashMap<String, PathBuf>) -> Vec<String> {
    args.iter()
        .map(|arg| {
            if arg.starts_with("qec-code/tests/fixtures/") || output_map.contains_key(arg) {
                workspace_path_arg(arg, output_map)
            } else {
                arg.to_owned()
            }
        })
        .collect()
}

fn run_documented_command(
    command: &DocumentedCommand,
    output_map: &HashMap<String, PathBuf>,
) -> Output {
    let args = materialize_doc_args(&command.args, output_map);
    Command::new(qec_code_bin())
        .args(args)
        .output()
        .expect("documented qec-code command should run")
}

fn json_output(stdout: &[u8]) -> serde_json::Value {
    let stdout = String::from_utf8(stdout.to_vec()).expect("stdout should be valid utf-8");
    serde_json::from_str(&stdout).expect("stdout should be JSON")
}

fn is_sparse_rows_json(stdout: &[u8]) -> bool {
    serde_json::from_slice::<serde_json::Value>(stdout).is_ok_and(|json| {
        json["format"] == "sparse_rows" && json["rows"].is_array()
    })
}

fn is_distance_json(stdout: &[u8]) -> bool {
    serde_json::from_slice::<serde_json::Value>(stdout).is_ok_and(|json| {
        json["status"] == "completed" && json["distance"].is_number()
    })
}
```

Add this test at the end of `qec-code/tests/quantum_tanner_cli.rs`:

```rust
#[test]
fn quantum_tanner_cli_doc_commands_stay_current() {
    assert!(QUANTUM_TANNER_CLI_DOC.contains(
        "Rust consumes explicit finite-group specs"
    ));
    assert!(QUANTUM_TANNER_CLI_DOC.contains("does not search for groups"));
    assert!(QUANTUM_TANNER_CLI_DOC.contains("does not call GAP or Oscar"));
    assert!(QUANTUM_TANNER_CLI_DOC.contains("drafts/qLDPC/src/qldpc/codes/quantum.py"));
    assert!(QUANTUM_TANNER_CLI_DOC.contains("drafts/qLDPC/src/qldpc/objects.py"));
    assert!(QUANTUM_TANNER_CLI_DOC.contains("drafts/qLDPC/src/qldpc/codes/quantum_test.py"));
    assert!(QUANTUM_TANNER_CLI_DOC.contains("https://github.com/qLDPCOrg/qLDPC"));
    assert!(QUANTUM_TANNER_CLI_DOC.contains("https://github.com/QuantumSavory/QuantumExpanders.jl"));
    assert!(QUANTUM_TANNER_CLI_DOC.contains("https://github.com/RebKatRad/qTanner"));
    assert!(QUANTUM_TANNER_CLI_DOC.contains("Apache-2.0"));
    assert!(QUANTUM_TANNER_CLI_DOC.contains("reference-only unless a compatible license is confirmed"));

    let tempdir = tempdir().expect("temporary output directory should be created");
    let hx_path = tempdir.path().join("toric_d4_hx.json");
    let hz_path = tempdir.path().join("toric_d4_hz.json");
    let output_map = HashMap::from([
        (
            "target/qec-code-workflow/toric_d4_hx.json".to_owned(),
            hx_path.clone(),
        ),
        (
            "target/qec-code-workflow/toric_d4_hz.json".to_owned(),
            hz_path.clone(),
        ),
    ]);

    let commands = documented_qec_code_commands("quantum_tanner_cli:toric_d4_commands");
    assert!(!commands.is_empty(), "workflow doc should contain qec-code commands");

    for command in &commands {
        let output = run_documented_command(command, &output_map);
        assert!(
            output.status.success(),
            "documented command failed: {command:?}\nstderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stderr, b"");

        if let Some(redirect) = &command.redirect {
            let path = output_map
                .get(redirect)
                .unwrap_or_else(|| panic!("unexpected redirect path {redirect}"));
            fs::write(path, &output.stdout).expect("redirected output should be writable");
            assert_quantum_tanner_sparse_rows_output(&output.stdout);
        }

        if command.args.iter().any(|arg| arg == "--json") {
            let json = json_output(&output.stdout);
            assert_eq!(json["status"], "completed");
            assert_eq!(json["distance"], 4);
        }
    }

    assert!(hx_path.exists(), "documented hx export should write a file");
    assert!(hz_path.exists(), "documented hz export should write a file");

    let invalid_commands =
        documented_qec_code_commands("quantum_tanner_cli:invalid_spec_command");
    assert_eq!(invalid_commands.len(), 1);
    let output = run_documented_command(&invalid_commands[0], &HashMap::new());
    assert!(!output.status.success());
    assert!(!is_sparse_rows_json(&output.stdout));
    assert!(!is_distance_json(&output.stdout));
}
```

- [ ] **Step 2: Run the focused test to verify RED**

Run:

```bash
cargo test -p qec-code quantum_tanner_cli_doc_commands_stay_current -q
```

Expected: FAIL at compile time because `qec-code/doc/quantum_tanner_cli.md` does not exist yet. That proves the regression test is wired to the missing workflow document.

- [ ] **Step 3: Add the workflow document**

Create `qec-code/doc/quantum_tanner_cli.md` with this exact content:

```markdown
# Quantum Tanner CLI Workflow

This workflow starts from a committed quantum Tanner spec, exports the
constructed CSS checks as `sparse_rows`, and verifies the CSS distance.

Run the commands from the repository root. The example uses the committed
`toric_d4` fixture:

```text
qec-code/tests/fixtures/quantum_tanner/toric_d4.json
```

## Boundary

Rust consumes explicit finite-group specs. It validates and constructs from the
finite multiplication table, generator indices, and local GF(2) code matrices in
the spec. It does not search for groups, does not call GAP or Oscar, and does
not call qLDPC Python, Julia/Oscar, or other external construction code at
runtime.

The middle shape is intentional: use external tools or checked-in fixtures to
prepare explicit finite data, then hand that data to `qec-code` for deterministic
CSS matrix export and distance checks.

## Inspect The Fixture

The fixture records a `Z4 x Z4` no-cover left-right Cayley-complex example with
expected CSS metadata `n = 16`, `k = 2`, and expected distance `4`.

```bash
sed -n '1,80p' qec-code/tests/fixtures/quantum_tanner/toric_d4.json
```

## Export `Hx` And `Hz`

These commands write ordinary `sparse_rows` JSON matrices.

<!-- quantum_tanner_cli:toric_d4_commands -->
```bash
mkdir -p target/qec-code-workflow
cargo run -q -p qec-code -- code css quantum-tanner --spec qec-code/tests/fixtures/quantum_tanner/toric_d4.json hx > target/qec-code-workflow/toric_d4_hx.json
cargo run -q -p qec-code -- code css quantum-tanner --spec qec-code/tests/fixtures/quantum_tanner/toric_d4.json hz > target/qec-code-workflow/toric_d4_hz.json
cargo run -q -p qec-code -- code css-distance exact --hx target/qec-code-workflow/toric_d4_hx.json --hz target/qec-code-workflow/toric_d4_hz.json --json
cargo run -q -p qec-code -- code css-distance exact --quantum-tanner-spec qec-code/tests/fixtures/quantum_tanner/toric_d4.json --json
```

The final command should return JSON with:

```json
{
  "status": "completed",
  "distance": 4
}
```

The `--hx`/`--hz` command verifies the exported files. The
`--quantum-tanner-spec` command verifies the same code directly from the spec.

## Negative Control

This invalid fixture removes an inverse generator from `A`. It should exit
non-zero before emitting a valid matrix or distance result.

<!-- quantum_tanner_cli:invalid_spec_command -->
```bash
cargo run -q -p qec-code -- code css quantum-tanner --spec qec-code/tests/fixtures/quantum_tanner/invalid_non_symmetric_a.json hx
```

## References And Licenses

The quantum Tanner construction vocabulary and fixture expectations were checked
against these references:

- local qLDPC reference implementation:
  `drafts/qLDPC/src/qldpc/codes/quantum.py`
- local qLDPC Cayley-complex reference:
  `drafts/qLDPC/src/qldpc/objects.py`
- local qLDPC toric Tanner test:
  `drafts/qLDPC/src/qldpc/codes/quantum_test.py`
- upstream qLDPC: <https://github.com/qLDPCOrg/qLDPC>
- QuantumExpanders.jl:
  <https://github.com/QuantumSavory/QuantumExpanders.jl>
- qTanner data/code repository for future import ideas:
  <https://github.com/RebKatRad/qTanner>

The local qLDPC clone used as a reference is Apache-2.0. Use the other
repositories according to their own licenses; treat them as reference-only
unless compatible licensing is confirmed.
```

- [ ] **Step 4: Run the focused test to verify GREEN**

Run:

```bash
cargo test -p qec-code quantum_tanner_cli_doc_commands_stay_current -q
```

Expected: PASS. The test should execute the documented `toric_d4` commands, confirm `sparse_rows` exports, confirm exact distance `4`, and confirm the invalid-spec command exits non-zero without valid result JSON.

- [ ] **Step 5: Commit**

Run:

```bash
git add qec-code/doc/quantum_tanner_cli.md qec-code/tests/quantum_tanner_cli.rs
git commit -m "docs: add quantum tanner cli workflow"
```

Expected: commit succeeds with only the workflow doc and its regression test staged.

---

## Plan Self-Review

- Spec coverage: issue #186 workflow, reference, license, middle-shape, exact-distance, and negative-control requirements are covered by Task 1.
- Completeness scan: no incomplete markers remain.
- Type consistency: helper names and test names are consistent within the plan.
