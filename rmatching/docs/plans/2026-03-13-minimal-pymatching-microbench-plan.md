# Minimal PyMatching Microbenchmark Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a minimal DEM-only benchmark that compares `rmatching` against `PyMatching` on tiny hand-written cases, reporting decode-time summaries and exact prediction agreement.

**Architecture:** A Python driver owns the benchmark cases, exhaustive syndrome generation, PyMatching reference decode, artifact writing, and orchestration. A new Rust CLI accepts a JSON request containing a DEM and syndrome batch, decodes them with `rmatching`, and returns predictions plus decode/build timing summaries as JSON. Existing `rmatching_cli` remains unchanged.

**Tech Stack:** Rust 2024, `rmatching`, Python 3 stdlib `unittest`, `pymatching`, `numpy`, `serde`, `serde_json`

---

### Task 1: Define the minimal benchmark cases and output artifact names

**Files:**
- Create: `benchmarks/__init__.py`
- Create: `benchmarks/minimal_cases.py`
- Create: `benchmarks/test_minimal_cases.py`
- Modify: `benchmarks/.gitignore`

**Step 1: Write the failing Python tests**

Create `benchmarks/test_minimal_cases.py`:

```python
import unittest

from benchmarks.minimal_cases import build_cases


class MinimalCasesTest(unittest.TestCase):
    def test_case_names_and_sizes_are_stable(self):
        cases = build_cases()
        self.assertEqual([case.name for case in cases], [
            "boundary-2",
            "square-4",
            "blossom-3",
        ])
        self.assertEqual([case.num_detectors for case in cases], [2, 4, 3])
        self.assertEqual([case.num_edges for case in cases], [3, 9, 6])

    def test_syndromes_are_exhaustive(self):
        cases = {case.name: case for case in build_cases()}
        self.assertEqual(len(cases["boundary-2"].syndromes), 4)
        self.assertEqual(len(cases["square-4"].syndromes), 16)
        self.assertEqual(len(cases["blossom-3"].syndromes), 8)
        self.assertEqual(cases["square-4"].syndromes[0], [0, 0, 0, 0])
        self.assertEqual(cases["square-4"].syndromes[-1], [1, 1, 1, 1])


if __name__ == "__main__":
    unittest.main()
```

**Step 2: Run the tests to verify they fail**

Run:

```bash
python3 -m unittest benchmarks.test_minimal_cases -v
```

Expected: import failure because `benchmarks.minimal_cases` does not exist yet.

**Step 3: Implement the case definitions**

Create `benchmarks/minimal_cases.py`:

```python
from dataclasses import dataclass
from itertools import product


@dataclass(frozen=True)
class MinimalCase:
    name: str
    dem: str
    num_detectors: int
    num_edges: int
    syndromes: list[list[int]]


def exhaustive_syndromes(num_detectors: int) -> list[list[int]]:
    return [list(bits) for bits in product((0, 1), repeat=num_detectors)]


def build_cases() -> list[MinimalCase]:
    return [
        MinimalCase(
            name="boundary-2",
            dem=(
                "error(0.1) D0 D1\n"
                "error(0.05) D0\n"
                "error(0.05) D1\n"
            ),
            num_detectors=2,
            num_edges=3,
            syndromes=exhaustive_syndromes(2),
        ),
        MinimalCase(
            name="square-4",
            dem=(
                "error(0.1) D0 D1\n"
                "error(0.1) D2 D3\n"
                "error(0.1) D0 D2\n"
                "error(0.1) D1 D3\n"
                "error(0.1) D0 D3 L0\n"
                "error(0.05) D0\n"
                "error(0.05) D1\n"
                "error(0.05) D2\n"
                "error(0.05) D3\n"
            ),
            num_detectors=4,
            num_edges=9,
            syndromes=exhaustive_syndromes(4),
        ),
        MinimalCase(
            name="blossom-3",
            dem=(
                "error(0.1) D0 D1\n"
                "error(0.1) D1 D2\n"
                "error(0.1) D0 D2 L0\n"
                "error(0.05) D0\n"
                "error(0.05) D1\n"
                "error(0.05) D2\n"
            ),
            num_detectors=3,
            num_edges=6,
            syndromes=exhaustive_syndromes(3),
        ),
    ]
```

Create `benchmarks/__init__.py` as an empty file so `python3 -m unittest benchmarks.test_minimal_cases` imports cleanly.

Update `benchmarks/.gitignore` to:

```gitignore
results.csv
minimal_results.csv
minimal_mismatches.json
```

**Step 4: Run the tests again**

Run:

```bash
python3 -m unittest benchmarks.test_minimal_cases -v
```

Expected: `OK`.

**Step 5: Commit**

```bash
git add benchmarks/__init__.py benchmarks/minimal_cases.py benchmarks/test_minimal_cases.py benchmarks/.gitignore
git commit -m "test: define minimal microbenchmark cases"
```

### Task 2: Add a Rust JSON microbenchmark binary with pure helper tests

**Files:**
- Modify: `Cargo.toml`
- Create: `src/bin/rmatching_microbench.rs`

**Step 1: Write failing Rust tests for request handling and timing summaries**

Create `src/bin/rmatching_microbench.rs` with tests first:

```rust
#[cfg(feature = "bench")]
mod bench {
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Deserialize)]
    struct BenchmarkRequest {
        dem: String,
        syndromes: Vec<Vec<u8>>,
        warmup_rounds: usize,
        measure_rounds: usize,
    }

    #[derive(Debug, Serialize)]
    struct BenchmarkResponse {
        predictions: Vec<Vec<u8>>,
        build_us: f64,
        decode_latencies_us: Vec<f64>,
        mean_decode_us: f64,
        median_decode_us: f64,
        p95_decode_us: f64,
    }

    fn summarize_latencies(_samples: &[f64]) -> (f64, f64, f64) {
        todo!()
    }

    fn run_request(_req: BenchmarkRequest) -> BenchmarkResponse {
        todo!()
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn summarize_latencies_basic() {
            let (mean, median, p95) = summarize_latencies(&[4.0, 1.0, 3.0, 2.0, 5.0]);
            assert_eq!(mean, 3.0);
            assert_eq!(median, 3.0);
            assert_eq!(p95, 5.0);
        }

        #[test]
        fn run_request_decodes_square_case() {
            let req = BenchmarkRequest {
                dem: "error(0.1) D0 D1\nerror(0.1) D2 D3\nerror(0.1) D0 D2\nerror(0.1) D1 D3\nerror(0.1) D0 D3 L0\nerror(0.05) D0\nerror(0.05) D1\nerror(0.05) D2\nerror(0.05) D3\n".to_string(),
                syndromes: vec![vec![1, 0, 0, 1], vec![1, 1, 0, 0]],
                warmup_rounds: 1,
                measure_rounds: 3,
            };
            let resp = run_request(req);
            assert_eq!(resp.predictions, vec![vec![1], vec![0]]);
            assert_eq!(resp.decode_latencies_us.len(), 3);
            assert!(resp.build_us >= 0.0);
            assert!(resp.mean_decode_us >= 0.0);
        }
    }
}

#[cfg(not(feature = "bench"))]
fn main() {}

#[cfg(feature = "bench")]
fn main() {}
```

**Step 2: Run the tests to verify they fail**

Run:

```bash
cargo test --features bench --bin rmatching_microbench
```

Expected: compile failure because `serde` / `serde_json` are not available yet, or `todo!()` panics once dependencies are added.

**Step 3: Add the minimal dependencies and implement the helpers**

Modify `Cargo.toml`:

```toml
[features]
rsinter = ["dep:rsinter", "dep:rstim"]
bench = ["dep:rstim", "dep:rand", "dep:serde", "dep:serde_json"]

[dependencies]
rsinter = { git = "https://github.com/nzy1997/rstim.git", optional = true }
rstim = { git = "https://github.com/nzy1997/rstim.git", optional = true }
rand = { version = "0.8", optional = true }
serde = { version = "1", features = ["derive"], optional = true }
serde_json = { version = "1", optional = true }
```

Implement `summarize_latencies` by sorting a local copy, taking the arithmetic mean, middle element median, and `ceil(0.95 * len) - 1` percentile index.

Implement `run_request` by:

- timing `Matching::from_dem(&req.dem)` once for `build_us`
- decoding the full syndrome batch once to capture `predictions`
- warming up with `warmup_rounds`
- timing each measured full-batch decode loop and storing one microsecond sample per round
- returning summary fields derived from the measured batch latencies

**Step 4: Run the tests again**

Run:

```bash
cargo test --features bench --bin rmatching_microbench
```

Expected: `test result: ok`.

**Step 5: Commit**

```bash
git add Cargo.toml src/bin/rmatching_microbench.rs
git commit -m "feat: add JSON microbenchmark binary for rmatching"
```

### Task 3: Finish the Rust CLI main path and JSON I/O

**Files:**
- Modify: `src/bin/rmatching_microbench.rs`

**Step 1: Write a failing serialization test**

Add this test inside `src/bin/rmatching_microbench.rs`:

```rust
#[test]
fn response_serializes_predictions_and_stats() {
    let req = BenchmarkRequest {
        dem: "error(0.1) D0 D1\nerror(0.05) D0\nerror(0.05) D1\n".to_string(),
        syndromes: vec![vec![0, 0], vec![1, 1]],
        warmup_rounds: 0,
        measure_rounds: 1,
    };
    let resp = run_request(req);
    let json = serde_json::to_string(&resp).unwrap();
    assert!(json.contains("\"predictions\""));
    assert!(json.contains("\"mean_decode_us\""));
}
```

**Step 2: Run the test to confirm the current binary plumbing is incomplete**

Run:

```bash
cargo test --features bench --bin rmatching_microbench response_serializes_predictions_and_stats
```

Expected: fail until the final `main()` path and serialization are wired cleanly.

**Step 3: Implement stdin/stdout JSON plumbing**

Replace the feature-gated `main()` with:

```rust
#[cfg(feature = "bench")]
fn main() {
    use bench::{run_request, BenchmarkRequest};
    use std::io::{self, Read};

    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    let req: BenchmarkRequest = serde_json::from_str(&input).unwrap_or_else(|e| {
        eprintln!("Failed to parse benchmark request JSON: {e}");
        std::process::exit(1);
    });
    let resp = run_request(req);
    println!("{}", serde_json::to_string(&resp).unwrap());
}
```

Keep the non-bench `main()` as:

```rust
#[cfg(not(feature = "bench"))]
fn main() {
    eprintln!("Build with --features bench to use rmatching_microbench");
    std::process::exit(1);
}
```

**Step 4: Verify with tests and one manual invocation**

Run:

```bash
cargo test --features bench --bin rmatching_microbench
```

Run:

```bash
python3 - <<'PY'
import json, subprocess
req = {
    "dem": "error(0.1) D0 D1\nerror(0.05) D0\nerror(0.05) D1\n",
    "syndromes": [[0, 0], [1, 1]],
    "warmup_rounds": 0,
    "measure_rounds": 2,
}
res = subprocess.run(
    ["cargo", "run", "--quiet", "--features", "bench", "--bin", "rmatching_microbench"],
    input=json.dumps(req),
    text=True,
    capture_output=True,
    check=True,
)
payload = json.loads(res.stdout)
assert payload["predictions"] == [[0], [0]]
assert len(payload["decode_latencies_us"]) == 2
PY
```

Expected: no output and exit code `0`.

**Step 5: Commit**

```bash
git add src/bin/rmatching_microbench.rs
git commit -m "feat: wire JSON I/O for rmatching microbenchmark binary"
```

### Task 4: Add the Python benchmark driver and pure summary tests

**Files:**
- Create: `benchmarks/run_minimal_benchmark.py`
- Create: `benchmarks/test_run_minimal_benchmark.py`

**Step 1: Write failing tests for summary logic**

Create `benchmarks/test_run_minimal_benchmark.py`:

```python
import unittest

from benchmarks.run_minimal_benchmark import summarize_case


class RunMinimalBenchmarkTest(unittest.TestCase):
    def test_summarize_case_reports_match_rate_and_mismatches(self):
        row, mismatches = summarize_case(
            case_name="square-4",
            num_detectors=4,
            num_edges=9,
            py_predictions=[[1], [0], [1]],
            rm_predictions=[[1], [1], [1]],
            rm_stats={
                "build_us": 10.0,
                "mean_decode_us": 2.0,
                "median_decode_us": 2.0,
                "p95_decode_us": 3.0,
            },
            syndromes=[[1, 0, 0, 1], [1, 1, 0, 0], [0, 0, 0, 0]],
        )
        self.assertEqual(row["prediction_match_rate"], 2 / 3)
        self.assertEqual(row["mismatch_cases"], 1)
        self.assertEqual(mismatches[0]["syndrome"], [1, 1, 0, 0])
```

**Step 2: Run the tests to verify they fail**

Run:

```bash
python3 -m unittest benchmarks.test_run_minimal_benchmark -v
```

Expected: import failure because the driver does not exist yet.

**Step 3: Implement the driver with pure helper functions first**

Create `benchmarks/run_minimal_benchmark.py` with:

- `run_pymatching(case)` that builds `pymatching.Matching` from the DEM text and decodes the exhaustive syndrome matrix
- `run_rmatching(case, warmup_rounds, measure_rounds)` that sends JSON over stdin to `target/release/rmatching_microbench` or `cargo run --release --features bench --bin rmatching_microbench`
- `summarize_case(...)` that returns one CSV row plus a mismatch list
- `write_outputs(rows, mismatches, results_path, mismatch_path)` for artifacts
- `main()` that loops over `build_cases()`

Use CSV columns:

```python
CSV_HEADER = [
    "case_name",
    "decoder",
    "num_detectors",
    "num_edges",
    "num_syndromes_tested",
    "prediction_match_rate",
    "mismatch_cases",
    "build_us",
    "mean_decode_us",
    "median_decode_us",
    "p95_decode_us",
]
```

Write artifacts to:

- `benchmarks/minimal_results.csv`
- `benchmarks/minimal_mismatches.json`

The driver should emit two rows per case:

- one `pymatching` row with the case metadata and blank timing columns
- one `rmatching` row with the case metadata, match rate, mismatch count, and timing stats

**Step 4: Run the tests again**

Run:

```bash
python3 -m unittest benchmarks.test_run_minimal_benchmark -v
```

Expected: `OK`.

**Step 5: Commit**

```bash
git add benchmarks/run_minimal_benchmark.py benchmarks/test_run_minimal_benchmark.py
git commit -m "feat: add minimal PyMatching microbenchmark driver"
```

### Task 5: Add an end-to-end smoke test and full verification run

**Files:**
- Create: `tests/minimal_microbench.rs`

**Step 1: Add an ignored smoke test**

Create `tests/minimal_microbench.rs`:

```rust
#[test]
#[ignore = "requires pymatching Python package and release bench binary"]
fn minimal_microbenchmark_smoke() {
    let status = std::process::Command::new("python3")
        .arg("benchmarks/run_minimal_benchmark.py")
        .arg("--warmup-rounds")
        .arg("2")
        .arg("--measure-rounds")
        .arg("5")
        .status()
        .expect("failed to run minimal benchmark driver");
    assert!(status.success(), "minimal benchmark driver failed");
}
```

**Step 2: Run the normal Rust and Python tests**

Run:

```bash
cargo test
```

Run:

```bash
python3 -m unittest benchmarks.test_minimal_cases benchmarks.test_run_minimal_benchmark -v
```

Expected: both pass.

**Step 3: Build the release binary and run the smoke test manually**

Run:

```bash
cargo build --release --features bench --bin rmatching_microbench
python3 benchmarks/run_minimal_benchmark.py --warmup-rounds 5 --measure-rounds 20
```

Expected:

- `benchmarks/minimal_results.csv` exists
- `benchmarks/minimal_mismatches.json` exists
- every `rmatching` row reports `prediction_match_rate` of `1.0`

**Step 4: Optionally run the ignored smoke test**

Run:

```bash
cargo test -- --ignored minimal_microbenchmark_smoke
```

Expected: pass if Python dependencies are installed.

**Step 5: Commit**

```bash
git add tests/minimal_microbench.rs benchmarks/minimal_results.csv benchmarks/minimal_mismatches.json
git reset benchmarks/minimal_results.csv benchmarks/minimal_mismatches.json
git commit -m "test: add minimal microbenchmark smoke coverage"
```
