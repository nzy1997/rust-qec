# rmatching Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Port PyMatching's Sparse Blossom MWPM decoder to Rust as a standalone crate.

**Architecture:** Faithful port of PyMatching's C++ core using index-based arenas instead of raw pointers. Six modules: util (Varying, RadixHeap, Arena), flooder (graph, nodes, regions, flooding), matcher (MWPM solver, alternating trees), interop (events, compressed edges), search (Dijkstra path extraction), driver (UserGraph, DEM parser, decode API).

**Tech Stack:** Rust 2024 edition, no external dependencies for core (rsinter optional behind feature flag).

**Reference:** PyMatching source at `./PyMatching/src/pymatching/sparse_blossom/`

---

### Task 1: Project Scaffold + Types

**Files:**
- Create: `Cargo.toml`
- Create: `src/lib.rs`
- Create: `src/types.rs`

**Step 1: Create Cargo.toml**

```toml
[package]
name = "rmatching"
version = "0.1.0"
edition = "2024"

[features]
rsinter = ["dep:rsinter", "dep:rstim"]

[dependencies]
rsinter = { path = "../rsinter", optional = true }
rstim = { path = "../rstim", optional = true }

[dev-dependencies]
```

**Step 2: Create src/types.rs**

Index newtypes replacing C++ raw pointers, plus integer type aliases:

```rust
use std::num::Wrapping;

// Index types — u32 indices into Vec arenas
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeIdx(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RegionIdx(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AltTreeIdx(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SearchNodeIdx(pub u32);

// Integer type aliases matching PyMatching's ints.h
pub type ObsMask = u64;           // obs_int — up to 64 observables
pub type Weight = u32;            // weight_int — edge weights (positive)
pub type SignedWeight = i32;      // signed_weight_int — potentially negative
pub type CumulativeTime = i64;    // cumulative_time_int — absolute times
pub type TotalWeight = i64;       // total_weight_int — solution weight
pub type CyclicTime = Wrapping<u32>; // cyclic_time_int — wrapping timestamps

// Sentinel for "no neighbor"
pub const NO_NEIGHBOR: usize = usize::MAX;
```

**Step 3: Create src/lib.rs with module declarations**

```rust
pub mod types;
pub mod util;
pub mod interop;
pub mod flooder;
pub mod matcher;
pub mod search;
pub mod driver;

#[cfg(feature = "rsinter")]
pub mod decoder;
```

Create empty module files for each submodule (util/mod.rs, flooder/mod.rs, etc.).

**Step 4: Verify `cargo build` passes**

**Step 5: Commit**

```bash
git add -A && git commit -m "feat: project scaffold with types and module structure"
```

---

### Task 2: Util — Varying, Arena, RadixHeap

**Files:**
- Create: `src/util/mod.rs`
- Create: `src/util/varying.rs`
- Create: `src/util/arena.rs`
- Create: `src/util/radix_heap.rs`
- Create: `tests/util.rs`

**Reference:** `PyMatching/src/pymatching/sparse_blossom/flooder_matcher_interop/varying.h` and `tracker/radix_heap_queue.h`

#### 2a: Varying<T>

Bit-packed time-varying value. Top 2 bits encode slope (+1/0/-1), remaining bits encode y-intercept.

```rust
// Slope encoding in bottom 2 bits:
// 0b00 = frozen (slope 0)
// 0b01 = growing (slope +1)
// 0b10 = shrinking (slope -1)

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Varying<T: VaryingInt>(pub T);

pub trait VaryingInt: Copy + ... { ... }
impl VaryingInt for i32 { ... }
impl VaryingInt for i64 { ... }

pub type Varying32 = Varying<i32>;
pub type Varying64 = Varying<i64>;
pub type VaryingCT = Varying<i64>; // cumulative_time_int
```

Key methods to implement (see varying.h for exact logic):
- `get_distance_at_time(time) -> T`
- `time_of_x_intercept() -> T`
- `time_of_x_intercept_when_added_to(other) -> T`
- `is_growing()`, `is_shrinking()`, `is_frozen()`
- `colliding_with(other) -> bool`
- `then_growing_at_time(t)`, `then_shrinking_at_time(t)`, `then_frozen_at_time(t)`
- `growing_varying_with_zero_distance_at_time(t)` (factory)
- `y_intercept() -> T`
- `Add<T>` and `Sub<T>` operators (shift y-intercept)

Tests:
- `varying_growing_at_time`: create growing, check distance increases
- `varying_frozen`: frozen value doesn't change
- `varying_collision_time`: two growing regions, verify intercept
- `varying_state_transitions`: growing → frozen → shrinking

#### 2b: Arena<T>

Simple arena allocator using Vec + free list:

```rust
pub struct Arena<T> {
    items: Vec<T>,
    free: Vec<u32>,
}

impl<T: Default> Arena<T> {
    pub fn new() -> Self;
    pub fn alloc(&mut self) -> u32;       // returns index
    pub fn free(&mut self, idx: u32);
    pub fn get(&self, idx: u32) -> &T;
    pub fn get_mut(&mut self, idx: u32) -> &mut T;
    pub fn clear(&mut self);
}
```

Tests:
- `arena_alloc_free_reuse`: alloc, free, alloc reuses slot

#### 2c: RadixHeapQueue

33-bucket monotonic priority queue. See radix_heap_queue.h for exact logic.

```rust
pub struct RadixHeapQueue {
    buckets: [Vec<FloodCheckEvent>; 33],
    pub cur_time: CumulativeTime,
    num_enqueued: usize,
}
```

Key methods:
- `enqueue(event)` — bucket = bit_width(time XOR cur_time)
- `dequeue() -> FloodCheckEvent` — pop from bucket 0, or redistribute from first non-empty bucket
- `is_empty()`, `len()`, `clear()`, `reset()`

Tests:
- `radix_heap_monotonic_dequeue`: enqueue several events, dequeue in order
- `radix_heap_empty`: dequeue from empty returns NO_EVENT
- `radix_heap_same_time`: multiple events at same time all dequeued

**Commit:** `feat: add Varying, Arena, and RadixHeapQueue utilities`

---

### Task 3: Interop — Events, CompressedEdge, RegionEdge

**Files:**
- Create: `src/interop/mod.rs`
- Create: `src/interop/compressed_edge.rs`
- Create: `src/interop/region_edge.rs`
- Create: `src/interop/event.rs`
- Create: `src/interop/flood_check_event.rs`
- Create: `src/interop/queued_event_tracker.rs`

**Reference:** `flooder_matcher_interop/` directory

```rust
// compressed_edge.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompressedEdge {
    pub loc_from: Option<NodeIdx>,
    pub loc_to: Option<NodeIdx>,  // None = boundary
    pub obs_mask: ObsMask,
}

// region_edge.rs
pub struct RegionEdge {
    pub region: RegionIdx,
    pub edge: CompressedEdge,
}

pub struct Match {
    pub region: Option<RegionIdx>,  // None = boundary match
    pub edge: CompressedEdge,
}

// event.rs — Rust enum replaces C++ union
pub enum MwpmEvent {
    NoEvent,
    RegionHitRegion {
        region1: RegionIdx,
        region2: RegionIdx,
        edge: CompressedEdge,
    },
    RegionHitBoundary {
        region: RegionIdx,
        edge: CompressedEdge,
    },
    BlossomShatter {
        blossom: RegionIdx,
        in_parent: RegionIdx,
        in_child: RegionIdx,
    },
}

// flood_check_event.rs
pub enum FloodCheckEvent {
    NoEvent,
    LookAtNode { node: NodeIdx, time: CyclicTime },
    LookAtShrinkingRegion { region: RegionIdx, time: CyclicTime },
    LookAtSearchNode { node: SearchNodeIdx, time: CyclicTime },
}

// queued_event_tracker.rs
pub struct QueuedEventTracker {
    pub desired_time: CyclicTime,
    pub queued_time: CyclicTime,
    pub has_desired_time: bool,
    pub has_queued_time: bool,
}
```

Implement `QueuedEventTracker::set_desired_event()` and `dequeue_decision()` per PyMatching logic.

Tests:
- `compressed_edge_reversed`: verify reversed() swaps from/to
- `compressed_edge_merged`: verify obs_mask XOR
- `queued_event_tracker_dedup`: set desired, verify only one enqueue

**Commit:** `feat: add interop types (events, compressed edges, tracker)`

---

### Task 4: Flooder — DetectorNode, MatchingGraph, GraphFillRegion

**Files:**
- Create: `src/flooder/mod.rs`
- Create: `src/flooder/detector_node.rs`
- Create: `src/flooder/graph.rs`
- Create: `src/flooder/fill_region.rs`
- Create: `tests/flooder.rs`

**Reference:** `flooder/detector_node.h`, `flooder/graph.h`, `flooder/graph_fill_region.h`

#### 4a: DetectorNode

```rust
pub struct DetectorNode {
    // Permanent (graph structure)
    pub neighbors: Vec<NodeIdx>,          // NodeIdx or BOUNDARY sentinel
    pub neighbor_weights: Vec<Weight>,
    pub neighbor_observables: Vec<ObsMask>,
    // Ephemeral (reset between decodes)
    pub region_that_arrived: Option<RegionIdx>,
    pub region_that_arrived_top: Option<RegionIdx>,
    pub reached_from_source: Option<NodeIdx>,
    pub observables_crossed_from_source: ObsMask,
    pub radius_of_arrival: CumulativeTime,
    pub wrapped_radius_cached: i32,
    pub node_event_tracker: QueuedEventTracker,
}
```

Key methods:
- `local_radius(regions: &[GraphFillRegion]) -> VaryingCT` — returns `region_top.radius + wrapped_radius_cached`
- `compute_wrapped_radius(regions: &[GraphFillRegion]) -> i32` — walk blossom hierarchy
- `has_same_owner_as(other: &DetectorNode) -> bool`
- `reset()` — zero all ephemeral fields

#### 4b: MatchingGraph

```rust
pub struct MatchingGraph {
    pub nodes: Vec<DetectorNode>,
    pub num_observables: usize,
    pub negative_weight_detection_events: Vec<usize>,
    pub negative_weight_observables: Vec<usize>,
    pub negative_weight_obs_mask: ObsMask,
    pub negative_weight_sum: TotalWeight,
    pub is_user_graph_boundary_node: Vec<bool>,
    pub normalising_constant: f64,
}
```

Key methods:
- `add_edge(u, v, weight: SignedWeight, observables: &[usize])` — handles negative weights
- `add_boundary_edge(u, weight: SignedWeight, observables: &[usize])`

#### 4c: GraphFillRegion

```rust
pub struct GraphFillRegion {
    pub blossom_parent: Option<RegionIdx>,
    pub blossom_parent_top: Option<RegionIdx>,
    pub alt_tree_node: Option<AltTreeIdx>,
    pub radius: VaryingCT,
    pub shrink_event_tracker: QueuedEventTracker,
    pub match_: Option<Match>,
    pub blossom_children: Vec<RegionEdge>,
    pub shell_area: Vec<NodeIdx>,
}
```

Key methods:
- `wrap_into_blossom(new_parent: RegionIdx, ...)` — update all descendants
- `cleanup_shell_area()`
- `tree_equal(other, alt_trees)` — check if same alternating tree
- `do_op_for_each_descendant_and_self(callback)` — recursive traversal

Tests:
- `matching_graph_add_edge`: add edges, verify neighbor lists
- `matching_graph_negative_weight`: add negative edge, verify tracking
- `matching_graph_boundary_edge`: add boundary edge, verify sentinel

**Commit:** `feat: add DetectorNode, MatchingGraph, GraphFillRegion`

---

### Task 5: Flooder — GraphFlooder

**Files:**
- Create: `src/flooder/graph_flooder.rs`
- Modify: `src/flooder/mod.rs`
- Create: `tests/graph_flooder.rs`

**Reference:** `flooder/graph_flooder.h` and `graph_flooder.cc`

```rust
pub struct GraphFlooder {
    pub graph: MatchingGraph,
    pub region_arena: Arena<GraphFillRegion>,
    pub queue: RadixHeapQueue,
    pub match_edges: Vec<CompressedEdge>,
    pub negative_weight_detection_events: Vec<u64>,
    pub negative_weight_observables: Vec<usize>,
    pub negative_weight_obs_mask: ObsMask,
    pub negative_weight_sum: TotalWeight,
}
```

Key methods (port from graph_flooder.cc):
- `create_detection_event(node: NodeIdx) -> RegionIdx`
- `run_until_next_mwpm_notification() -> MwpmEvent` — main flooding loop
- `process_tentative_event_returning_mwpm_event(event) -> MwpmEvent`
- `do_look_at_node_event(node) -> MwpmEvent`
- `find_next_event_at_node_returning_neighbor_index_and_time(node) -> (usize, CumulativeTime)`
- `reschedule_events_at_detector_node(node)`
- `set_region_growing(region)`, `set_region_frozen(region)`, `set_region_shrinking(region)`

The flooding loop logic:
1. Dequeue event from radix heap
2. Validate via QueuedEventTracker::dequeue_decision()
3. For LOOK_AT_NODE: check all neighbors for collisions
   - Same owner → skip
   - Boundary → emit RegionHitBoundary
   - Different owner, both growing → collision_time = (weight - rad1 - rad2) / 2
   - Different owner, one frozen → collision_time = weight - rad1 - rad2
4. For LOOK_AT_SHRINKING_REGION: emit BlossomShatter if radius hits zero

Tests:
- `flooder_single_detection_event`: create one event, verify region created
- `flooder_two_events_collide`: two events on adjacent nodes, verify RegionHitRegion
- `flooder_boundary_hit`: event near boundary, verify RegionHitBoundary

**Commit:** `feat: add GraphFlooder with region flooding`

---

### Task 6: Matcher — AltTreeNode + Mwpm

**Files:**
- Create: `src/matcher/mod.rs`
- Create: `src/matcher/alt_tree.rs`
- Create: `src/matcher/mwpm.rs`
- Create: `tests/matcher.rs`

**Reference:** `matcher/alternating_tree.h/.cc` and `matcher/mwpm.h/.cc`

#### 6a: AltTreeNode

```rust
pub struct AltTreeEdge {
    pub alt_tree_node: AltTreeIdx,
    pub edge: CompressedEdge,
}

pub struct AltTreeNode {
    pub inner_region: Option<RegionIdx>,
    pub outer_region: Option<RegionIdx>,
    pub inner_to_outer_edge: CompressedEdge,
    pub parent: Option<AltTreeEdge>,
    pub children: Vec<AltTreeEdge>,
    pub visited: bool,
}
```

Key methods:
- `become_root(arena, flooder)` — tree rotation (see alternating_tree.cc:84-99)
- `most_recent_common_ancestor(other, arena) -> Option<AltTreeIdx>` — LCA via visited flags
- `add_child(child: AltTreeEdge)`
- `prune_upward_path_stopping_before(arena, prune_parent, back)`

#### 6b: Mwpm

```rust
pub struct Mwpm {
    pub flooder: GraphFlooder,
    pub node_arena: Arena<AltTreeNode>,
    pub search_flooder: SearchFlooder,
}
```

Key methods (port from mwpm.cc):
- `create_detection_event(node: NodeIdx)`
- `process_event(event: MwpmEvent)`
- `handle_region_hit_region(event)` — dispatches to same-tree (blossom) or different-tree (match)
- `handle_tree_hitting_other_tree(event)` — become_root both, shatter descendants, match
- `handle_tree_hitting_boundary(event)` — become_root, shatter descendants, match to boundary
- `handle_tree_hitting_same_tree_region(event)` — find LCA, form blossom
- `handle_blossom_shattering(event)`
- `shatter_descendants_into_matches_and_freeze(node)`
- `shatter_blossom_and_extract_matches(region) -> MatchingResult`
- `reset()` — reset all ephemeral state for next decode

Tests:
- `mwpm_two_nodes_match`: two detection events on 2-node graph, verify they match
- `mwpm_boundary_match`: one detection event near boundary, verify boundary match
- `mwpm_blossom_formation`: three detection events forming odd cycle, verify blossom

**Commit:** `feat: add AltTreeNode and Mwpm solver`

---

### Task 7: Search — SearchGraph + SearchFlooder

**Files:**
- Create: `src/search/mod.rs`
- Create: `src/search/search_graph.rs`
- Create: `src/search/search_flooder.rs`
- Create: `tests/search.rs`

**Reference:** `search/search_graph.h/.cc` and `search/search_flooder.h/.cc`

```rust
pub struct SearchDetectorNode {
    pub neighbors: Vec<Option<SearchNodeIdx>>,  // None = boundary
    pub neighbor_weights: Vec<Weight>,
    pub neighbor_observables: Vec<ObsMask>,
    // Ephemeral
    pub reached_from_source: Option<SearchNodeIdx>,
    pub distance_from_source: CumulativeTime,
    pub index_of_predecessor: Option<usize>,
    pub node_event_tracker: QueuedEventTracker,
}

pub struct SearchGraph {
    pub nodes: Vec<SearchDetectorNode>,
    pub num_observables: usize,
}

pub struct SearchFlooder {
    pub graph: SearchGraph,
    pub queue: RadixHeapQueue,
}
```

Key methods:
- `run_until_collision(src, dst) -> SearchGraphEdge` — bidirectional Dijkstra
- `iter_edges_on_shortest_path(src, dst, callback)` — trace path, call callback per edge
- `reset()` — clear ephemeral state

Tests:
- `search_shortest_path`: 3-node chain, verify correct path found
- `search_boundary_path`: path to boundary node

**Commit:** `feat: add SearchGraph and SearchFlooder for path extraction`

---

### Task 8: Driver — UserGraph

**Files:**
- Create: `src/driver/mod.rs`
- Create: `src/driver/user_graph.rs`
- Create: `tests/user_graph.rs`

**Reference:** `driver/user_graph.h/.cc`

```rust
pub struct UserEdge {
    pub node1: usize,
    pub node2: usize,
    pub observable_indices: Vec<usize>,
    pub weight: f64,
    pub error_probability: f64,
}

pub struct UserGraph {
    pub nodes: Vec<UserNode>,
    pub edges: Vec<UserEdge>,
    pub boundary_nodes: HashSet<usize>,
    pub num_observables: usize,
    mwpm: Option<Mwpm>,
}
```

Key methods:
- `new() -> Self`
- `add_edge(n1, n2, observables, weight, error_probability)`
- `add_boundary_edge(node, observables, weight, error_probability)`
- `set_boundary(nodes)`
- `to_matching_graph(num_distinct_weights) -> MatchingGraph`
- `to_search_graph(num_distinct_weights) -> SearchGraph`
- `to_mwpm() -> Mwpm`
- `get_mwpm() -> &mut Mwpm` — lazy init
- `handle_dem_instruction(p, detectors, observables)` — weight = ln((1-p)/p)

Weight discretization: `discretized = round(weight / normalising_constant * 2)` where normalising_constant = max_weight / NUM_DISTINCT_WEIGHTS.

Tests:
- `user_graph_add_edge`: add edge, verify stored
- `user_graph_to_matching_graph`: convert, verify node/edge counts
- `user_graph_dem_instruction`: handle_dem_instruction, verify weight conversion

**Commit:** `feat: add UserGraph with edge management and graph conversion`

---

### Task 9: Driver — DEM Parser

**Files:**
- Create: `src/driver/dem_parse.rs`
- Create: `tests/dem_parse.rs`

Standalone DEM text parser (~200 lines). No rstim dependency.

```rust
pub fn parse_dem(text: &str) -> Result<UserGraph, String>
```

Parses:
- `error(p) D<i> [D<j>] [L<k>...]` → handle_dem_instruction
- `detector D<i>` → ensure node exists
- `repeat N { ... }` → repeat block with detector offset
- Comments `#` and blank lines
- `^` separator → ignored (no correlated matching)

Tests:
- `parse_simple_dem`: `"error(0.1) D0 D1 L0"` → 1 edge
- `parse_boundary_dem`: `"error(0.1) D0 L0"` → boundary edge
- `parse_repeat_dem`: repeat block with shifted indices
- `parse_dem_roundtrip`: parse → decode → verify

**Commit:** `feat: add standalone DEM text parser`

---

### Task 10: Driver — Decode API (Matching struct)

**Files:**
- Create: `src/driver/decoding.rs`
- Modify: `src/lib.rs` (re-export Matching)
- Create: `tests/decode.rs`

**Reference:** `driver/mwpm_decoding.h/.cc`

```rust
pub struct Matching {
    user_graph: UserGraph,
}

impl Matching {
    pub fn from_dem(dem_text: &str) -> Result<Self, String>;
    pub fn new() -> Self;
    pub fn add_edge(&mut self, n1: usize, n2: usize, weight: f64,
                    observables: &[usize], error_probability: f64);
    pub fn add_boundary_edge(&mut self, node: usize, weight: f64,
                             observables: &[usize], error_probability: f64);
    pub fn set_boundary(&mut self, boundary: &[usize]);

    pub fn decode(&mut self, syndrome: &[u8]) -> Vec<u8>;
    pub fn decode_batch(&mut self, syndromes: &[Vec<u8>]) -> Vec<Vec<u8>>;
    pub fn decode_to_edges(&mut self, syndrome: &[u8]) -> Vec<(i64, i64)>;
}
```

The decode flow (from mwpm_decoding.cc):
1. Convert syndrome bytes → detection event indices
2. Handle negative weight flipping
3. Call `process_timeline_until_completion()` — create events, run flooding loop
4. Shatter blossoms, extract observable masks
5. XOR negative weight observables
6. Reset MWPM state
7. Return predictions

Tests:
- `decode_simple_chain`: 3-node chain, fire D0+D1, verify L0 prediction
- `decode_boundary`: single detection near boundary
- `decode_no_errors`: empty syndrome → no observable flips
- `decode_batch_matches_single`: batch results match individual decodes
- `decode_to_edges_simple`: verify matched pairs returned

**Commit:** `feat: add Matching public API with decode/decode_batch/decode_to_edges`

---

### Task 11: rsinter Decoder Integration (feature-gated)

**Files:**
- Create: `src/decoder.rs`
- Create: `tests/decoder.rs`

```rust
#[cfg(feature = "rsinter")]
use rsinter::decode::{Decoder, CompiledDecoder};
use rstim::dem::DetectorErrorModel;

pub struct MwpmDecoder;

struct CompiledMwpmDecoder {
    matching: Matching,
}

impl CompiledDecoder for CompiledMwpmDecoder {
    fn decode_shots_bit_packed(&self, dets: &[u8], num_shots: usize,
                                num_dets: usize, num_obs: usize) -> Vec<u8> {
        // For each shot: unpack dets → syndrome → decode → pack predictions
    }
}

impl Decoder for MwpmDecoder {
    fn compile_for_dem(&self, dem: &DetectorErrorModel) -> Box<dyn CompiledDecoder> {
        let matching = Matching::from_dem(&dem.to_string()).unwrap();
        Box::new(CompiledMwpmDecoder { matching })
    }
}
```

Tests (with `#[cfg(feature = "rsinter")]`):
- `mwpm_decoder_compiles_for_dem`: compile for simple DEM
- `mwpm_decoder_decodes_shots`: decode bit-packed shots, verify predictions

**Commit:** `feat: add rsinter Decoder integration behind feature flag`

---

### Task 12: End-to-End Integration Tests

**Files:**
- Create: `tests/integration.rs`

Tests that exercise the full pipeline:
- `e2e_rep_code_d3`: build rep code DEM manually, decode 1000 syndromes, verify error rate reasonable
- `e2e_surface_code_d3`: build surface code DEM, decode, verify
- `e2e_from_dem_text`: parse DEM text → decode → verify
- `e2e_negative_weights`: DEM with p > 0.5 edges, verify correct handling
- `e2e_decode_to_edges_consistency`: decode and decode_to_edges give consistent results

**Commit:** `feat: add end-to-end integration tests`

---

### Task 13: Cross-Validation with PyMatching (optional)

**Files:**
- Create: `tests/cross_validate.py` (Python script)
- Create: `tests/cross_validate.rs`

Generate test vectors using PyMatching (Python), save as JSON, load in Rust tests and verify identical predictions. This is the strongest correctness check.

**Commit:** `test: add cross-validation test vectors from PyMatching`
