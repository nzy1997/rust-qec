# QP101 Rename And Coverage Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Rename the `qp101` export surface consistently across code, tests, fixtures, and docs while adding targeted tests that improve PR coverage on the export and CLI paths.

**Architecture:** Keep behavior and JSON structure the same except for the renamed standard/module/type language, then drive the refactor with test-first changes in the exporter and CLI integration tests. Update fixture paths and documentation in the same pass so the repository presents only the new `qp101` name.

**Tech Stack:** Rust, clap, serde/serde_json, cargo test

---

### Task 1: Add failing tests for the renamed export surface

**Files:**
- Modify: `/Users/nzy/rcode/rstim/rstim/tests/cli_export_json.rs`
- Modify: `/Users/nzy/rcode/rstim/rstim/tests/qp101_export.rs`
- Modify: `/Users/nzy/rcode/rstim/rstim/tests/qp101_fixtures.rs`

**Step 1: Write the failing test**

- Update exporter tests to import `rstim::qp101` symbols and expect `QP101-ZY`.
- Add exporter assertions that cover observable include validation and raw target serialization for inverted, sweep, and noise targets.
- Add CLI assertions that cover stdout export, file export, and invalid circuit input under the renamed standard.

**Step 2: Run test to verify it fails**

Run: `cargo test --test qp101_export --test qp101_fixtures --test cli_export_json`
Expected: FAIL to compile or FAIL assertions because the code still exposes the pre-rename surface.

**Step 3: Write minimal implementation**

- Rename the module and public symbols to `qp101`.
- Update CLI export code to call the renamed exporter.
- Keep the JSON layout unchanged except for the renamed standard string.

**Step 4: Run test to verify it passes**

Run: `cargo test --test qp101_export --test qp101_fixtures --test cli_export_json`
Expected: PASS

### Task 2: Rename files, fixtures, and docs to qp101

**Files:**
- Modify: `/Users/nzy/rcode/rstim/rstim/src/lib.rs`
- Move: `/Users/nzy/rcode/rstim/rstim/src/qp101.rs`
- Modify: `/Users/nzy/rcode/rstim/rstim/src/cli.rs`
- Move: `/Users/nzy/rcode/rstim/rstim/tests/qp101_export.rs`
- Move: `/Users/nzy/rcode/rstim/rstim/tests/qp101_fixtures.rs`
- Move: `/Users/nzy/rcode/rstim/rstim/tests/fixtures/qp101`
- Modify: `/Users/nzy/rcode/rstim/rstim/doc/cli.md`
- Modify: `/Users/nzy/rcode/rstim/rstim/doc/getting_started.md`
- Modify: `/Users/nzy/rcode/rstim/README.md`
- Modify: `/Users/nzy/rcode/rstim/docs/plans/2026-03-21-qp101-viz-semantic-anchor-design.md`
- Modify: `/Users/nzy/rcode/rstim/docs/plans/2026-03-21-qp101-viz-semantic-anchor-plan.md`
- Modify: `/Users/nzy/rcode/rstim/docs/plans/2026-03-22-rstim-stats-cli-design.md`
- Modify: `/Users/nzy/rcode/rstim/docs/plans/2026-03-22-rstim-stats-cli-plan.md`
- Modify: `/Users/nzy/rcode/rstim/tmp/tycode/qp101-viz/examples/anchor-basic.typ`
- Modify: `/Users/nzy/rcode/rstim/tmp/tycode/qp101-viz/examples/anchor-repeat.typ`

**Step 1: Write the failing test**

- Re-run the renamed tests after moving test files or imports if compilation still points at old paths.

**Step 2: Run test to verify it fails**

Run: `cargo test --test qp101_export --test qp101_fixtures --test cli_export_json`
Expected: FAIL until all module paths, file paths, and fixture references are aligned.

**Step 3: Write minimal implementation**

- Rename code module paths and fixture directories.
- Rename human-facing references in docs and examples from `qp101`'s previous name to `qp101`.
- Update any example file names or comments that still expose the old term.

**Step 4: Run test to verify it passes**

Run: `cargo test --test qp101_export --test qp101_fixtures --test cli_export_json`
Expected: PASS

### Task 3: Verify broader coverage-sensitive paths

**Files:**
- Test: `/Users/nzy/rcode/rstim/rstim/tests/cli_stats.rs`
- Test: `/Users/nzy/rcode/rstim/rstim/tests/stats.rs`

**Step 1: Write the failing test**

- Add only any additional stats or CLI export tests needed if verification still shows renamed/new branches uncovered.

**Step 2: Run test to verify it fails**

Run: `cargo test --test cli_stats --test stats`
Expected: FAIL only if a new branch-specific test was added.

**Step 3: Write minimal implementation**

- Adjust code or expectations only as needed to satisfy the new branch coverage tests.

**Step 4: Run test to verify it passes**

Run: `cargo test --test cli_stats --test stats`
Expected: PASS

### Task 4: Final verification

**Files:**
- Modify: `/Users/nzy/rcode/rstim/docs/plans/2026-03-22-qp101-rename-coverage-plan.md`

**Step 1: Run targeted verification**

Run: `cargo test --test qp101_export --test qp101_fixtures --test cli_export_json --test cli_stats --test stats`
Expected: PASS

**Step 2: Run broader verification**

Run: `cargo test --workspace`
Expected: PASS

**Step 3: Record outcome**

- Update the final task notes in this plan if the verification surface had to change.
