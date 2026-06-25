# Issue 174 Built-In SVG Renderer Documentation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Document `rstim render_svg` as the primary built-in static circuit visualization workflow and add a CLI documentation-tracking test.

**Architecture:** Keep the implementation documentation-only plus one integration test. Update the README, CLI reference, and optional Typst package README so `render_svg` is the user-facing SVG path while `export_json` remains the structured QP101 data export path.

**Tech Stack:** Markdown documentation, Rust 2024 integration tests, existing `rstim` CLI binary test helpers, `tempfile`.

## Global Constraints

- Document `render_svg` in `README.md` as the primary static circuit visualization path.
- Document `render_svg` in `rstim/doc/cli.md` with one plain file-output command, stdin/stdout behavior, one seeded sample-shot command, one DEM-highlight command, and the documented `--seed` without `--sample_shot` error behavior.
- Keep `export_json` documented as QP101 structured-data export for downstream processing, fixture generation, and the optional legacy/prototype Typst path.
- Update `qp101-viz/README.md` to describe Typst as optional legacy/prototype infrastructure now that the committed renderer examples are covered by the built-in CLI.
- Add a documentation-tracking CLI test named `render_svg_documented_workflow_matches_cli`.
- Include a negative control in that test that rejects stale command spelling such as `rstim svg_render`.
- Avoid promising coordinate-layout rendering or interactive browser editing.
- Do not change CLI behavior.
- Do not change QP101 JSON output.
- Do not switch the QP101 gallery or website build.
- Do not remove `qp101-viz/`.
- Do not write a long visualization tutorial beyond README and CLI reference updates.
- Verification command required by issue #174: `cargo test -p rstim --test cli_render_svg render_svg_documented_workflow_matches_cli -q`.

---

### Task 1: Documentation Updates And Documentation-Tracking Test

**Files:**
- Modify: `README.md`
- Modify: `rstim/doc/cli.md`
- Modify: `qp101-viz/README.md`
- Modify: `rstim/tests/cli_render_svg.rs`

**Interfaces:**
- Consumes: existing `rstim_cmd() -> Command` helper in `rstim/tests/cli_render_svg.rs`.
- Consumes: existing `run_render_svg_with_stdin_args(args: &[&str], stdin_data: &str) -> std::process::Output` helper in `rstim/tests/cli_render_svg.rs`.
- Produces: documentation examples for `rstim render_svg --in circuit.stim --out circuit.svg`, `rstim render_svg --sample_shot --seed 7 ...`, and `rstim render_svg --highlight_dem_error 0 ...`.
- Produces: test `render_svg_documented_workflow_matches_cli`.

- [ ] **Step 1: Write the failing documentation-tracking test**

Append this test to `rstim/tests/cli_render_svg.rs` after the helper functions and before or after the existing tests:

```rust
#[test]
fn render_svg_documented_workflow_matches_cli() {
    let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("rstim crate should live under repository root");
    let read_doc = |path: &str| -> String {
        std::fs::read_to_string(repo_root.join(path))
            .unwrap_or_else(|err| panic!("failed to read {path}: {err}"))
    };
    let readme = read_doc("README.md");
    let cli_doc = read_doc("rstim/doc/cli.md");

    for (name, doc) in [("README.md", &readme), ("rstim/doc/cli.md", &cli_doc)] {
        assert!(doc.contains("render_svg"), "{name} should document render_svg");
        assert!(
            doc.contains("export_json"),
            "{name} should still document export_json for QP101 data export"
        );
        assert!(
            !doc.contains("rstim svg_render"),
            "{name} should not contain stale svg_render command spelling"
        );
    }

    for required in [
        "rstim render_svg --in circuit.stim --out circuit.svg",
        "--sample_shot --seed 7",
        "--highlight_dem_error 0",
        "--seed is only supported with --sample_shot",
    ] {
        assert!(
            cli_doc.contains(required),
            "CLI docs missing documented render_svg workflow marker {required}"
        );
    }

    let circuit = "H 0\nCX 0 1\nTICK\nM 0\n";
    let input = tempfile::NamedTempFile::new().unwrap();
    let output = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(input.path(), circuit).unwrap();

    let plain_output = rstim_cmd()
        .arg("render_svg")
        .arg("--in")
        .arg(input.path())
        .arg("--out")
        .arg(output.path())
        .output()
        .unwrap();
    assert!(
        plain_output.status.success(),
        "documented plain render command should succeed, stderr: {}",
        String::from_utf8_lossy(&plain_output.stderr)
    );
    assert!(
        plain_output.stdout.is_empty(),
        "documented file-output render should not write stdout: {}",
        String::from_utf8_lossy(&plain_output.stdout)
    );
    let svg = std::fs::read_to_string(output.path()).unwrap();
    assert!(svg.starts_with("<svg"), "documented command produced non-SVG: {svg}");
    for marker in ["q0", "H", "M"] {
        assert!(
            svg.contains(marker),
            "documented command SVG missing marker {marker}: {svg}"
        );
    }

    let protected_output = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(protected_output.path(), "existing svg should remain").unwrap();
    let bad_output = run_render_svg_with_stdin_args(
        &[
            "--seed",
            "7",
            "--out",
            protected_output.path().to_str().unwrap(),
        ],
        "M 0\n",
    );
    assert!(
        !bad_output.status.success(),
        "documented --seed without --sample_shot failure should fail"
    );
    let stderr = String::from_utf8_lossy(&bad_output.stderr);
    assert!(
        stderr.contains("--seed is only supported with --sample_shot"),
        "stderr should match documented seed compatibility error: {stderr}"
    );
    let protected_text = std::fs::read_to_string(protected_output.path()).unwrap();
    assert_eq!(protected_text, "existing svg should remain");
}
```

- [ ] **Step 2: Run the focused test to verify RED**

Run:

```sh
cargo test -p rstim --test cli_render_svg render_svg_documented_workflow_matches_cli -q
```

Expected: FAIL because the docs do not yet contain all required `render_svg`
workflow markers.

If the environment cannot reach the crate index, repeat the same focused test
with offline cargo:

```sh
CARGO_NET_OFFLINE=true cargo test -p rstim --test cli_render_svg render_svg_documented_workflow_matches_cli -q
```

Expected: FAIL for the same missing-doc marker, not for dependency resolution.

- [ ] **Step 3: Update the README workspace map and common workflow bullets**

In `README.md`, change the `qp101-viz/` workspace map row to:

```markdown
| `qp101-viz/` | Optional legacy/prototype Typst renderer for QP101 circuit JSON |
```

In the "Common Workflows" list under "Inspect And Sample A Circuit", change the final bullet group from:

```markdown
- `export_json` for QP101 export
```

to:

```markdown
- `render_svg` for built-in static SVG circuit diagrams
- `export_json` for QP101 structured-data export
```

- [ ] **Step 4: Update the README visualization section**

Replace the section header `## Atom Loss And QP101 Export` and its first
render/export paragraphs with this content, preserving the existing example
Stim circuit, mixed-noise showcase command, and related-file links after the new
examples:

````markdown
## Static SVG Diagrams And Atom Loss Overlays

For static circuit visualization, use the built-in SVG renderer first:

```sh
rstim render_svg --in circuit.stim --out circuit.svg
```

Omit `--out` to write the SVG document to stdout, which is useful for pipes and
quick checks:

```sh
printf 'H 0\nCX 0 1\nTICK\nM 0\n' | rstim render_svg > circuit.svg
```

Atom loss is a first-class workflow in `rstim`. The simulator can model
explicit `LOSS` events, propagate loss through later operations, and annotate
loss-caused measurement outcomes in seeded sample-shot SVGs.
````

After the existing example Stim circuit block, replace the old
`Export one seeded sample shot as QP101 JSON:` block with:

````markdown
Render one seeded sample shot with atom-loss and detector-flip overlays:

```sh
rstim render_svg --sample_shot --seed 7 \
  --in qp101-viz/examples/atom-loss-sample.stim \
  --out atom-loss-sample.svg
```

Render a selected detector-error-model error as source and symptom highlights:

```sh
printf 'X_ERROR(0.1) 0\nM 0\nDETECTOR rec[-1]\n' > /tmp/rstim-dem-highlight.stim
rstim render_svg --highlight_dem_error 0 \
  --in /tmp/rstim-dem-highlight.stim \
  --out dem-highlight.svg
```

Use `export_json` when you need QP101 structured data for downstream tooling,
fixture generation, or the optional legacy/prototype Typst workflow:

```sh
rstim export_json --sample_shot --seed 7 \
  < qp101-viz/examples/atom-loss-sample.stim
```
````

- [ ] **Step 5: Update the CLI command family list**

In `rstim/doc/cli.md`, change:

```markdown
- generation and export: `gen`, `export_json`
```

to:

```markdown
- generation and export: `gen`, `render_svg`, `export_json`
```

- [ ] **Step 6: Add the CLI `render_svg` reference section**

In `rstim/doc/cli.md`, insert this section immediately before
`## Export QP101 JSON with export_json`:

````markdown
## Render SVG diagrams with `render_svg`

`render_svg` is the primary static circuit visualization path. It parses a
Stim-like circuit, builds the repository's QP101 document internally, and emits
an SVG diagram without requiring Typst:

```sh
rstim render_svg --in circuit.stim --out circuit.svg
```

The command follows the common CLI I/O convention. `--in <path>` reads a circuit
from a file; omitting `--in` reads from stdin. `--out <path>` writes the SVG to a
file; omitting `--out` writes SVG to stdout:

```sh
printf 'H 0\nCX 0 1\nTICK\nM 0\n' | rstim render_svg > circuit.svg
```

For seeded sample-shot overlays, pass `--sample_shot` and an optional
deterministic seed:

```sh
rstim render_svg --sample_shot --seed 7 --in circuit.stim --out sample.svg
```

The sample-shot SVG includes visible QP101 annotations for supported sampled
events such as fired noise branches, loss-caused measurement information,
measurement outcomes, and detector flips. `--seed` is only supported with
`--sample_shot`; running `rstim render_svg --seed 7` without `--sample_shot`
fails with `--seed is only supported with --sample_shot`.

For detector-error-model debugging, render one DEM error term as source and
symptom highlights:

```sh
rstim render_svg --highlight_dem_error 0 --in circuit.stim --out highlight.svg
```

`--sample_shot` and `--highlight_dem_error` are mutually exclusive. Use one
overlay mode per render.
````

- [ ] **Step 7: Update the CLI `export_json` and suggested workflow text**

In `rstim/doc/cli.md`, replace:

```markdown
This is useful for external visualization or structured downstream processing.
```

with:

```markdown
Use `export_json` when you need QP101 structured data for downstream
processing, fixture generation, or the optional legacy/prototype Typst
`qp101-viz` workflow. For ordinary static SVG diagrams, prefer `render_svg`.
```

In the "Suggested Workflow" list, replace:

```markdown
5. `rstim export_json` when handing the circuit to structured tooling
```

with:

```markdown
5. `rstim render_svg` when you want a static SVG circuit diagram
6. `rstim export_json` when handing QP101 data to structured tooling
```

- [ ] **Step 8: Reframe `qp101-viz` as optional prototype infrastructure**

In `qp101-viz/README.md`, replace the opening sentence after the title with:

````markdown
`qp101-viz` is a local Typst package prototype for rendering QP101-ZY circuit
JSON as a timeline view. For normal static SVG output, prefer the built-in
`rstim render_svg` CLI:

```sh
rstim render_svg --in circuit.stim --out circuit.svg
```

This package remains useful as optional legacy/prototype infrastructure for
Typst-specific workflows and direct QP101 JSON experiments.
````

Leave the existing Typst API and example sections in place.

- [ ] **Step 9: Run the focused test to verify GREEN**

Run:

```sh
cargo test -p rstim --test cli_render_svg render_svg_documented_workflow_matches_cli -q
```

Expected: PASS with 1 test passed.

If dependency resolution fails because the environment is offline, run:

```sh
CARGO_NET_OFFLINE=true cargo test -p rstim --test cli_render_svg render_svg_documented_workflow_matches_cli -q
```

Expected: PASS with 1 test passed.

- [ ] **Step 10: Run required broader checks and commit**

Run:

```sh
git diff --check
cargo test
```

If the exact `cargo test` command cannot resolve dependencies in this restricted
network environment, also run:

```sh
CARGO_NET_OFFLINE=true cargo test
```

Expected: `git diff --check` exits 0 and the available cargo test command exits
0.

Commit the implementation:

```sh
git add README.md rstim/doc/cli.md qp101-viz/README.md rstim/tests/cli_render_svg.rs
git commit -m "docs: document render_svg workflow"
```
