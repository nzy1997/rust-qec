# Benchmark Design: rstim+rmatching vs stim+PyMatching

## Goal

Compare two end-to-end decoding pipelines on the pre-existing rotated surface code circuits from `PyMatching/benchmarks/surface_codes/` (4 error rates × 9 code distances, 10,000 shots each):

| Pipeline | Sampler | Decoder |
|----------|---------|---------|
| Baseline | stim (Python) | PyMatching (Python) |
| Rust | rstim (Rust, local workspace) | rmatching (Rust) |

**Metrics per circuit:**
- `decode_us_per_round`: `decode_total_seconds × 1e6 / (num_shots × d)` — microseconds per shot-round, matching the units in the existing `pymatching_v2.csv` data
- `logical_error_rate`: fraction of shots where any observable prediction ≠ true observable flip

## Circuits

Use the 36 pre-existing `.stim` files in:
```
PyMatching/benchmarks/surface_codes/
  surface_code_rotated_memory_x_p_{p}_d_{d}_..._both_bases/
    surface_code_rotated_memory_x_p_{p}_d_{d}.stim
```
Error rates `p ∈ {0.001, 0.005, 0.008, 0.01}`, distances `d ∈ {5, 7, 9, 13, 17, 23, 29, 39, 50}`.

## Components

### 1. Rust binary: `rmatching_bench`

**File**: `src/bin/rmatching_bench.rs`

**CLI**: `rmatching_bench <stim_file> <num_shots>`

**Internal pipeline:**
```
read .stim file
  → rstim::parser::parse()                         // Vec<StimInstr>
  → ErrorAnalyzer::circuit_to_dem()                // DetectorErrorModel
  → dem.to_string()                                // DEM text
  → Matching::from_dem()                           // build decoder (setup, not timed)
  → rstim::sampler::sample_batch()                 // detections + observable_flips BitTables (not timed)
  → convert detections BitTable → Vec<Vec<u8>>     // syndromes
  → [START TIMER] matching.decode_batch()          // MWPM decode
  → [STOP TIMER]
  → compare predictions vs observable_flips        // logical error rate
  → stdout: CSV line
```

**Stdout format** (one line, no header):
```
rmatching,{p},{d},{decode_us_per_round:.4},{logical_error_rate:.6}
```

`p` and `d` are parsed from the filename with a simple regex.

**Cargo.toml change**: add `rstim` as a non-optional path dependency:
```toml
[dependencies]
rstim = { path = "../rstim" }
```
(The existing optional git-dep `rstim` under `[features]` remains for the `rsinter` feature; this is a separate non-optional entry.)

### 2. Python driver: `benchmarks/run_benchmark.py`

**Location**: `benchmarks/run_benchmark.py` (new directory at repo root)

**Python dependencies**: `stim`, `pymatching`, `numpy`

**Algorithm:**
```python
for stim_file in sorted(glob("PyMatching/benchmarks/surface_codes/**/*.stim")):
    p, d = parse_filename(stim_file)

    # PyMatching baseline
    circuit = stim.Circuit.from_file(stim_file)
    dem = circuit.detector_error_model(decompose_errors=True)
    matcher = pymatching.Matching.from_detector_error_model(dem)
    sampler = circuit.compile_detector_sampler()
    detections, obs_flips = sampler.sample(num_shots, separate_observables=True)
    t0 = perf_counter()
    predictions = matcher.decode_batch(detections)
    decode_s = perf_counter() - t0
    logical_err_rate = np.any(predictions != obs_flips, axis=1).mean()
    decode_us_per_round = decode_s * 1e6 / (num_shots * d)
    emit row: ("pymatching", p, d, decode_us_per_round, logical_err_rate)

    # rmatching (via cargo run --release)
    result = subprocess.run(
        ["cargo", "run", "--release", "--bin", "rmatching_bench", "--", stim_file, str(num_shots)],
        capture_output=True, text=True
    )
    emit row: parse CSV line from result.stdout

write benchmarks/results.csv
print formatted table to stdout
```

**Output CSV columns**: `decoder,p,d,decode_us_per_round,logical_error_rate`

**Printed table example:**
```
decoder       p       d    decode_us/round   logical_err_rate
pymatching    0.001    5         0.4200           0.002300
rmatching     0.001    5         0.1800           0.002280
...
```

## File Layout

```
rmatching/
├── benchmarks/
│   ├── run_benchmark.py        # Python driver (new)
│   └── results.csv             # Generated output (gitignored)
├── src/
│   └── bin/
│       └── rmatching_bench.rs  # Rust benchmark binary (new)
└── Cargo.toml                  # Add rstim path dep (modified)
```

## Notes

- **Timing scope**: Only `decode_batch()` is timed for both pipelines. Sampling and setup are excluded.
- **rstim decompose_errors**: `ErrorAnalyzer::circuit_to_dem()` does not have a `decompose_errors` option. If rmatching fails on hyperedges from the surface code DEM, the benchmark must detect this and report an error.
- **Cargo run latency**: `cargo run --release` will recompile only on first run. On repeated runs it is fast. This is acceptable given the user chose this approach.
- **Shots**: Fixed at 10,000 to match the `pymatching_v2.csv` methodology.
