# Blossom Correctness + Cross-Validation Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Fix two critical stubs (`do_blossom_shattering` and `pair_and_shatter_subblossoms`) so rmatching produces correct predictions for all syndromes, then cross-validate against PyMatching.

**Architecture:** Direct port of PyMatching C++ logic. The flooder emits `BlossomShatter` events using `heir_region_on_shatter()` (walks `region_that_arrived` up the blossom parent chain). The matcher shatters sub-blossoms during match extraction by finding the outermost blossom child owning the match edge endpoint, then pairing remaining children in order. A CLI binary and Python script cross-validate against PyMatching on 1000+ random syndromes.

**Tech Stack:** Rust 2024, no new dependencies. Cross-validation uses `pip install pymatching stim` in Python.

**Reference:** `PyMatching/src/pymatching/sparse_blossom/`

---

### Task 1: Add `heir_region_on_shatter` to DetectorNode

**Files:**
- Modify: `src/flooder/detector_node.rs`

**Context:** In PyMatching's C++ (`detector_node.cc:55-64`):
```cpp
GraphFillRegion *DetectorNode::heir_region_on_shatter() const {
    GraphFillRegion *r = region_that_arrived;
    while (true) {
        GraphFillRegion *p = r->blossom_parent;
        if (p == region_that_arrived_top) return r;
        r = p;
    }
}
```
This walks the blossom hierarchy from the node's immediate owning region up toward `region_that_arrived_top`, returning the child region one step below the top. Used to determine which blossom child contains a given detector node.

**Step 1: Add the method to DetectorNode**

In `src/flooder/detector_node.rs`, add after the `reset` method:

```rust
/// Walk blossom parent chain from region_that_arrived up to (but not including)
/// region_that_arrived_top. Returns the child region directly under top.
/// Used by do_blossom_shattering to find in_parent and in_child.
pub fn heir_region_on_shatter(&self, regions: &[GraphFillRegion]) -> Option<RegionIdx> {
    let top = self.region_that_arrived_top?;
    let mut r = self.region_that_arrived?;
    loop {
        let parent = regions[r.0 as usize].blossom_parent;
        if parent == Some(top) || parent.is_none() {
            return Some(r);
        }
        r = parent.unwrap();
    }
}
```

**Step 2: Write a unit test in `tests/flooder.rs` (create if needed)**

```rust
#[test]
fn heir_region_on_shatter_single_level() {
    use rmatching::flooder::detector_node::DetectorNode;
    use rmatching::flooder::fill_region::GraphFillRegion;
    use rmatching::types::*;

    // Node owned by region 0, whose top is region 1 (blossom parent of 0)
    let mut regions = vec![GraphFillRegion::default(), GraphFillRegion::default()];
    regions[0].blossom_parent = Some(RegionIdx(1));

    let mut node = DetectorNode::new();
    node.region_that_arrived = Some(RegionIdx(0));
    node.region_that_arrived_top = Some(RegionIdx(1));

    assert_eq!(node.heir_region_on_shatter(&regions), Some(RegionIdx(0)));
}

#[test]
fn heir_region_on_shatter_two_levels() {
    use rmatching::flooder::detector_node::DetectorNode;
    use rmatching::flooder::fill_region::GraphFillRegion;
    use rmatching::types::*;

    // region 0 -> blossom_parent -> region 1 -> blossom_parent -> region 2 (top)
    let mut regions = vec![
        GraphFillRegion::default(),
        GraphFillRegion::default(),
        GraphFillRegion::default(),
    ];
    regions[0].blossom_parent = Some(RegionIdx(1));
    regions[1].blossom_parent = Some(RegionIdx(2));

    let mut node = DetectorNode::new();
    node.region_that_arrived = Some(RegionIdx(0));
    node.region_that_arrived_top = Some(RegionIdx(2));

    // Should return region 1, the child directly under top (2)
    assert_eq!(node.heir_region_on_shatter(&regions), Some(RegionIdx(1)));
}

#[test]
fn heir_region_on_shatter_no_region() {
    use rmatching::flooder::detector_node::DetectorNode;
    use rmatching::flooder::fill_region::GraphFillRegion;
    let regions: Vec<GraphFillRegion> = vec![];
    let node = DetectorNode::new();
    assert_eq!(node.heir_region_on_shatter(&regions), None);
}
```

**Step 3: Run tests**
```
cargo test heir_region
```
Expected: 3 tests pass.

**Step 4: Commit**
```bash
git add src/flooder/detector_node.rs tests/flooder.rs
git commit -m "feat: add heir_region_on_shatter to DetectorNode"
```

---

### Task 2: Implement `do_blossom_shattering` in GraphFlooder

**Files:**
- Modify: `src/flooder/graph_flooder.rs`

**Context:** PyMatching C++ (`graph_flooder.cc:230-235`):
```cpp
MwpmEvent GraphFlooder::do_blossom_shattering(GraphFillRegion &region) {
    return BlossomShatterEventData{
        &region,
        region.alt_tree_node->parent.edge.loc_from->heir_region_on_shatter(),
        region.alt_tree_node->inner_to_outer_edge.loc_from->heir_region_on_shatter()};
}
```

The `region.alt_tree_node` is the `AltTreeIdx` stored on the region. But GraphFlooder does NOT own `node_arena`. The solution: `do_blossom_shattering` needs to accept `node_arena` as a parameter, OR we store the required info on the region when creating the blossom.

**Simpler solution:** Store `blossom_in_parent_node` and `blossom_in_child_node` (the two `NodeIdx`es) on `GraphFillRegion` when `create_blossom` sets up the blossom. Then `do_blossom_shattering` reads them directly.

Actually, even simpler: the flooder already has `region_arena`. The alt_tree_node field on the region stores `Option<AltTreeIdx>`. But the `node_arena` lives in `Mwpm`. So the cleanest fix: make `do_blossom_shattering` take `&Arena<AltTreeNode>` as parameter and call it from the match event loop in `Mwpm`.

Wait — looking at the Rust code, `run_until_next_mwpm_notification` is called from the flooder without access to the node_arena. The `BlossomShatter` event currently gets triggered in `do_region_shrinking` → `do_blossom_shattering`. The flooder doesn't know about `AltTreeNode`.

**Best fix:** Store `blossom_in_parent_loc: Option<NodeIdx>` and `blossom_in_child_loc: Option<NodeIdx>` on `GraphFillRegion`. Set them in `Mwpm::create_blossom` from the parent edge's `loc_from` and the `inner_to_outer_edge.loc_from`. Then `do_blossom_shattering` reads these stored node indices and calls `heir_region_on_shatter`.

**Step 1: Add fields to GraphFillRegion**

In `src/flooder/fill_region.rs`, add two fields:
```rust
pub struct GraphFillRegion {
    // ... existing fields ...
    /// Node anchoring the parent-side edge (set when this is a blossom's inner region)
    pub blossom_in_parent_loc: Option<NodeIdx>,
    /// Node anchoring the child-side edge (set when this is a blossom's inner region)
    pub blossom_in_child_loc: Option<NodeIdx>,
}
```

Also add `Default` derive or update the `Default` impl to include `None` for these fields.

**Step 2: Set fields in `Mwpm::create_blossom`**

In `src/matcher/mwpm.rs`, after `handle_tree_hitting_same_tree` identifies the blossom's inner region's parent alt_tree_node:

The inner region of `common_ancestor`'s alt_tree_node has:
- `inner_to_outer_edge` = the edge from inner to outer within the node
- `parent` edge has `loc_from` = the node anchoring the connection to the tree parent

Set on the blossom region (after `create_blossom` returns `blossom_region`):
```rust
// The blossom's inner_to_outer connecting node tells us in_child_loc
let inner_to_outer_loc = self.node_arena[common_ancestor.0].inner_to_outer_edge.loc_from;
// The parent edge's loc_from tells us in_parent_loc
let parent_loc = self.node_arena[common_ancestor.0].parent
    .as_ref()
    .and_then(|p| p.edge.loc_from);
self.flooder.region_arena[blossom_region.0].blossom_in_parent_loc = parent_loc;
self.flooder.region_arena[blossom_region.0].blossom_in_child_loc = inner_to_outer_loc;
```

**Step 3: Implement `do_blossom_shattering`**

Replace the placeholder in `src/flooder/graph_flooder.rs`:

```rust
fn do_blossom_shattering(&self, region_idx: RegionIdx) -> MwpmEvent {
    let region = &self.region_arena[region_idx.0];

    let in_parent = region.blossom_in_parent_loc.and_then(|node_idx| {
        self.graph.nodes[node_idx.0 as usize]
            .heir_region_on_shatter(self.region_arena.items())
    });

    let in_child = region.blossom_in_child_loc.and_then(|node_idx| {
        self.graph.nodes[node_idx.0 as usize]
            .heir_region_on_shatter(self.region_arena.items())
    });

    match (in_parent, in_child) {
        (Some(ip), Some(ic)) => MwpmEvent::BlossomShatter {
            blossom: region_idx,
            in_parent: ip,
            in_child: ic,
        },
        _ => MwpmEvent::NoEvent,
    }
}
```

**Step 4: Run existing tests**
```
cargo test
```
Expected: all 130 tests pass.

**Step 5: Commit**
```bash
git add src/flooder/fill_region.rs src/flooder/graph_flooder.rs src/matcher/mwpm.rs
git commit -m "feat: implement do_blossom_shattering — emit BlossomShatter event"
```

---

### Task 3: Implement `pair_and_shatter_subblossoms`

**Files:**
- Modify: `src/matcher/mwpm.rs`

**Context:** PyMatching C++ (`mwpm.cc:303-327`):
```cpp
GraphFillRegion *Mwpm::pair_and_shatter_subblossoms_and_extract_matches(
    GraphFillRegion *region, MatchingResult &res) {
    for (auto &r : region->blossom_children) {
        r.region->clear_blossom_parent_ignoring_wrapped_radius();
    }
    auto subblossom = region->match.edge.loc_from->region_that_arrived_top;
    subblossom->match = region->match;
    if (subblossom->match.region)
        subblossom->match.region->match.region = subblossom;
    res.weight += region->radius.y_intercept();
    auto iter = find subblossom in blossom_children;
    size_t index = distance to it;
    size_t num_children = region->blossom_children.size();
    for (size_t i = 0; i < num_children - 1; i += 2) {
        auto &re1 = region->blossom_children[(index + i + 1) % num_children];
        auto &re2 = region->blossom_children[(index + i + 2) % num_children];
        re1.region->add_match(re2.region, re1.edge);
        res += shatter_blossom_and_extract_matches(re1.region);
    }
    flooder.region_arena.del(region);
    return subblossom;
}
```

**Algorithm in plain English:**
1. Clear blossom parent on all children (they are now standalone)
2. Find which blossom child `subblossom` contains `match.edge.loc_from` (via `region_that_arrived_top` on that detector node)
3. Transfer the blossom's outer match to `subblossom`
4. Find `subblossom`'s index in the blossom children list
5. Starting from the child AFTER subblossom, pair up children in consecutive pairs: `(index+1, index+2)`, `(index+3, index+4)`, ...
6. For each pair, match them together and recursively shatter
7. Free the blossom region, return `subblossom`

**Step 1: Add `clear_blossom_parent` helper to GraphFillRegion**

In `src/flooder/fill_region.rs`:
```rust
pub fn clear_blossom_parent(&mut self) {
    self.blossom_parent = None;
    self.blossom_parent_top = None;
}
```

**Step 2: Replace the stub in `mwpm.rs`**

```rust
fn pair_and_shatter_subblossoms(
    &mut self,
    region: RegionIdx,
    res: &mut MatchingResult,
) -> RegionIdx {
    // 1. Clear blossom parent on all children
    let children: Vec<RegionEdge> = self.flooder.region_arena[region.0].blossom_children.clone();
    for child in &children {
        self.flooder.region_arena[child.region.0].blossom_parent = None;
        self.flooder.region_arena[child.region.0].blossom_parent_top = None;
    }

    // 2. Find which child owns the match edge's loc_from node
    let match_edge = self.flooder.region_arena[region.0].match_.as_ref().unwrap().edge;
    let subblossom = match_edge.loc_from
        .and_then(|node_idx| self.flooder.graph.nodes[node_idx.0 as usize].region_that_arrived_top)
        .expect("match edge loc_from must have a region");

    // 3. Transfer the blossom's match to subblossom
    let blossom_match = self.flooder.region_arena[region.0].match_.clone().unwrap();
    self.flooder.region_arena[subblossom.0].match_ = Some(Match {
        region: blossom_match.region,
        edge: blossom_match.edge,
    });
    if let Some(other) = blossom_match.region {
        self.flooder.region_arena[other.0].match_ = Some(Match {
            region: Some(subblossom),
            edge: blossom_match.edge.reversed(),
        });
    }

    // 4. Accumulate blossom radius weight
    res.weight += self.flooder.region_arena[region.0].radius.y_intercept();

    // 5. Find subblossom index in children
    let index = children.iter().position(|c| c.region == subblossom)
        .expect("subblossom must be in blossom_children");
    let num_children = children.len();

    // 6. Pair up remaining children in consecutive pairs starting after subblossom
    let mut i = 0;
    while i < num_children - 1 {
        let re1 = &children[(index + i + 1) % num_children];
        let re2 = &children[(index + i + 2) % num_children];
        let r1 = re1.region;
        let r2 = re2.region;
        let e = re1.edge;
        self.flooder.region_arena[r1.0].match_ = Some(Match { region: Some(r2), edge: e });
        self.flooder.region_arena[r2.0].match_ = Some(Match { region: Some(r1), edge: e.reversed() });
        let sub_res = self.shatter_blossom_and_extract_matches(r1);
        *res += sub_res;
        i += 2;
    }

    // 7. Free the blossom region and return subblossom
    self.flooder.region_arena.free(region.0);
    subblossom
}
```

**Step 3: Run existing tests**
```
cargo test
```
Expected: all 130 tests pass.

**Step 4: Commit**
```bash
git add src/matcher/mwpm.rs src/flooder/fill_region.rs
git commit -m "feat: implement pair_and_shatter_subblossoms — correct sub-blossom match extraction"
```

---

### Task 4: Add blossom-specific unit tests

**Files:**
- Modify: `tests/matcher.rs`

**Context:** Add tests that actually trigger blossom shattering and verify correct observable predictions.

**Step 1: Add triangle blossom test that verifies prediction**

In `tests/matcher.rs`, add:

```rust
#[test]
fn mwpm_blossom_then_match_4_events() {
    use rmatching::Matching;
    // 4-node graph: 0-1-2 triangle + node 3 connected to 0
    // D0 connects to D1 (L0), D1 connects to D2 (no obs), D2 connects to D0 (no obs)
    // D0 has a boundary edge (L0)
    // Fire D0, D1, D2, D3: triangle forces blossom, then blossom shatters to match D3
    let dem = "error(0.1) D0 D1 L0\nerror(0.1) D1 D2\nerror(0.1) D0 D2\nerror(0.1) D0\n";
    let mut m = Matching::from_dem(dem).unwrap();
    // All 3 detectors firing: odd cycle = blossom must form
    let pred = m.decode(&[1, 1, 1]);
    // Result should be valid (not panic), exact value depends on matching
    assert_eq!(pred.len(), 1);
}

#[test]
fn mwpm_blossom_decode_chain_5() {
    use rmatching::Matching;
    // 5-node chain: 0-1-2-3-4, with L0 on edge 0-1 and L1 on edge 2-3
    // Boundary edges at 0 and 4
    let dem = concat!(
        "error(0.1) D0 D1 L0\n",
        "error(0.1) D1 D2\n",
        "error(0.1) D2 D3 L1\n",
        "error(0.1) D3 D4\n",
        "error(0.1) D0\n",
        "error(0.1) D4\n",
    );
    let mut m = Matching::from_dem(dem).unwrap();
    // Fire D0+D1: edge match → L0 flipped
    let pred = m.decode(&[1, 1, 0, 0, 0]);
    assert_eq!(pred, vec![1, 0]);
    // Fire D2+D3: edge match → L1 flipped
    let pred = m.decode(&[0, 0, 1, 1, 0]);
    assert_eq!(pred, vec![0, 1]);
    // Empty syndrome
    let pred = m.decode(&[0, 0, 0, 0, 0]);
    assert_eq!(pred, vec![0, 0]);
}
```

**Step 2: Run the new tests**
```
cargo test mwpm_blossom
```
Expected: pass. If they reveal bugs in the blossom shattering, fix the code.

**Step 3: Commit**
```bash
git add tests/matcher.rs
git commit -m "test: add blossom shattering unit tests"
```

---

### Task 5: Add CLI binary

**Files:**
- Create: `src/bin/rmatching_cli.rs`

**Purpose:** Allows the Python cross-validation script to call rmatching without PyO3.

**Step 1: Create the binary**

```rust
// src/bin/rmatching_cli.rs
//! rmatching CLI: decode syndromes from a DEM file
//!
//! Usage: rmatching_cli <dem_file>
//! Stdin:  one syndrome per line, space-separated 0/1 per detector
//! Stdout: one prediction per line, space-separated 0/1 per observable

use rmatching::Matching;
use std::io::{self, BufRead, Write};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: rmatching_cli <dem_file>");
        std::process::exit(1);
    }

    let dem_text = std::fs::read_to_string(&args[1])
        .unwrap_or_else(|e| { eprintln!("Failed to read DEM file: {e}"); std::process::exit(1); });

    let mut matching = Matching::from_dem(&dem_text)
        .unwrap_or_else(|e| { eprintln!("Failed to parse DEM: {e}"); std::process::exit(1); });

    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = io::BufWriter::new(stdout.lock());

    for line in stdin.lock().lines() {
        let line = line.unwrap();
        let line = line.trim();
        if line.is_empty() { continue; }

        let syndrome: Vec<u8> = line.split_whitespace()
            .map(|s| s.parse::<u8>().expect("syndrome values must be 0 or 1"))
            .collect();

        let pred = matching.decode(&syndrome);
        let pred_str: Vec<String> = pred.iter().map(|b| b.to_string()).collect();
        writeln!(out, "{}", pred_str.join(" ")).unwrap();
    }
}
```

**Step 2: Build it**
```
cargo build --bin rmatching_cli
```
Expected: compiles successfully.

**Step 3: Quick smoke test**
```bash
echo "error(0.1) D0 D1 L0" > /tmp/test.dem
echo "1 1" | cargo run --bin rmatching_cli -- /tmp/test.dem
```
Expected output: `1`

**Step 4: Commit**
```bash
git add src/bin/rmatching_cli.rs
git commit -m "feat: add rmatching_cli binary for cross-validation"
```

---

### Task 6: Add Python cross-validation script

**Files:**
- Create: `tests/cross_validate.py`

**Prerequisites:** `pip install pymatching stim` on the local machine.

**Step 1: Create the script**

```python
#!/usr/bin/env python3
"""Cross-validate rmatching against PyMatching on random syndromes."""
import subprocess
import sys
import random
import tempfile
import os

def make_rep_code_dem(d: int, p: float) -> str:
    """Generate a distance-d repetition code DEM string."""
    lines = []
    for i in range(d - 1):
        lines.append(f"error({p}) D{i} D{i+1} L0")
    lines.append(f"error({p}) D0")
    lines.append(f"error({p}) D{d-2}")
    return "\n".join(lines) + "\n"

def decode_with_rmatching(dem_text: str, syndromes: list) -> list:
    """Decode syndromes using rmatching CLI binary."""
    with tempfile.NamedTemporaryFile(mode='w', suffix='.dem', delete=False) as f:
        f.write(dem_text)
        dem_path = f.name
    try:
        stdin_data = "\n".join(
            " ".join(str(b) for b in s) for s in syndromes
        ) + "\n"
        result = subprocess.run(
            ["cargo", "run", "--bin", "rmatching_cli", "--quiet", "--", dem_path],
            input=stdin_data, capture_output=True, text=True, check=True
        )
        predictions = []
        for line in result.stdout.strip().split("\n"):
            if line.strip():
                predictions.append([int(x) for x in line.strip().split()])
        return predictions
    finally:
        os.unlink(dem_path)

def decode_with_pymatching(dem_text: str, syndromes: list) -> list:
    """Decode syndromes using PyMatching."""
    import pymatching
    m = pymatching.Matching()
    for line in dem_text.strip().split("\n"):
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        parts = line.split()
        if parts[0] != "error":
            continue
        p = float(parts[1].strip("()"))
        dets = [int(x[1:]) for x in parts[2:] if x.startswith("D")]
        obs  = [int(x[1:]) for x in parts[2:] if x.startswith("L")]
        weight = -__import__('math').log(p / (1 - p)) if p != 0.5 else 0
        if len(dets) == 2:
            m.add_edge(dets[0], dets[1], fault_ids=obs, weight=weight, error_probability=p)
        elif len(dets) == 1:
            m.add_boundary_edge(dets[0], fault_ids=obs, weight=weight, error_probability=p)
    predictions = []
    for s in syndromes:
        pred = m.decode(s)
        predictions.append(list(pred))
    return predictions

def main():
    random.seed(42)
    NUM_SYNDROMES = 1000
    D = 5
    P = 0.1
    NUM_DETS = D - 1

    print(f"Cross-validating rmatching vs PyMatching: rep code d={D}, p={P}, {NUM_SYNDROMES} syndromes")

    dem_text = make_rep_code_dem(D, P)
    syndromes = [
        [random.randint(0, 1) for _ in range(NUM_DETS)]
        for _ in range(NUM_SYNDROMES)
    ]

    print("Decoding with rmatching...")
    rm_preds = decode_with_rmatching(dem_text, syndromes)

    print("Decoding with PyMatching...")
    pm_preds = decode_with_pymatching(dem_text, syndromes)

    assert len(rm_preds) == len(pm_preds) == NUM_SYNDROMES, \
        f"Length mismatch: rm={len(rm_preds)}, pm={len(pm_preds)}"

    mismatches = 0
    first_mismatch = None
    for i, (rm, pm) in enumerate(zip(rm_preds, pm_preds)):
        if rm != pm:
            mismatches += 1
            if first_mismatch is None:
                first_mismatch = (i, syndromes[i], rm, pm)

    if mismatches == 0:
        print(f"✅ PASS: all {NUM_SYNDROMES} syndromes match!")
        sys.exit(0)
    else:
        print(f"❌ FAIL: {mismatches}/{NUM_SYNDROMES} mismatches")
        if first_mismatch:
            idx, syn, rm, pm = first_mismatch
            print(f"  First mismatch at index {idx}:")
            print(f"    syndrome:    {syn}")
            print(f"    rmatching:   {rm}")
            print(f"    pymatching:  {pm}")
        sys.exit(1)

if __name__ == "__main__":
    main()
```

**Step 2: Run it locally** (requires `pip install pymatching stim`)
```
python3 tests/cross_validate.py
```
Expected: `✅ PASS: all 1000 syndromes match!`

If there are failures, debug the blossom shattering code before proceeding.

**Step 3: Commit**
```bash
git add tests/cross_validate.py
git commit -m "test: add Python cross-validation script against PyMatching"
```

---

### Task 7: Add ignored Rust test that runs the cross-validation script

**Files:**
- Create: `tests/cross_validate.rs`

**Step 1: Create the test**

```rust
// tests/cross_validate.rs
//! Cross-validation tests against PyMatching.
//! Run with: cargo test -- --include-ignored cross_validate

#[test]
#[ignore = "requires pymatching and stim Python packages"]
fn cross_validate_rep_code_d5() {
    let status = std::process::Command::new("python3")
        .arg("tests/cross_validate.py")
        .status()
        .expect("failed to run python3 tests/cross_validate.py");
    assert!(status.success(), "cross_validate.py reported mismatches — see output above");
}
```

**Step 2: Run normally (should be skipped)**
```
cargo test cross_validate
```
Expected: `1 test ignored`

**Step 3: Run with --include-ignored (requires pymatching)**
```
cargo test -- --include-ignored cross_validate
```
Expected: `✅ PASS` from the Python script

**Step 4: Commit**
```bash
git add tests/cross_validate.rs
git commit -m "test: add ignored cross-validation Rust test"
```

---

### Task 8: Push all changes

**Step 1: Verify everything passes**
```
cargo test
```
Expected: all tests pass (130+).

**Step 2: Push**
```
git push origin main
```
