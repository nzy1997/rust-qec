# Issue 345 Random-Window Weight Pruning Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Prune random-window component candidates whose weight cannot improve the current best witness while preserving upper-bound semantics.

**Architecture:** Split the existing random-window candidate loop so a testable row-processing helper can apply the new current-best weight guard before stabilizer span checks and witness validation. Extend `RandomWindowSearchStats` and benchmark summary handling so pruning is visible in Rust CLI JSON and benchmark summaries.

**Tech Stack:** Rust (`qec-code` crate), serde JSON, Python unittest benchmark summarizer tests, Cargo, Make.

## Global Constraints

- Do not add target-weight early stopping to no-target benchmarks.
- Do not change random permutations, random seed semantics, or the issue-225 ladder fixture.
- Do not add bit-packed storage or a reusable GF(2) workspace.
- Do not claim exact distance certification.
- Prune equal-weight candidates because the result contract only promises the upper-bound value and one valid witness.
- Apply pruning after rejecting zero candidates and before row-span membership and Pauli witness construction.
- Preserve deterministic output for the same seed as much as possible.
- Keep timing diagnostics from issue #343 and expect span-filter or validation timing to drop on no-target smoke runs when pruning is active.

---

### Task 1: Current-Best Weight Pruning And Diagnostics

**Files:**
- Modify: `qec-code/src/distance_bound.rs`
- Modify: `qec-code/tests/distance_bound.rs`
- Modify: `benchmarks/qec_code_random_window/summarize.py`
- Modify: `benchmarks/qec_code_random_window/tests/test_summarize_search_stats.py`
- Modify: `benchmarks/qec_code_random_window/tests/test_summarize_search_timing.py`
- Modify: `benchmarks/qec_code_random_window/tests/test_summarize.py`
- Modify: `qec-code/doc/css_distance.md`

**Interfaces:**
- Consumes: `RandomWindowSearchStats`, `consider_component_candidates`, `component_candidate_to_pauli`, `validate_witness_against_code_with_span`, `SEARCH_STAT_INT_FIELDS`.
- Produces: `RandomWindowSearchStats.weight_pruned_candidates: usize`; a private `consider_component_candidate_rows(...) -> Result<()>` helper; CSV field `search_stats_total_weight_pruned_candidates`.

- [ ] **Step 1: Write failing Rust pruning tests**

Add these helper functions and tests inside the existing `#[cfg(test)] mod tests` in `qec-code/src/distance_bound.rs`:

```rust
fn empty_reduced_rows(width: usize) -> gf2::ReducedRows {
    gf2::try_rref_with_width(&[], width).unwrap()
}

fn x_pauli(width: usize, support: &[usize]) -> Pauli {
    let mut x = vec![0; width];
    for &index in support {
        x[index] = 1;
    }
    Pauli::from_xz_bits(x, vec![0; width]).unwrap()
}

#[test]
fn random_window_prunes_candidates_that_cannot_improve_best() {
    let width = 3;
    let code = StabilizerCode::from_stabilizers(width, vec![]).unwrap();
    let component_span = empty_reduced_rows(width);
    let stabilizer_span = empty_reduced_rows(width * 2);
    let mut best_witness = Some(x_pauli(width, &[0, 1]));
    let mut search_stats = RandomWindowSearchStats::default();

    consider_component_candidate_rows(
        vec![
            vec![0, 0, 0],
            vec![1, 1, 0],
            vec![1, 1, 1],
            vec![0, 0, 1],
        ],
        &component_span,
        ComponentKind::XLike,
        &code,
        &stabilizer_span,
        &mut best_witness,
        &mut search_stats,
    )
    .unwrap();

    let best = best_witness.expect("strictly lighter candidate should replace current best");
    assert_eq!(best.weight(), 1);
    assert_eq!(search_stats.component_candidates_generated, 4);
    assert_eq!(search_stats.zero_candidates_rejected, 1);
    assert_eq!(search_stats.weight_pruned_candidates, 2);
    assert_eq!(search_stats.valid_witnesses_found, 1);
    assert_eq!(search_stats.best_witness_updates, 1);
    assert_eq!(search_stats.stabilizer_span_candidates_rejected, 0);
    assert_eq!(search_stats.witness_validation_candidates_rejected, 0);

    let stats_json = serde_json::to_value(search_stats).unwrap();
    assert_eq!(stats_json["weight_pruned_candidates"], 2);
}

#[test]
fn random_window_pruning_does_not_skip_strictly_better_candidate() {
    let width = 5;
    let code = StabilizerCode::from_stabilizers(width, vec![]).unwrap();
    let component_span = empty_reduced_rows(width);
    let stabilizer_span = empty_reduced_rows(width * 2);
    let mut best_witness = Some(x_pauli(width, &[0, 1, 2, 3, 4]));
    let mut search_stats = RandomWindowSearchStats::default();

    consider_component_candidate_rows(
        vec![vec![1, 1, 1, 0, 0]],
        &component_span,
        ComponentKind::XLike,
        &code,
        &stabilizer_span,
        &mut best_witness,
        &mut search_stats,
    )
    .unwrap();

    let best = best_witness.expect("weight-3 candidate should replace weight-5 best");
    assert_eq!(best.weight(), 3);
    assert_eq!(search_stats.component_candidates_generated, 1);
    assert_eq!(search_stats.weight_pruned_candidates, 0);
    assert_eq!(search_stats.valid_witnesses_found, 1);
    assert_eq!(search_stats.best_witness_updates, 1);
}
```

- [ ] **Step 2: Run tests to verify they fail before implementation**

Run:

```bash
cargo test -p qec-code random_window_prunes_candidates_that_cannot_improve_best -q --offline
cargo test -p qec-code random_window_pruning_does_not_skip_strictly_better_candidate -q --offline
```

Expected: both fail to compile because `consider_component_candidate_rows` and `weight_pruned_candidates` do not exist.

- [ ] **Step 3: Implement the stats field and row-processing helper**

In `RandomWindowSearchStats`, add the field after `zero_candidates_rejected`:

```rust
pub weight_pruned_candidates: usize,
```

Replace the body of `consider_component_candidates` after candidate generation with:

```rust
    let candidates = candidates?;
    consider_component_candidate_rows(
        candidates,
        stabilizer_component_span,
        component,
        code,
        stabilizer_span,
        best_witness,
        search_stats,
    )
```

Add the helper below `consider_component_candidates`:

```rust
fn consider_component_candidate_rows(
    candidates: Vec<Vec<u8>>,
    stabilizer_component_span: &gf2::ReducedRows,
    component: ComponentKind,
    code: &StabilizerCode,
    stabilizer_span: &gf2::ReducedRows,
    best_witness: &mut Option<Pauli>,
    search_stats: &mut RandomWindowSearchStats,
) -> Result<()> {
    search_stats.component_candidates_generated += candidates.len();

    for candidate in candidates {
        let span_started = Instant::now();
        let candidate_weight = candidate.iter().filter(|&&bit| bit == 1).count();
        if candidate_weight == 0 {
            add_elapsed_ns(&mut search_stats.span_filter_time_ns, span_started);
            search_stats.zero_candidates_rejected += 1;
            continue;
        }
        if best_witness
            .as_ref()
            .is_some_and(|current| candidate_weight >= current.weight())
        {
            add_elapsed_ns(&mut search_stats.span_filter_time_ns, span_started);
            search_stats.weight_pruned_candidates += 1;
            continue;
        }
        let in_component_span = gf2::try_in_reduced_row_span(stabilizer_component_span, &candidate);
        add_elapsed_ns(&mut search_stats.span_filter_time_ns, span_started);
        if in_component_span? {
            search_stats.stabilizer_span_candidates_rejected += 1;
            continue;
        }

        let validation_started = Instant::now();
        let witness = component_candidate_to_pauli(component, candidate)?;
        let witness_is_valid =
            validate_witness_against_code_with_span(code, stabilizer_span, &witness).is_ok();
        add_elapsed_ns(
            &mut search_stats.witness_validation_time_ns,
            validation_started,
        );
        if !witness_is_valid {
            search_stats.witness_validation_candidates_rejected += 1;
            continue;
        }
        search_stats.valid_witnesses_found += 1;
        let best_update_started = Instant::now();
        let should_update = best_witness
            .as_ref()
            .is_none_or(|current| witness.weight() < current.weight());
        if should_update {
            search_stats.best_witness_updates += 1;
            *best_witness = Some(witness);
        }
        add_elapsed_ns(&mut search_stats.best_update_time_ns, best_update_started);
    }

    Ok(())
}
```

- [ ] **Step 4: Update Rust serialization coverage and docs**

In `qec-code/tests/distance_bound.rs`, add `"weight_pruned_candidates"` to the `random_window_upper_bound_reports_search_stats` field list after `"zero_candidates_rejected"`.

In `qec-code/doc/css_distance.md`, add `"weight_pruned_candidates": 0` to the JSON example after `"zero_candidates_rejected": 0`, and mention current-best weight pruning in the `search_stats` bullet.

- [ ] **Step 5: Update Python summary field and tests**

In `benchmarks/qec_code_random_window/summarize.py`, add `"weight_pruned_candidates"` to `SEARCH_STAT_INT_FIELDS` after `"zero_candidates_rejected"`.

In `write_summary_md`, include the aggregate in `search_stats_text`:

```python
                f"weight_pruned={summary['search_stats_total_weight_pruned_candidates']}, "
```

Update fake stats in `benchmarks/qec_code_random_window/tests/test_summarize_search_stats.py` and `benchmarks/qec_code_random_window/tests/test_summarize_search_timing.py` to include `"weight_pruned_candidates"`. In the positive search-stats test, set the two rows to `5` and `7`, then assert:

```python
self.assertEqual(row["search_stats_total_weight_pruned_candidates"], "12")
self.assertIn("weight_pruned=12", markdown)
```

In `benchmarks/qec_code_random_window/tests/test_summarize.py`, add the blank expected CSV key `"search_stats_total_weight_pruned_candidates": ""` immediately after `"search_stats_total_zero_candidates_rejected": ""` in each expected summary row.

- [ ] **Step 6: Run focused green tests**

Run:

```bash
cargo test -p qec-code random_window_prunes_candidates_that_cannot_improve_best -q --offline
cargo test -p qec-code random_window_pruning_does_not_skip_strictly_better_candidate -q --offline
cargo test -p qec-code random_window_upper_bound_reports_search_stats -q --offline
python3 -m unittest benchmarks.qec_code_random_window.tests.test_summarize_search_stats -q
python3 -m unittest benchmarks.qec_code_random_window.tests.test_summarize_search_timing -q
python3 -m unittest benchmarks.qec_code_random_window.tests.test_summarize -q
```

Expected: all pass.

- [ ] **Step 7: Commit implementation**

Run:

```bash
git add qec-code/src/distance_bound.rs qec-code/tests/distance_bound.rs qec-code/doc/css_distance.md benchmarks/qec_code_random_window/summarize.py benchmarks/qec_code_random_window/tests/test_summarize_search_stats.py benchmarks/qec_code_random_window/tests/test_summarize_search_timing.py benchmarks/qec_code_random_window/tests/test_summarize.py
git commit -m "feat: prune random-window non-improving candidates"
```

Expected: one implementation commit containing only the pruning code, tests, summary handling, and related docs.
