# Issue 235 Random-Window Doc Workflow Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a user-facing CSS distance workflow document and a fast doc-contract test for `random-window-upper-bound`.

**Architecture:** Keep the durable user workflow in `qec-code/doc/css_distance.md`. Add one focused integration test in `qec-code/tests/cli.rs` that parses marked command blocks, runs only small documented examples, and checks issue #234 ladder commands as text only.

**Tech Stack:** Rust 2024, existing qec-code CLI binary integration helpers, `serde_json`, Markdown command blocks with HTML markers.

## Global Constraints

- User-facing document path is exactly `qec-code/doc/css_distance.md`.
- Doc-contract test name is exactly `random_window_upper_bound_doc_contract`.
- The document must contain `random-window-upper-bound`.
- The document must contain `randomized-upper-bound` as a baseline comparison.
- The document must contain `bound_type` and `upper`.
- The document must explicitly warn that an upper-bound result is not a certified exact distance.
- The documented built-in CLI example must parse and return completed JSON in the focused test.
- The documented file-input example must use existing sparse-row fixtures.
- The document must include these exact issue #234 commands without running the full ladder in this doc test:
  - `cargo test -p qec-code issue_225_random_window_upper_bound_smoke_ladder -- --nocapture`
  - `cargo test -p qec-code issue_225_random_window_upper_bound_full_ladder -- --ignored --nocapture`
- Keep the doc focused on user workflows and result interpretation. Avoid restating the full algorithm proof.
- Do not rewrite the full qec-code documentation set.
- Do not add benchmark plots or external baseline tables.
- Do not close #225 automatically.
- Verification command from issue #235 is `cargo test -p qec-code random_window_upper_bound_doc_contract -q`.

---

### Task 1: CSS Distance Workflow Doc Contract

**Files:**
- Create: `qec-code/doc/css_distance.md`
- Modify: `qec-code/tests/cli.rs`

**Interfaces:**
- Consumes: existing `qec_code_bin()`, `workspace_root()`, and `run_qec_code(...)` helpers in `qec-code/tests/cli.rs`.
- Produces: `random_window_upper_bound_doc_contract`, which parses `<!-- css_distance:random_window_builtin -->` and `<!-- css_distance:random_window_files -->` blocks from `qec-code/doc/css_distance.md`.

- [ ] **Step 1: Write the failing doc-contract test**

Add this block after `run_qec_code_in_process_os(...)` in `qec-code/tests/cli.rs`:

```rust
const CSS_DISTANCE_DOC: &str = include_str!("../doc/css_distance.md");
const CSS_DISTANCE_DOC_COMMAND_PREFIX: &str = "cargo run -q -p qec-code -- ";

#[derive(Debug)]
struct CssDistanceDocCommand {
    args: Vec<String>,
}

fn css_distance_doc_command_block(marker: &str) -> &str {
    let marker_text = format!("<!-- {marker} -->");
    let after_marker = CSS_DISTANCE_DOC
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

fn css_distance_doc_command(marker: &str) -> CssDistanceDocCommand {
    let commands: Vec<&str> = css_distance_doc_command_block(marker)
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with(CSS_DISTANCE_DOC_COMMAND_PREFIX))
        .collect();
    assert_eq!(commands.len(), 1, "marker {marker} should contain one qec-code command");

    let args = commands[0]
        .strip_prefix(CSS_DISTANCE_DOC_COMMAND_PREFIX)
        .unwrap()
        .split_whitespace()
        .map(str::to_owned)
        .collect();
    CssDistanceDocCommand { args }
}

fn materialize_css_distance_doc_args(command: &CssDistanceDocCommand) -> Vec<OsString> {
    command
        .args
        .iter()
        .map(|arg| {
            if arg.starts_with("qec-code/tests/fixtures/")
                || arg.starts_with("rsinter/tests/fixtures/")
            {
                workspace_root().join(arg).into_os_string()
            } else {
                OsString::from(arg)
            }
        })
        .collect()
}

fn run_css_distance_doc_command(marker: &str) -> Output {
    let command = css_distance_doc_command(marker);
    Command::new(qec_code_bin())
        .args(materialize_css_distance_doc_args(&command))
        .output()
        .expect("documented qec-code command should run")
}

fn assert_random_window_doc_json(marker: &str) -> serde_json::Value {
    let output = run_css_distance_doc_command(marker);
    assert!(
        output.status.success(),
        "documented command {marker} failed with stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stderr, b"");
    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid utf-8");
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout should be JSON");
    assert_eq!(json["status"], "completed");
    assert_eq!(json["method"], "random-window-upper-bound");
    assert_eq!(json["bound_type"], "upper");
    json
}
```

Add this test near the existing random-window CLI tests in `qec-code/tests/cli.rs`:

```rust
#[test]
fn random_window_upper_bound_doc_contract() {
    assert!(CSS_DISTANCE_DOC.contains("random-window-upper-bound"));
    assert!(CSS_DISTANCE_DOC.contains("randomized-upper-bound"));
    assert!(CSS_DISTANCE_DOC.contains("bound_type"));
    assert!(CSS_DISTANCE_DOC.contains("upper"));
    assert!(CSS_DISTANCE_DOC.contains(
        "When `bound_type: \"upper\"` appears in this JSON, the value is an upper bound from a randomized search, not a certified exact distance."
    ), "docs no longer warn users not to treat the randomized result as an exact distance");
    assert!(CSS_DISTANCE_DOC.contains(
        "cargo test -p qec-code issue_225_random_window_upper_bound_smoke_ladder -- --nocapture"
    ));
    assert!(CSS_DISTANCE_DOC.contains(
        "cargo test -p qec-code issue_225_random_window_upper_bound_full_ladder -- --ignored --nocapture"
    ));

    let built_in = assert_random_window_doc_json("css_distance:random_window_builtin");
    assert_eq!(built_in["upper_bound"], 3);
    assert_eq!(built_in["witness"]["weight"], 3);

    let file_block = css_distance_doc_command_block("css_distance:random_window_files");
    assert!(file_block.contains("qec-code/tests/fixtures/css/steane_hx.json"));
    assert!(file_block.contains("qec-code/tests/fixtures/css/steane_hz.json"));
    assert!(workspace_root()
        .join("qec-code/tests/fixtures/css/steane_hx.json")
        .exists());
    assert!(workspace_root()
        .join("qec-code/tests/fixtures/css/steane_hz.json")
        .exists());

    let files = assert_random_window_doc_json("css_distance:random_window_files");
    assert_eq!(files["upper_bound"], 3);
    assert_eq!(files["witness"]["weight"], 3);
}
```

- [ ] **Step 2: Run the focused test to verify RED**

Run:

```bash
cargo test -p qec-code random_window_upper_bound_doc_contract -q
```

Expected: FAIL at compile time because `qec-code/doc/css_distance.md` does not exist yet. The failure proves the doc-contract test is wired to the missing document.

- [ ] **Step 3: Add the CSS distance workflow document**

Create `qec-code/doc/css_distance.md` with this content:

```markdown
# CSS Distance Workflow

This note covers the user-facing CSS distance commands in `qec-code`. Run the
commands from the repository root.

Use `random-window-upper-bound` when you need a fast randomized CSS distance
upper-bound search with the current windowed sampler. It is useful for checking
known CSS instances, reproducing issue-225 ladder evidence, or comparing a new
code against a pinned upper-bound target.

Use `randomized-upper-bound` when you want the older simple sampler as a
baseline comparison. It remains available unchanged and is still useful as a
small negative-control style baseline, but it may return looser bounds on cases
where the windowed search succeeds.

## Built-In Code Example

The built-in path accepts any registered built-in CSS code ID.

<!-- css_distance:random_window_builtin -->
```bash
cargo run -q -p qec-code -- code css-distance random-window-upper-bound --code-id steane --iterations 500 --restarts 4 --seed 7 --target-weight 3 --json
```

The command should print one JSON object to stdout and nothing to stderr.

## Sparse-Row File Example

The file path accepts explicit `sparse_rows` `Hx` and `Hz` JSON matrices. This
example uses committed sparse-row fixtures.

<!-- css_distance:random_window_files -->
```bash
cargo run -q -p qec-code -- code css-distance random-window-upper-bound --hx qec-code/tests/fixtures/css/steane_hx.json --hz qec-code/tests/fixtures/css/steane_hz.json --iterations 500 --restarts 4 --seed 7 --target-weight 3 --json
```

Use the same shape with your own files:

```bash
cargo run -q -p qec-code -- code css-distance random-window-upper-bound --hx path/to/hx.json --hz path/to/hz.json --iterations 5000 --restarts 8 --seed 7 --target-weight 5 --json
```

## JSON Result Fields

A completed random-window run has this shape:

```json
{
  "status": "completed",
  "method": "random-window-upper-bound",
  "bound_type": "upper",
  "upper_bound": 3,
  "logical_class": "x_like",
  "witness": {
    "x": [1, 1, 1, 0, 0, 0, 0],
    "z": [0, 0, 0, 0, 0, 0, 0],
    "weight": 3
  },
  "options": {
    "iterations": 500,
    "restarts": 4,
    "seed": 7,
    "target_weight": 3
  },
  "provenance": {
    "tool": "qec-code",
    "tool_version": "0.1.0",
    "method_revision": 1
  }
}
```

- `status`: `completed` means the search returned a validated logical witness.
- `method`: identifies `random-window-upper-bound`; use it to distinguish the
  windowed search from `randomized-upper-bound`.
- `bound_type`: `upper` means the result is an upper bound.
- `upper_bound`: the returned witness weight.
- `logical_class`: the logical class of the witness, such as `x_like` or
  `z_like`.
- `witness`: the Pauli support and its `weight`.
- `options`: the effective randomized-search options.
- `provenance`: the emitting tool version and method revision.

When `bound_type: "upper"` appears in this JSON, the value is an upper bound
from a randomized search, not a certified exact distance. Treat `upper_bound` as
evidence that a logical operator of that weight was found; do not treat it as a
proof that no lower-weight logical operator exists.

## Issue-225 Ladder Evidence

Issue #234 added the issue-225 ladder evidence tests for the windowed method.
The smoke command is intended for normal local checks:

```bash
cargo test -p qec-code issue_225_random_window_upper_bound_smoke_ladder -- --nocapture
```

The full ladder is intentionally ignored by default. Run it explicitly when you
need the full acceptance evidence:

```bash
cargo test -p qec-code issue_225_random_window_upper_bound_full_ladder -- --ignored --nocapture
```

The old sampler remains available as a simple baseline. Issue #234 also keeps a
negative-control check showing that `randomized-upper-bound` is rejected by the
issue-225 ladder verifier on a known loose case:

```bash
cargo test -p qec-code issue_225_current_randomized_upper_bound_ladder_negative_control -q
```
```

- [ ] **Step 4: Run the focused test to verify GREEN**

Run:

```bash
cargo test -p qec-code random_window_upper_bound_doc_contract -q
```

Expected: PASS.

- [ ] **Step 5: Run formatting and full verification**

Run:

```bash
rustfmt --check qec-code/tests/cli.rs
cargo test -p qec-code random_window_upper_bound_doc_contract -q
cargo test
```

Expected: all commands exit 0. The full `cargo test` may emit pre-existing warnings, but no failures.

- [ ] **Step 6: Commit**

Run:

```bash
git add qec-code/doc/css_distance.md qec-code/tests/cli.rs docs/superpowers/plans/2026-06-26-issue-235-random-window-doc-workflow.md
git commit -m "docs: document random-window upper-bound workflow"
```

Expected: one commit containing the user-facing document, doc-contract test, and this implementation plan.

## Self-Review

- Spec coverage: the plan creates the requested doc, includes both CLI example types, warns about `bound_type: "upper"`, names the old baseline, includes the #234 commands, and adds the requested fast doc-contract test.
- Placeholder scan: no deferred-work markers remain.
- Type consistency: helper names and marker names are consistent across test and document steps.
