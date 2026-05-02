# rmatching

[![CI](https://github.com/nzy1997/rmatching/actions/workflows/ci.yml/badge.svg)](https://github.com/nzy1997/rmatching/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/nzy1997/rmatching/branch/main/graph/badge.svg)](https://codecov.io/gh/nzy1997/rmatching)

A Rust implementation of the Sparse Blossom minimum-weight perfect matching (MWPM) decoder for quantum error correction, ported from [PyMatching](https://github.com/oscarhiggott/PyMatching).

## Features

- Full Sparse Blossom algorithm with alternating trees and blossom contraction/shattering
- Standalone DEM (Detector Error Model) text parser — no external dependencies
- Negative edge weight support
- Decode API: `decode`, `decode_batch`, `decode_to_edges`
- Optional [rsinter](https://github.com/nzy1997/rstim) `Decoder` trait integration behind `rsinter` feature flag

## Quick Start

```rust
use rmatching::Matching;

// From a DEM string
let mut m = Matching::from_dem("error(0.1) D0 D1 L0\nerror(0.1) D0\nerror(0.1) D1\n").unwrap();
let prediction = m.decode(&[1, 1]);
assert_eq!(prediction, vec![1]);

// Or build manually
let mut m = Matching::new();
m.add_edge(0, 1, 2.2, &[0], 0.1);
m.add_boundary_edge(0, 2.2, &[0], 0.1);
m.add_boundary_edge(1, 2.2, &[], 0.1);
let prediction = m.decode(&[1, 0]);
```

## Architecture

| Module | Description |
|--------|-------------|
| `util` | Varying (time-varying values), Arena (index-based allocator), RadixHeapQueue |
| `flooder` | DetectorNode, MatchingGraph, GraphFillRegion, GraphFlooder |
| `matcher` | AltTreeNode (alternating trees), Mwpm (MWPM solver) |
| `search` | SearchGraph, SearchFlooder (bidirectional Dijkstra path extraction) |
| `interop` | CompressedEdge, MwpmEvent, FloodCheckEvent, QueuedEventTracker |
| `driver` | UserGraph, DEM parser, Matching (public decode API) |
| `decoder` | rsinter `Decoder` trait impl (feature-gated) |

## Benchmark Snapshot

Snapshot date: 2026-03-14. These numbers come from a fresh local rerun of
[benchmarks/minimal_results.csv](benchmarks/minimal_results.csv)
and
[benchmarks/surface_dem_results.csv](benchmarks/surface_dem_results.csv)
at commit `33faf6c`.
The CSV files are overwritten on each benchmark run, so treat this section as a
point-in-time snapshot instead of a stable baseline.

### Minimal DEM Cases

| DEM | Accuracy | rmatching mean decode | PyMatching mean decode | Notes |
|-----|----------|-----------------------:|-----------------------:|-------|
| `boundary-2` | `100%` match, `0` mismatches | `1.203 us` | `1.450 us` | rmatching slightly faster |
| `square-4` | `100%` match, `0` mismatches | `10.397 us` | `14.538 us` | rmatching faster |
| `blossom-3` | `100%` match, `0` mismatches | `3.369 us` | `7.863 us` | rmatching faster |

### Surface-Code DEM Cases

| DEM | Accuracy | rmatching mean decode | PyMatching mean decode | Notes |
|-----|----------|-----------------------:|-----------------------:|-------|
| `surface-d5-p0.001` | `100%` match, `0` mismatches | `65.319 us` | `11.208 us` | rmatching slower |
| `surface-d17-p0.001` | `100%` match, `0` mismatches | `1943.961 us` | `552.951 us` | rmatching slower |

## Running Tests And Benchmarks

### Rust Test Suite

Run the full Rust suite:

```bash
cargo test
```

List available Rust tests:

```bash
cargo test -- --list
```

### Benchmark Prerequisites

The Python benchmark drivers compare `rmatching` against PyMatching and require:

```bash
python3 -m pip install numpy pymatching stim
```

Build the benchmark binary first:

```bash
cargo build --release --features bench --bin rmatching_microbench
```

### Minimal DEM Benchmark Suite

Runs the small hand-written DEM cases and writes:
- `benchmarks/minimal_results.csv`
- `benchmarks/minimal_mismatches.json`

Command:

```bash
python3 benchmarks/run_minimal_benchmark.py --warmup-rounds 20 --measure-rounds 80
```

### Surface-Code DEM Benchmark Suite

Runs the fixed surface-code DEM cases (`d=5` and `d=17` by default) and writes:
- `benchmarks/surface_dem_results.csv`
- `benchmarks/surface_dem_mismatches.json`

Command:

```bash
python3 benchmarks/run_surface_dem_benchmark.py --shots 64 --seed 12345 --warmup-rounds 10 --measure-rounds 30
```

### Benchmark Driver Unit Tests

The Python harness itself has unit tests:

```bash
python3 -m unittest benchmarks.test_run_minimal_benchmark -v
python3 -m unittest benchmarks.test_run_surface_dem_benchmark -v
```

The fixed `d17` regression check used during performance work is:

```bash
python3 -m unittest benchmarks.test_run_surface_dem_benchmark.RunSurfaceDemBenchmarkTest.test_rmatching_decodes_known_d17_regression_syndrome -v
```

## License

MIT
