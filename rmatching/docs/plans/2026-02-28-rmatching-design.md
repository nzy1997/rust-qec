# rmatching Design Document

A Rust port of PyMatching's Sparse Blossom MWPM decoder for quantum error correction.

## Goals

- Full port of the Sparse Blossom algorithm (single-pass MWPM, no correlated matching in MVP)
- Standalone crate with own DEM parser + manual graph builder
- Negative weight edge support
- APIs: `decode()`, `decode_batch()`, `decode_to_edges()`
- rsinter `Decoder` trait impl behind `rsinter` feature flag (lives in rmatching)

## Module Structure

```
rmatching/src/
├── lib.rs                    # Public API re-exports
├── util/
│   ├── varying.rs            # Varying<T> — time-varying values
│   ├── radix_heap.rs         # RadixHeapQueue — O(1) priority queue
│   └── arena.rs              # Arena<T> — index-based allocation
├── flooder/
│   ├── detector_node.rs      # DetectorNode — graph nodes
│   ├── graph.rs              # MatchingGraph — weighted detector graph
│   ├── fill_region.rs        # GraphFillRegion — growing/shrinking regions
│   └── graph_flooder.rs      # GraphFlooder — region flooding orchestrator
├── matcher/
│   ├── mwpm.rs               # Mwpm — main solver
│   └── alt_tree.rs           # AltTreeNode — alternating tree
├── interop/
│   ├── event.rs              # MwpmEvent enum
│   ├── compressed_edge.rs    # CompressedEdge
│   └── region_edge.rs        # RegionEdge, Match
├── search/
│   ├── search_graph.rs       # SearchGraph — Dijkstra path extraction
│   └── search_flooder.rs     # SearchFlooder
├── driver/
│   ├── user_graph.rs         # UserGraph — user-facing graph builder
│   ├── dem_parse.rs          # DEM text parser (standalone)
│   └── decoding.rs           # decode(), decode_batch(), decode_to_edges()
└── decoder.rs                # rsinter Decoder trait (feature-gated)
```

## Core Data Structures

### Index Types (replacing C++ raw pointers)

```rust
pub struct NodeIdx(u32);
pub struct RegionIdx(u32);
pub struct AltTreeIdx(u32);
```

All graph nodes, regions, and tree nodes live in `Vec`s accessed by index.
`Option<Idx>` replaces nullable pointers.

### Varying<T>

Encodes slope in top 2 bits, y-intercept in remaining bits.
Slope: +1 (growing), 0 (frozen), -1 (shrinking).
Used for region radii that change over time.

### DetectorNode

```rust
pub struct DetectorNode {
    // Permanent (graph structure)
    pub neighbors: Vec<NodeIdx>,
    pub neighbor_weights: Vec<u32>,
    pub neighbor_observables: Vec<u64>,
    // Ephemeral (reset between decodes)
    pub region_that_arrived: Option<RegionIdx>,
    pub region_that_arrived_top: Option<RegionIdx>,
    pub reached_from_source: Option<NodeIdx>,
    pub observables_crossed_from_source: u64,
    pub radius_of_arrival: i64,
    pub node_event_tracker: QueuedEventTracker,
}
```

### GraphFillRegion

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

### CompressedEdge

```rust
pub struct CompressedEdge {
    pub loc_from: Option<NodeIdx>,
    pub loc_to: Option<NodeIdx>,   // None = boundary
    pub obs_mask: u64,
}
```

### MwpmEvent

```rust
pub enum MwpmEvent {
    NoEvent,
    RegionHitRegion { region1: RegionIdx, region2: RegionIdx, edge: CompressedEdge },
    RegionHitBoundary { region: RegionIdx, edge: CompressedEdge },
    BlossomShatter { blossom: RegionIdx, in_parent: RegionIdx, in_child: RegionIdx },
}
```

### RadixHeapQueue

33 buckets indexed by MSB of `(event_time XOR current_time)`.
O(1) amortized enqueue/dequeue with monotonic time.

### Integer Types

| C++ type | Rust type | Purpose |
|----------|-----------|---------|
| `obs_int` (u64) | `u64` | Observable bit masks |
| `weight_int` (u32) | `u32` | Edge weights |
| `signed_weight_int` (i32) | `i32` | Potentially negative weights |
| `cumulative_time_int` (i64) | `i64` | Absolute times/distances |
| `total_weight_int` (i64) | `i64` | Total solution weight |
| `cyclic_time_int` | `Wrapping<u32>` | Timestamps near current time |

## Algorithm Flow

### Phase A — Flooding (GraphFlooder)

1. Create detection events at fired detector nodes
2. Initialize regions (one per event), radius = growing from 0
3. Main loop:
   - Dequeue next event from radix heap
   - LOOK_AT_NODE: region arrived at node → check neighbors, schedule collisions
   - LOOK_AT_SHRINKING_REGION: blossom shrank to zero → emit BlossomShatter
   - Pass event to Mwpm matcher

### Phase B — Matching (Mwpm)

Process each MwpmEvent:
- RegionHitRegion: same tree → form blossom; different trees → match roots
- RegionHitBoundary: match region to boundary
- BlossomShatter: destroy blossom, redistribute children

### Phase C — Path Extraction (SearchFlooder)

For each matched pair: Dijkstra on SearchGraph → XOR observables along path → prediction.

## Negative Weight Handling

For edges with `p > 0.5` (negative weight `ln((1-p)/p)`):
1. Flip weight to positive, XOR observables into global set, mark endpoints
2. Before decode: flip syndrome bits at marked nodes
3. After decode: XOR negative_weight_observables into prediction

## Public API

```rust
pub struct Matching { /* ... */ }

impl Matching {
    // Construction
    pub fn from_dem(dem_text: &str) -> Result<Self, String>;
    pub fn new() -> Self;
    pub fn add_edge(&mut self, n1: usize, n2: usize, weight: f64,
                    observables: &[usize], error_probability: f64);
    pub fn add_boundary_edge(&mut self, node: usize, weight: f64,
                             observables: &[usize], error_probability: f64);
    pub fn set_boundary(&mut self, boundary: &[usize]);

    // Decoding
    pub fn decode(&mut self, syndrome: &[u8]) -> Vec<u8>;
    pub fn decode_batch(&mut self, syndromes: &[Vec<u8>]) -> Vec<Vec<u8>>;
    pub fn decode_to_edges(&mut self, syndrome: &[u8]) -> Vec<(i64, i64)>;
}
```

`Matching` owns a `UserGraph` and lazily builds the `Mwpm` solver on first decode.
Subsequent decodes reuse the solver, resetting ephemeral state only.

### rsinter Integration (feature-gated)

```rust
#[cfg(feature = "rsinter")]
pub struct MwpmDecoder;
// Implements rsinter::decode::Decoder
// Implements rsinter::decode::CompiledDecoder
```

## DEM Parser

Standalone parser (~200 lines) for the DEM text subset:
- `error(p) D<i> [D<j>] [L<k>...]` — edges
- `detector D<i>` — declare detector
- `repeat N { ... }` — repeated blocks with detector index shifting
- `^` separator — ignored in non-correlated mode

## Testing Strategy

1. Unit tests: Varying arithmetic, radix heap, region growth, blossom formation
2. Decode correctness: known graphs with expected predictions
3. Cross-validation: same DEMs + syndromes through PyMatching and rmatching
