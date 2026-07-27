# Issue 574 Family CLI Verifier Showcase Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `qec-code code css verify-families`, an offline in-process verifier that demonstrates the 12 supported and 2 deferred requested QEC families and documents the workflow.

**Architecture:** Add a focused production verifier module that reads the checked-in family manifest, constructs available-family positive fixtures through the existing CSS constructor path, compares computed metadata to the manifest, and formats a stable transcript. Wire the verifier into the existing `code css` CLI and teach the binary entry point to write verifier failure reports to stdout while exiting nonzero. Keep the showcase update tied to `tools/check_showcase_docs.py`.

**Tech Stack:** Rust 2024, clap, serde/serde_json, existing `qec-code` family contract APIs, Rust integration tests, Python showcase checker.

## Global Constraints

- The command is exactly `qec-code code css verify-families`.
- The success summary is exactly `SUMMARY PASS supported=12 deferred=2 failed=0`.
- Output must have 12 `PASS` lines, 2 `DEFERRED` lines, and one summary line.
- Every `PASS` line includes family ID, normalized parameters, `n`, check counts, ranks, `k`, row-weight summary, orthogonality result, and provenance identifier.
- The verifier calls constructors and validators in-process; it does not shell out or access the network.
- Any fixture mismatch, construction error, orthogonality error, or unsupported-status mismatch yields a `FAIL` line and nonzero exit status.
- Output ordering follows the manifest and is byte-for-byte stable.
- Deferred families are exactly `hyperbolic_5_5` and `perturbed_hgp`, and their lines include tracking issue and contract path.
- A supported target with `disposition=supported, availability=planned` prints `FAIL <family_id> disposition=supported availability=planned expected=available`.
- Mutating the generalized-bicycle fixture expected `rank_x` from 4 to 5 prints a deterministic `FAIL generalized_bicycle` line containing `expected rank_x=5 actual rank_x=4`, reports one failure, and exits nonzero in CLI mode.
- Existing CSS construction CLI behavior remains compatible.
- The showcase command transcript is checked by `python3 tools/check_showcase_docs.py docs/showcases/qec-code-css-construction.md`.

---

### Task 1: Add Failing Family CLI Tests

**Files:**
- Create: `qec-code/tests/family_cli.rs`

**Interfaces:**
- Consumes: existing binary helper pattern from `qec-code/tests/cli.rs`, manifest fixture text, and the planned `qec_code::family_verifier::verify_family_manifest_text`.
- Produces: the three issue-required exact tests.

- [ ] **Step 1: Write the failing integration test**

Create `qec-code/tests/family_cli.rs` with this structure:

```rust
use std::process::{Command, Output};

use qec_code::family_verifier::verify_family_manifest_text;

const MANIFEST_TEXT: &str = include_str!("fixtures/family_manifest/manifest.v1.json");

fn qec_code_bin() -> &'static str {
    env!("CARGO_BIN_EXE_qec-code")
}

fn run_qec_code(args: &[&str]) -> Output {
    Command::new(qec_code_bin())
        .args(args)
        .output()
        .expect("qec-code binary should run")
}

fn line_count(stdout: &str, prefix: &str) -> usize {
    stdout.lines().filter(|line| line.starts_with(prefix)).count()
}

fn family_ids(stdout: &str) -> Vec<&str> {
    stdout
        .lines()
        .filter(|line| {
            line.starts_with("PASS ")
                || line.starts_with("DEFERRED ")
                || line.starts_with("FAIL ")
        })
        .map(|line| line.split_whitespace().nth(1).unwrap())
        .collect()
}

fn mutate_generalized_bicycle(
    mutate: impl FnOnce(&mut serde_json::Value),
) -> String {
    let mut value: serde_json::Value = serde_json::from_str(MANIFEST_TEXT).unwrap();
    let family = value["families"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|entry| entry["family_id"] == "generalized_bicycle")
        .unwrap();
    mutate(family);
    format!("{}\n", serde_json::to_string_pretty(&value).unwrap())
}

#[test]
fn verify_families_cli_reports_12_pass_and_2_deferred() {
    let output = run_qec_code(&["code", "css", "verify-families"]);

    assert!(output.status.success());
    assert_eq!(output.stderr, b"");

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(line_count(&stdout, "PASS "), 12);
    assert_eq!(line_count(&stdout, "DEFERRED "), 2);
    assert_eq!(line_count(&stdout, "FAIL "), 0);
    assert!(stdout.ends_with("SUMMARY PASS supported=12 deferred=2 failed=0\n"));
    assert_eq!(
        family_ids(&stdout),
        vec![
            "directional",
            "quantum_tanner",
            "generalized_bicycle",
            "la_cross",
            "random_hgp",
            "lifted_product",
            "hyperbolic_5_5",
            "coprime_bb",
            "toric_3d",
            "color_666",
            "surface",
            "shor_like",
            "random_two_block",
            "perturbed_hgp",
        ]
    );

    for line in stdout.lines().filter(|line| line.starts_with("PASS ")) {
        for required in [
            " params=",
            " n=",
            " checks=h_x:",
            " ranks=rank_x:",
            " k=",
            " row_weights=h_x:",
            " orthogonal=true",
            " provenance=",
        ] {
            assert!(line.contains(required), "line missing {required:?}: {line}");
        }
    }
    assert!(stdout.contains(
        "DEFERRED hyperbolic_5_5 tracking_issue=#571 contract=qec-code/doc/hyperbolic_5_5_contract.md"
    ));
    assert!(stdout.contains(
        "DEFERRED perturbed_hgp tracking_issue=#572 contract=qec-code/doc/perturbed_hgp_contract.md"
    ));
}

#[test]
fn verify_families_cli_fails_on_mutated_rank() {
    let text = mutate_generalized_bicycle(|family| {
        family["expected"]["rank_x"] = serde_json::json!(5);
    });

    let report = verify_family_manifest_text(&text);

    assert_eq!(report.failed, 1);
    assert!(report.output.contains(
        "FAIL generalized_bicycle expected rank_x=5 actual rank_x=4"
    ));
    assert!(report
        .output
        .ends_with("SUMMARY FAIL supported=12 deferred=2 failed=1"));
}

#[test]
fn verify_families_cli_fails_when_supported_target_is_planned() {
    let text = mutate_generalized_bicycle(|family| {
        family["availability"] = serde_json::json!("planned");
    });

    let report = verify_family_manifest_text(&text);

    assert_eq!(report.failed, 1);
    assert!(report.output.contains(
        "FAIL generalized_bicycle disposition=supported availability=planned expected=available"
    ));
    assert!(report
        .output
        .ends_with("SUMMARY FAIL supported=12 deferred=2 failed=1"));
}
```

- [ ] **Step 2: Run the focused tests to verify RED**

Run:

```bash
cargo test -p qec-code --test family_cli verify_families_cli_reports_12_pass_and_2_deferred -- --exact
cargo test -p qec-code --test family_cli verify_families_cli_fails_on_mutated_rank -- --exact
cargo test -p qec-code --test family_cli verify_families_cli_fails_when_supported_target_is_planned -- --exact
```

Expected: FAIL to compile because `qec_code::family_verifier` and the CLI
subcommand do not exist yet.

- [ ] **Step 3: Commit Task 1**

```bash
git add qec-code/tests/family_cli.rs
git commit -m "test(qec-code): add family verifier CLI coverage"
```

### Task 2: Implement The In-Process Verifier And CLI Route

**Files:**
- Create: `qec-code/src/family_verifier.rs`
- Modify: `qec-code/src/lib.rs`
- Modify: `qec-code/src/cli.rs`
- Modify: `qec-code/src/error.rs`
- Modify: `qec-code/src/main.rs`

**Interfaces:**
- Consumes: `parse_css_construction_json`, `construct_css`, `verify_css_orthogonality`, `RequestedFamilyId`, and the checked-in manifest fixture.
- Produces: `verify_checked_in_family_manifest`, `verify_family_manifest_text`, `FamilyVerificationReport`, `CssCommands::VerifyFamilies`, and stdout-on-failure behavior for `QecError::FamilyVerificationFailed`.

- [ ] **Step 1: Create verifier module with public entry points**

Add `qec-code/src/family_verifier.rs` with these public definitions and local schema types:

```rust
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use serde::Deserialize;
use serde_json::Value;

use crate::error::{CssMatrixReadSource, QecError};
use crate::family_contract::{
    construct_css, parse_css_construction_json, verify_css_orthogonality,
    CssCodeStats, CssConstructionResult, RequestedFamilyId,
};

const MANIFEST_REL_PATH: &str = "tests/fixtures/family_manifest/manifest.v1.json";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FamilyVerificationReport {
    pub output: String,
    pub failed: usize,
}

pub fn verify_checked_in_family_manifest() -> Result<FamilyVerificationReport, QecError> {
    let path = checked_in_manifest_path();
    let text = fs::read_to_string(&path).map_err(|error| QecError::CssMatrixReadFailed {
        path: path.display().to_string(),
        source: CssMatrixReadSource(error.to_string()),
    })?;
    Ok(verify_family_manifest_text(&text))
}

pub fn verify_family_manifest_text(text: &str) -> FamilyVerificationReport {
    match serde_json::from_str::<FamilyCatalog>(text) {
        Ok(catalog) => verify_catalog(&catalog),
        Err(error) => failure_report(format!("FAIL manifest invalid_json={error}"), 0, 0),
    }
}

fn checked_in_manifest_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(MANIFEST_REL_PATH)
}
```

Define `FamilyCatalog`, `FamilyCatalogEntry`, `ExpectedStats`,
`RowWeightSummary`, `RowWeightBucket`, `CallableConstructorRef`, and
`ExecutableCase` as private `Deserialize` structs mirroring the manifest fields
the verifier reads. Use `#[serde(rename_all = "snake_case")]` enums for
`FamilyDisposition`, `RuntimeAvailability`, `ExecutableCaseKind`, and
`ExpectedOutcome`.

- [ ] **Step 2: Implement deterministic verification and formatting**

Implement helpers with these exact behaviors:

```rust
fn verify_catalog(catalog: &FamilyCatalog) -> FamilyVerificationReport {
    let supported = catalog
        .families
        .iter()
        .filter(|entry| entry.disposition == FamilyDisposition::Supported)
        .count();
    let deferred = catalog
        .families
        .iter()
        .filter(|entry| entry.disposition == FamilyDisposition::Deferred)
        .count();
    let mut lines = Vec::with_capacity(catalog.families.len() + 1);
    let mut failed = 0usize;

    for entry in &catalog.families {
        match verify_entry(entry) {
            EntryVerification::Line(line) => lines.push(line),
            EntryVerification::Failure(line) => {
                failed += 1;
                lines.push(line);
            }
        }
    }

    let status = if failed == 0 { "PASS" } else { "FAIL" };
    lines.push(format!(
        "SUMMARY {status} supported={supported} deferred={deferred} failed={failed}"
    ));
    FamilyVerificationReport {
        output: lines.join("\n"),
        failed,
    }
}
```

For supported entries, check `availability == Available` before parsing cases.
For available entries, find the first positive success case, serialize its
request, parse with `parse_css_construction_json`, construct with
`construct_css`, call `verify_css_orthogonality`, compare requested family,
stats, row weights, and provenance source, then format the `PASS` line.

For deferred entries, require exactly one `research_contracts` path and format
with hard-coded tracking issue `#571` for `hyperbolic_5_5` and `#572` for
`perturbed_hgp`. Unknown deferred family IDs fail deterministically.

Format the first stat mismatch as:

```rust
format!(
    "FAIL {} expected {}={} actual {}={}",
    entry.family_id, field, expected, field, actual
)
```

This produces the required generalized-bicycle rank negative control.

Format row weights with sorted buckets:

```rust
fn row_weight_summary(rows: &[Vec<usize>]) -> Vec<RowWeightBucket> {
    let mut counts = BTreeMap::new();
    for row in rows {
        *counts.entry(row.len()).or_insert(0usize) += 1;
    }
    counts
        .into_iter()
        .map(|(weight, count)| RowWeightBucket { weight, count })
        .collect()
}
```

- [ ] **Step 3: Wire the module into library and CLI**

In `qec-code/src/lib.rs`, add:

```rust
pub mod family_verifier;
```

In `qec-code/src/cli.rs`, import the verifier and add the subcommand:

```rust
use crate::family_verifier::verify_checked_in_family_manifest;

pub enum CssCommands {
    List,
    VerifyFamilies,
    Export {
        code_id: String,
        matrix: CssMatrixKind,
    },
    Construct {
        #[arg(long)]
        spec: PathBuf,
        output: CssConstructionOutput,
    },
    QuantumTanner {
        #[arg(long)]
        spec: PathBuf,
        matrix: CssMatrixKind,
    },
}
```

Handle it in `run_css_args`:

```rust
Some(CssCommands::VerifyFamilies) => run_css_verify_families(),
```

Add:

```rust
fn run_css_verify_families() -> Result<String, QecError> {
    let report = verify_checked_in_family_manifest()?;
    if report.failed == 0 {
        Ok(report.output)
    } else {
        Err(QecError::FamilyVerificationFailed {
            report: report.output,
        })
    }
}
```

- [ ] **Step 4: Add stdout-on-failure error variant**

In `qec-code/src/error.rs`, add:

```rust
#[error("family verification failed")]
FamilyVerificationFailed { report: String },
```

In `qec-code/src/main.rs`, change `write_result` to pass stdout into the error
writer:

```rust
fn write_result(
    result: Result<String, QecError>,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> i32 {
    match result {
        Ok(output) => write_success(stdout, &output),
        Err(error) => write_error(stdout, stderr, &error),
    }
}

fn write_error(stdout: &mut impl Write, stderr: &mut impl Write, error: &QecError) -> i32 {
    match error {
        QecError::FamilyVerificationFailed { report } => {
            writeln!(stdout, "{report}").expect("stdout write should succeed");
        }
        _ => {
            writeln!(stderr, "{error}").expect("stderr write should succeed");
        }
    }
    1
}
```

- [ ] **Step 5: Run focused tests to verify GREEN**

Run:

```bash
cargo test -p qec-code --test family_cli verify_families_cli_reports_12_pass_and_2_deferred -- --exact
cargo test -p qec-code --test family_cli verify_families_cli_fails_on_mutated_rank -- --exact
cargo test -p qec-code --test family_cli verify_families_cli_fails_when_supported_target_is_planned -- --exact
cargo run -p qec-code -- code css verify-families
```

Expected: PASS for the three tests, and the command prints 12 `PASS` lines, 2
`DEFERRED` lines, and `SUMMARY PASS supported=12 deferred=2 failed=0`.

- [ ] **Step 6: Commit Task 2**

```bash
git add qec-code/src/family_verifier.rs qec-code/src/lib.rs qec-code/src/cli.rs qec-code/src/error.rs qec-code/src/main.rs qec-code/tests/family_cli.rs
git commit -m "feat(qec-code): add CSS family verifier CLI"
```

### Task 3: Update CSS Showcase Documentation

**Files:**
- Modify: `docs/showcases/qec-code-css-construction.md`

**Interfaces:**
- Consumes: Task 2 CLI output and existing showcase checker.
- Produces: documentation that shows the new verifier, representative Rust and CLI usage, output interpretation, deferred boundaries, and fixture-addition guidance.

- [ ] **Step 1: Add verifier command to the Run It block**

Add this command to the existing shell block:

```sh
cargo run -q -p qec-code -- code css verify-families
```

- [ ] **Step 2: Add a requested-family verifier section**

Insert a `## Family Verifier` section before `## Construction Routing` with:

```markdown
## Family Verifier

`code css verify-families` is the offline end-to-end catalog check for the 14
requested CSS families. It reads
`qec-code/tests/fixtures/family_manifest/manifest.v1.json`, constructs the
positive fixture for each available family in-process, validates the metadata,
and prints one stable line per family in manifest order.

The success transcript ends with:

```text
SUMMARY PASS supported=12 deferred=2 failed=0
```

A `PASS` line means the manifest entry is `disposition=supported`,
`availability=available`, its positive fixture parsed and constructed through
`construct_css`, its dimensions, ranks, row weights, requested-family ID,
orthogonality, and provenance matched the fixture, and no subprocess or network
was used.

`DEFERRED` is intentional for `hyperbolic_5_5` and `perturbed_hgp`. Those lines
include the tracking issue and the research contract path, and they do not imply
a callable constructor.
```

- [ ] **Step 3: Add representative Rust and CLI usage**

Add concise examples that use the existing construction APIs:

```markdown
Parameterized Rust usage stays on the typed constructor path:

```rust
use qec_code::family_contract::{
    construct_css, CssFamilySpec, SurfaceFamilySpec,
};

let result = construct_css(
    CssFamilySpec::Surface(SurfaceFamilySpec { distance: 3 }).into(),
)?;
assert_eq!(result.stats.n, 9);
```

Parameterized CLI usage stays on the existing export path:

```sh
cargo run -q -p qec-code -- code css export surface_rotated:d=3 hx
cargo run -q -p qec-code -- code css export color_666:d=5 hz
cargo run -q -p qec-code -- code css export toric_3d:lx=3,ly=3,lz=3 hx
```
```

- [ ] **Step 4: Add safe fixture-addition guidance**

Add text explaining that a new supported fixture must update one manifest entry
with normalized inputs, expected stats, row weights, distance-verification
class, provenance, positive and negative executable cases, then run
`code css verify-families`, the `family_catalog` tests, and the showcase checker.

- [ ] **Step 5: Run the showcase checker**

Run:

```bash
python3 tools/check_showcase_docs.py docs/showcases/qec-code-css-construction.md
```

Expected: PASS.

- [ ] **Step 6: Commit Task 3**

```bash
git add docs/showcases/qec-code-css-construction.md
git commit -m "docs(qec-code): showcase CSS family verifier"
```

### Task 4: Final Verification, Review, And Pull Request

**Files:**
- No planned source edits.

**Interfaces:**
- Consumes: committed Tasks 1-3.
- Produces: verified worker branch and a pull request against `master`.

- [ ] **Step 1: Run required focused verification**

Run:

```bash
cargo test -p qec-code --test family_cli verify_families_cli_reports_12_pass_and_2_deferred -- --exact
cargo test -p qec-code --test family_cli verify_families_cli_fails_on_mutated_rank -- --exact
cargo test -p qec-code --test family_cli verify_families_cli_fails_when_supported_target_is_planned -- --exact
cargo run -p qec-code -- code css verify-families
python3 tools/check_showcase_docs.py docs/showcases/qec-code-css-construction.md
```

Expected: all commands pass; the cargo run transcript ends with
`SUMMARY PASS supported=12 deferred=2 failed=0`.

- [ ] **Step 2: Run full workspace verification**

Run:

```bash
cargo test
```

Expected: PASS.

- [ ] **Step 3: Run final code review**

Dispatch a Superpowers code-review subagent using the branch merge-base with
`master` and `HEAD`. Include the issue body and this plan as requirements.
Fix Critical and Important findings, then rerun the covering tests.

- [ ] **Step 4: Finish branch with PR option**

Use `superpowers:finishing-a-development-branch`, choose `Push and create a
Pull Request`, push the branch, and open a PR against `master` that closes
issue #574.

