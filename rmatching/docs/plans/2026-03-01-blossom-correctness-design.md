# Blossom Correctness + Cross-Validation Design

**Date:** 2026-03-01

## Problem

Two stubs in the current rmatching codebase cause silent wrong answers whenever a blossom forms during decoding:

1. `GraphFlooder::do_blossom_shattering` (graph_flooder.rs:471) — returns `NoEvent` instead of emitting `BlossomShatter`
2. `Mwpm::pair_and_shatter_subblossoms` (mwpm.rs:683) — no-op instead of recursively extracting sub-blossom matches

Blossoms form whenever an odd cycle appears in the alternating tree (3+ detection events connected in an odd cycle). Any non-trivial syndrome on a surface code will routinely trigger blossoms.

## Approach

Direct port from PyMatching C++ reference + cross-validation script.

---

## Section 1: Fix `do_blossom_shattering`

**Reference:** `matcher/mwpm.cc` — the flooder's job is to detect when a shrinking blossom empties and emit the `BlossomShatter` event with the right `in_parent` and `in_child` regions.

**What it needs to do:**

When a blossom region's shell empties while shrinking, the flooder must:
1. Look up the blossom's `alt_tree_node` to find its position in the alternating tree
2. Find `in_parent`: the blossom child region connected toward the tree's root (via the alt-tree parent edge's compressed edge)
3. Find `in_child`: the blossom child region that holds the detection event that the blossom will "pass through" to continue the path outward
4. Emit `MwpmEvent::BlossomShatter { blossom: region_idx, in_parent, in_child }`

The flooder has access to `region_arena` (which stores `blossom_children: Vec<RegionEdge>`) and `alt_tree_node: Option<AltTreeIdx>`. However, the flooder does NOT own `node_arena` (AltTreeNode). To find `in_parent`/`in_child`, the flooder needs to be able to read alt-tree parent info.

**Solution:** Pass a reference to `node_arena: &Arena<AltTreeNode>` into `do_blossom_shattering`, or store the relevant info (parent region, child region) directly on `GraphFillRegion` when the blossom is created in `Mwpm::create_blossom`. The simpler option is to store `blossom_in_parent: Option<RegionIdx>` and `blossom_in_child: Option<RegionIdx>` on `GraphFillRegion`, set when creating the blossom, updated when shattering.

After careful study of the C++ reference (mwpm.cc), the correct approach is: when `Mwpm::create_blossom` is called, record which child connects toward the parent (the `in_parent` child) in the region. Then `do_blossom_shattering` reads it directly from the region without needing `node_arena`.

---

## Section 2: Fix `pair_and_shatter_subblossoms`

**Reference:** `driver/mwpm_decoding.cc` — `shatter_blossom_into_matches` function.

**What it needs to do:**

When extracting matches and a region is itself a blossom, pair up its children:
1. Iterate over `blossom_children: Vec<RegionEdge>` (which stores alternating inner/outer regions in order)
2. Children alternate: outer (growing), inner (shrinking), outer, inner, ...
3. Each inner/outer pair was matched during shattering — extract the edge from the blossom child's `match_` field
4. Recursively call `shatter_blossom_and_extract_matches` on each child that is itself a blossom
5. XOR all `obs_mask` values, sum weights

The key lookup needed: given the edge from the outer match to the boundary/other-tree, find which blossom child's `shell_area` contains `loc_from` / `loc_to`. This uses `region_that_arrived_top` on the detector nodes.

---

## Section 3: Cross-Validation

### CLI Binary (`src/bin/rmatching_cli.rs`)

A simple binary that:
- Takes a DEM file path as argument
- Reads newline-separated syndrome lines from stdin, each as space-separated `0`/`1` values (one per detector)
- Outputs one prediction line per input (space-separated `0`/`1` per observable)

```
rmatching_cli <dem_file>
stdin:  1 0 1 0 0
stdout: 1 0
```

### Python Script (`tests/cross_validate.py`)

```python
import stim, pymatching, subprocess, random

dem = stim.Circuit("""...""").detector_error_model(decompose_errors=True)
pm = pymatching.Matching.from_detector_error_model(dem)

syndromes = [random_syndrome(...) for _ in range(1000)]
pm_preds = [pm.decode(s) for s in syndromes]
rm_preds = [rmatching_cli_decode(dem_path, s) for s in syndromes]

assert pm_preds == rm_preds, f"Mismatch at index {first_diff_index}"
```

### Rust Test (`tests/cross_validate.rs`)

```rust
#[test]
#[ignore]  // run with: cargo test -- --include-ignored cross_validate
fn cross_validate_rep_code_d5() {
    let status = Command::new("python3")
        .arg("tests/cross_validate.py")
        .status().unwrap();
    assert!(status.success());
}
```

---

## Task Breakdown

1. **Study C++ reference** — read `mwpm.cc` and `mwpm_decoding.cc` to understand exact blossom shattering logic
2. **Add fields to GraphFillRegion** — `blossom_in_parent: Option<RegionIdx>`, `blossom_in_child: Option<RegionIdx>`
3. **Implement `do_blossom_shattering`** — emit correct `BlossomShatter` event
4. **Implement `pair_and_shatter_subblossoms`** — recursive sub-blossom match extraction
5. **Add unit tests** — blossom shattering on triangle, pentagon, nested blossom
6. **Add CLI binary** — `src/bin/rmatching_cli.rs`
7. **Add cross-validation script** — `tests/cross_validate.py`
8. **Add ignored Rust test** — `tests/cross_validate.rs`
