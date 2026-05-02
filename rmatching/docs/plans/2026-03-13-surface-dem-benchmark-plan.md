# Surface DEM Benchmark Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a decode-only benchmark that compares `rmatching` against `PyMatching` on fixed rotated surface-code DEMs for `d=5` and `d=17`.

**Architecture:** A new Python case module will select two existing `.stim` circuits, derive decomposed DEM text, and sample a deterministic batch of syndromes outside the timed section. A new Python driver will reuse the existing `rmatching_microbench` JSON path for Rust timing, run `PyMatching.decode_batch` on the same fixed inputs, compare predictions exactly, and write isolated result artifacts.

**Tech Stack:** Python 3 stdlib `unittest`, `stim`, `pymatching`, `numpy`, existing Rust `rmatching_microbench`

---

### Task 1: Define deterministic surface-code DEM benchmark cases

**Files:**
- Create: `benchmarks/surface_dem_cases.py`
- Create: `benchmarks/test_surface_dem_cases.py`
- Modify: `benchmarks/.gitignore`

**Step 1: Write the failing tests**

Add tests that assert:
- case names are exactly `surface-d5-p0.001` and `surface-d17-p0.001`
- each case has non-empty DEM text and a stable `num_detectors`
- syndrome batches are deterministic when built twice
- sampled syndrome count matches the configured shot count

**Step 2: Run the tests to verify they fail**

Run:

```bash
python3 -m unittest benchmarks.test_surface_dem_cases -v
```

Expected: import failure because `benchmarks.surface_dem_cases` does not exist yet.

**Step 3: Write the minimal implementation**

Implement a `SurfaceDemCase` dataclass plus helpers that:
- locate the selected `d=5` and `d=17` `.stim` files under `PyMatching/benchmarks/surface_codes`
- load the circuits with `stim`
- derive decomposed DEM text
- sample `shots` detector events and observable flips using `compile_detector_sampler(seed=...)`
- convert sampled numpy arrays into plain Python integer lists for reuse by the benchmark driver

Use a single fixed seed constant and a small default shot count suitable for repeated local timing.

**Step 4: Run the tests to verify they pass**

Run:

```bash
python3 -m unittest benchmarks.test_surface_dem_cases -v
```

Expected: `OK`.

### Task 2: Add a decode-only benchmark driver for the fixed surface-code DEM cases

**Files:**
- Create: `benchmarks/run_surface_dem_benchmark.py`
- Create: `benchmarks/test_run_surface_dem_benchmark.py`

**Step 1: Write the failing tests**

Add tests for:
- `summarize_case` reporting exact match rate and mismatch counts
- `write_outputs` producing the expected CSV columns for the new artifact names
- any small helper that normalizes observable predictions or converts bool arrays into `0/1`

**Step 2: Run the tests to verify they fail**

Run:

```bash
python3 -m unittest benchmarks.test_run_surface_dem_benchmark -v
```

Expected: import failure because `benchmarks.run_surface_dem_benchmark` does not exist yet.

**Step 3: Write the minimal implementation**

Implement a driver that:
- imports the fixed cases from `benchmarks.surface_dem_cases`
- builds a PyMatching matcher from each case’s DEM text
- calls the existing `rmatching_microbench` binary with the same DEM and syndrome batch
- times only the PyMatching `decode_batch` call
- compares predictions against both PyMatching reference observables and `rmatching`
- writes `benchmarks/surface_dem_results.csv` and `benchmarks/surface_dem_mismatches.json`

Include enough metadata in the CSV to identify `case_name`, `d`, `p`, `num_detectors`, `num_syndromes_tested`, `prediction_match_rate`, and decode timing summaries.

**Step 4: Run the tests to verify they pass**

Run:

```bash
python3 -m unittest benchmarks.test_run_surface_dem_benchmark -v
```

Expected: `OK`.

### Task 3: Verify the benchmark end to end

**Files:**
- Test: `tests/minimal_microbench.rs`

**Step 1: Run focused Python tests**

Run:

```bash
python3 -m unittest benchmarks.test_surface_dem_cases benchmarks.test_run_surface_dem_benchmark -v
```

Expected: `OK`.

**Step 2: Build the release microbenchmark binary**

Run:

```bash
cargo build --release --features bench --bin rmatching_microbench
```

Expected: exit code 0.

**Step 3: Run the new benchmark**

Run:

```bash
python3 benchmarks/run_surface_dem_benchmark.py --warmup-rounds 5 --measure-rounds 20
```

Expected:
- `benchmarks/surface_dem_results.csv` exists
- `benchmarks/surface_dem_mismatches.json` exists
- mismatch file is empty or contains explicit disagreement records

**Step 4: Run the full Rust and Python regression set touched by this change**

Run:

```bash
cargo test
python3 -m unittest benchmarks.test_minimal_cases benchmarks.test_run_minimal_benchmark benchmarks.test_surface_dem_cases benchmarks.test_run_surface_dem_benchmark -v
```

Expected: `OK`.
