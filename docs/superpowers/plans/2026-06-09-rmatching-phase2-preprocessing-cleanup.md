# rmatching Phase 2 Preprocessing Cleanup Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove the per-decode `HashSet` allocation from `rmatching` negative-weight detector preprocessing while preserving byte-path and packed-path decode semantics.

**Architecture:** Add a graph-owned sorted view of negative-weight detector indices during `MatchingGraph` construction, then switch decode-time preprocessing to a linear symmetric-difference merge over ordered detector-index buffers. Keep all public `Matching` APIs, MWPM execution, and benchmark wiring unchanged.

**Tech Stack:** Rust, Cargo tests, existing `rmatching` unit tests and surface decoder comparison benchmark tooling

---

## File Structure

- Modify: `rmatching/src/flooder/graph.rs`
  - Extend `MatchingGraph` with a sorted negative-weight detector cache and a small helper to finalize graph-owned derived metadata after edge construction.
- Modify: `rmatching/src/driver/user_graph.rs`
  - Finalize the new graph-owned sorted cache after all regular and boundary edges are inserted.
- Modify: `rmatching/src/driver/decoding.rs`
  - Replace the `HashSet`-based negative-weight preprocessing path with an ordered symmetric-difference merge and update tests to target the new semantics directly.
- Create: `docs/superpowers/plans/2026-06-09-rmatching-phase2-preprocessing-cleanup.md`
  - Implementation handoff for the approved phase 2 design.

### Task 1: Lock in Failing Coverage for Ordered Negative-Weight Preprocessing

**Files:**
- Modify: `rmatching/src/driver/decoding.rs`
- Test: `rmatching/src/driver/decoding.rs`

- [ ] **Step 1: Write the failing tests**

Add the following tests inside `#[cfg(test)] mod tests` in `rmatching/src/driver/decoding.rs`, directly after the existing `apply_negative_weight_events_into_filters_and_sorts` test:

```rust
    #[test]
    fn apply_negative_weight_events_into_merges_sorted_inputs_without_hashing() {
        let detection_events = vec![1, 3, 6];
        let neg_det_sorted = vec![0, 3, 4, 7];
        let is_boundary = vec![false; 8];
        let mut out = vec![999];

        apply_negative_weight_events_into(
            &detection_events,
            &neg_det_sorted,
            &is_boundary,
            &mut out,
        );

        assert_eq!(out, vec![0, 1, 4, 6, 7]);
    }

    #[test]
    fn apply_negative_weight_events_into_filters_boundary_nodes_from_both_inputs() {
        let detection_events = vec![0, 2, 5];
        let neg_det_sorted = vec![1, 2, 4, 6];
        let is_boundary = vec![false, true, false, false, true, false, false];
        let mut out = vec![999];

        apply_negative_weight_events_into(
            &detection_events,
            &neg_det_sorted,
            &is_boundary,
            &mut out,
        );

        assert_eq!(out, vec![0, 5, 6]);
    }
```

- [ ] **Step 2: Run the focused test target and verify it fails**

Run:

```bash
cargo test -p rmatching apply_negative_weight_events_into_merges_sorted_inputs_without_hashing -- --exact
```

Expected: compile failure because `apply_negative_weight_events_into(...)` still expects `&HashSet<usize>` instead of `&[usize]`.

- [ ] **Step 3: Commit the failing-test checkpoint**

```bash
git add rmatching/src/driver/decoding.rs
git commit -m "test: cover ordered negative-weight preprocessing"
```

Expected: commit records the new failing tests before implementation begins.

### Task 2: Add a Graph-Owned Sorted Negative-Weight Detector Cache

**Files:**
- Modify: `rmatching/src/flooder/graph.rs`
- Modify: `rmatching/src/driver/user_graph.rs`
- Test: `rmatching/src/driver/decoding.rs`

- [ ] **Step 1: Extend `MatchingGraph` with sorted derived metadata**

Update `rmatching/src/flooder/graph.rs` so `MatchingGraph` stores a sorted detector view and can finalize it after graph construction:

```rust
pub struct MatchingGraph {
    pub nodes: Vec<DetectorNode>,
    pub num_observables: usize,
    pub negative_weight_detection_events_set: HashSet<usize>,
    pub negative_weight_detection_events_sorted: Vec<usize>,
    pub negative_weight_observables_set: HashSet<usize>,
    pub negative_weight_obs_mask: ObsMask,
    pub negative_weight_sum: TotalWeight,
    pub is_user_graph_boundary_node: Vec<bool>,
    pub normalising_constant: f64,
}
```

Initialize the new field in `MatchingGraph::new(...)`:

```rust
        MatchingGraph {
            nodes: (0..num_nodes).map(|_| DetectorNode::new()).collect(),
            num_observables,
            negative_weight_detection_events_set: HashSet::new(),
            negative_weight_detection_events_sorted: Vec::new(),
            negative_weight_observables_set: HashSet::new(),
            negative_weight_obs_mask: 0,
            negative_weight_sum: 0,
            is_user_graph_boundary_node: Vec::new(),
            normalising_constant: 1.0,
        }
```

Add a graph finalizer method at the end of the `impl MatchingGraph` block:

```rust
    pub fn finalize_derived_state(&mut self) {
        self.negative_weight_detection_events_sorted.clear();
        self.negative_weight_detection_events_sorted.extend(
            self.negative_weight_detection_events_set.iter().copied(),
        );
        self.negative_weight_detection_events_sorted.sort_unstable();
    }
```

- [ ] **Step 2: Finalize the sorted cache after graph construction**

Update `rmatching/src/driver/user_graph.rs` inside `to_matching_graph(...)`, immediately before `mg` is returned:

```rust
        if !self.boundary_nodes.is_empty() {
            mg.is_user_graph_boundary_node = vec![false; self.nodes.len()];
            for &i in &self.boundary_nodes {
                mg.is_user_graph_boundary_node[i] = true;
            }
        }

        mg.finalize_derived_state();
        mg
```

- [ ] **Step 3: Add coverage for graph finalization**

Add this unit test near the other decoding-focused tests in `rmatching/src/driver/decoding.rs`, after the two new preprocessing tests from Task 1:

```rust
    #[test]
    fn negative_weight_detector_cache_is_sorted_after_graph_build() {
        let mut matching = Matching::new();
        matching.add_edge(5, 1, -1.0, &[], 0.1);
        matching.add_edge(3, 5, -1.0, &[], 0.1);
        matching.add_boundary_edge(2, -1.0, &[], 0.1);

        let mwpm = matching.user_graph.get_mwpm();

        assert_eq!(
            mwpm.flooder.graph.negative_weight_detection_events_sorted,
            vec![1, 2, 3]
        );
    }
```

- [ ] **Step 4: Run the focused cache test**

Run:

```bash
cargo test -p rmatching negative_weight_detector_cache_is_sorted_after_graph_build -- --exact
```

Expected: PASS, confirming the graph now exposes the sorted negative-weight detector cache.

- [ ] **Step 5: Commit the graph-cache change**

```bash
git add rmatching/src/flooder/graph.rs rmatching/src/driver/user_graph.rs rmatching/src/driver/decoding.rs
git commit -m "refactor: cache sorted negative-weight detectors"
```

### Task 3: Replace the Decode-Time `HashSet` Path With a Sorted Merge

**Files:**
- Modify: `rmatching/src/driver/decoding.rs`
- Test: `rmatching/src/driver/decoding.rs`

- [ ] **Step 1: Change the helper signatures to consume ordered slices**

Update the helper signatures in `rmatching/src/driver/decoding.rs`:

```rust
fn apply_negative_weight_events(
    detection_events: &[usize],
    neg_det_sorted: &[usize],
    is_boundary: &[bool],
) -> Vec<usize> {
    let mut result = Vec::new();
    apply_negative_weight_events_into(detection_events, neg_det_sorted, is_boundary, &mut result);
    result
}

fn apply_negative_weight_events_into(
    detection_events: &[usize],
    neg_det_sorted: &[usize],
    is_boundary: &[bool],
    out: &mut Vec<usize>,
) {
```

Update every call site in the same file so they pass:

```rust
&mwpm.flooder.graph.negative_weight_detection_events_sorted
```

instead of:

```rust
&mwpm.flooder.graph.negative_weight_detection_events_set
```

- [ ] **Step 2: Implement the ordered symmetric-difference merge**

Replace the body of `apply_negative_weight_events_into(...)` with:

```rust
    if neg_det_sorted.is_empty() {
        out.clear();
        out.extend(
            detection_events
                .iter()
                .copied()
                .filter(|&d| d >= is_boundary.len() || !is_boundary[d]),
        );
        return;
    }

    out.clear();
    let mut det_i = 0;
    let mut neg_i = 0;

    while det_i < detection_events.len() && neg_i < neg_det_sorted.len() {
        let det = detection_events[det_i];
        let neg = neg_det_sorted[neg_i];

        if det == neg {
            det_i += 1;
            neg_i += 1;
            continue;
        }

        let candidate = if det < neg {
            det_i += 1;
            det
        } else {
            neg_i += 1;
            neg
        };

        if candidate >= is_boundary.len() || !is_boundary[candidate] {
            out.push(candidate);
        }
    }

    while det_i < detection_events.len() {
        let det = detection_events[det_i];
        det_i += 1;
        if det >= is_boundary.len() || !is_boundary[det] {
            out.push(det);
        }
    }

    while neg_i < neg_det_sorted.len() {
        let neg = neg_det_sorted[neg_i];
        neg_i += 1;
        if neg >= is_boundary.len() || !is_boundary[neg] {
            out.push(neg);
        }
    }
```

- [ ] **Step 3: Remove stale test-only imports and update existing tests**

In `rmatching/src/driver/decoding.rs` test module:

- delete `use std::collections::HashSet;`
- update the existing `apply_negative_weight_events_into_filters_and_sorts` test to pass a sorted `Vec<usize>`:

```rust
        let neg_det_sorted = vec![2usize, 3usize];
```

and:

```rust
            &neg_det_sorted,
```

Expected assertion stays:

```rust
        assert_eq!(out, vec![0, 4]);
```

- [ ] **Step 4: Run the focused decoding unit tests**

Run:

```bash
cargo test -p rmatching driver::decoding::tests::apply_negative_weight_events_into_filters_and_sorts -- --exact
cargo test -p rmatching driver::decoding::tests::apply_negative_weight_events_into_merges_sorted_inputs_without_hashing -- --exact
cargo test -p rmatching driver::decoding::tests::apply_negative_weight_events_into_filters_boundary_nodes_from_both_inputs -- --exact
```

Expected: all three tests PASS.

- [ ] **Step 5: Commit the preprocessing helper rewrite**

```bash
git add rmatching/src/driver/decoding.rs
git commit -m "refactor: merge ordered negative-weight events"
```

### Task 4: Prove Decode Semantics Still Match Public and Packed Paths

**Files:**
- Modify: `rmatching/src/driver/decoding.rs`
- Test: `rmatching/src/driver/decoding.rs`

- [ ] **Step 1: Add a regression test covering negative-weight decode agreement**

Add this test in `rmatching/src/driver/decoding.rs`, after
`decode_bit_packed_into_matches_byte_syndrome_decode`:

```rust
    #[test]
    fn decode_negative_weight_graph_keeps_byte_and_packed_paths_aligned() {
        let mut matching = Matching::new();
        matching.add_edge(0, 1, -1.0, &[0], 0.1);
        matching.add_boundary_edge(0, 2.0, &[], 0.1);
        matching.add_boundary_edge(1, 2.0, &[], 0.1);

        let syndrome = vec![1u8, 0u8];
        let expected = matching.decode(&syndrome);
        let mut packed_out = vec![0xAA];

        matching.decode_bit_packed_into(&[0b0000_0001], 2, 1, &mut packed_out);

        assert_eq!(packed_out, vec![expected[0]]);
    }
```

- [ ] **Step 2: Run the focused regression test**

Run:

```bash
cargo test -p rmatching decode_negative_weight_graph_keeps_byte_and_packed_paths_aligned -- --exact
```

Expected: PASS.

- [ ] **Step 3: Run the full `rmatching` test suite**

Run:

```bash
cargo test -p rmatching
```

Expected: PASS across the existing `rmatching` unit and integration test suite.

- [ ] **Step 4: Commit the semantic-regression coverage**

```bash
git add rmatching/src/driver/decoding.rs
git commit -m "test: cover negative-weight packed decode agreement"
```

### Task 5: Rerun the rmatching Benchmark Slice and Refresh Reported Results

**Files:**
- Modify: `benchmarks/surface_decoder_compare/results/full/results.csv`
- Modify: `benchmarks/surface_decoder_compare/results/full/surface_decoder_compare.png`

- [ ] **Step 1: Run the `rmatching` full-tier benchmark slice**

Run from the repo root worktree:

```bash
.venv-surface-decoder/bin/python -m benchmarks.surface_decoder_compare.run_compare --tier full --decoders rmatching --merge-into-existing
```

Expected: benchmark runner completes and updates the `rmatching` rows in
`benchmarks/surface_decoder_compare/results/full/results.csv`.

- [ ] **Step 2: Regenerate the comparison figure**

Run:

```bash
.venv-surface-decoder/bin/python -m benchmarks.surface_decoder_compare.plot_compare --tier full
```

Expected: refreshed `benchmarks/surface_decoder_compare/results/full/surface_decoder_compare.png`.

- [ ] **Step 3: Inspect the result diff**

Run:

```bash
git diff -- benchmarks/surface_decoder_compare/results/full/results.csv
git diff --stat -- benchmarks/surface_decoder_compare/results/full/surface_decoder_compare.png
```

Expected: only `rmatching` benchmark entries and the corresponding plot update change.

- [ ] **Step 4: Commit the benchmark refresh if results changed materially**

```bash
git add benchmarks/surface_decoder_compare/results/full/results.csv benchmarks/surface_decoder_compare/results/full/surface_decoder_compare.png
git commit -m "bench: refresh rmatching phase 2 results"
```

If the benchmark output is unchanged or too noisy to keep, skip this commit and note that outcome in the final review.

### Task 6: Final Verification and Review Handoff

**Files:**
- Modify: `rmatching/src/flooder/graph.rs`
- Modify: `rmatching/src/driver/user_graph.rs`
- Modify: `rmatching/src/driver/decoding.rs`
- Modify: `benchmarks/surface_decoder_compare/results/full/results.csv` (if benchmark diff kept)
- Modify: `benchmarks/surface_decoder_compare/results/full/surface_decoder_compare.png` (if benchmark diff kept)

- [ ] **Step 1: Re-run the final verification commands**

Run:

```bash
cargo test -p rmatching
.venv-surface-decoder/bin/python -m benchmarks.surface_decoder_compare.run_compare --tier full --decoders rmatching --merge-into-existing
.venv-surface-decoder/bin/python -m benchmarks.surface_decoder_compare.plot_compare --tier full
```

Expected: tests pass, benchmark completes, and plot generation succeeds.

- [ ] **Step 2: Review the final diff**

Run:

```bash
git status --short
git diff --stat
```

Expected: only the planned code, tests, and benchmark artifacts are modified.

- [ ] **Step 3: Request code review before merge**

Use the required review workflow on the completed branch before any PR or merge step.

Example command sequence:

```bash
git log --oneline --decorate -n 5
git status --short
```

Expected: branch is ready for `superpowers:requesting-code-review`.
