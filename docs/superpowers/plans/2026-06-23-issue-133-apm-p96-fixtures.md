# P96 APM Sparse Fixtures Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Generate and validate pinned P=96 APM-CSS `Hx` and `Hz` sparse-row fixtures for the `[[1152,580,<=12]]` Table A1 instance.

**Architecture:** Keep the reproducible generator fixture-local under `qec-code/tests/fixtures/apm/`; it reads the existing Table A1 manifest and writes compact `sparse_rows` JSON. Keep validation test-local in `qec-code/tests/code.rs`, using qec-code's existing sparse-row parser and public binary rank helper without adding a production APM API.

**Tech Stack:** Rust 2024, qec-code, serde_json, Python 3 standard library, Cargo integration tests.

## Global Constraints

- Read the P=96 entry from `qec-code/tests/fixtures/apm/table_a1_manifest.json`.
- Write fixtures at exactly `qec-code/tests/fixtures/apm/p96_hx.json` and `qec-code/tests/fixtures/apm/p96_hz.json`.
- Fixture JSON format must be `{"format":"sparse_rows","num_cols":1152,"rows":[...]}`.
- Use the Appendix A block pattern: `Hx` has three circulant retained rows of `F0..F5,G0..G5`; `Hz` has three transposed reverse-circulant retained rows of `G0..G5,F0..F5`.
- For an affine map `m(x) = ax + b mod P`, represent a non-transposed block row by the support column `m(x)` and represent a transposed block row by the inverse affine map.
- Do not expose a built-in `apm_kasai` code id or public APM construction API.
- Verification must include the negative in-memory mutation control from issue #133.
- Run `cargo test -p qec-code apm_p96_fixture_matches_reference_stats -q`.
- Run `cargo test`.

---

## File Structure

- Create `qec-code/tests/fixtures/apm/generate_p96_fixtures.py`: manifest-driven fixture generator and `--check` drift gate.
- Create `qec-code/tests/fixtures/apm/p96_hx.json`: generated P=96 `Hx` sparse-row fixture.
- Create `qec-code/tests/fixtures/apm/p96_hz.json`: generated P=96 `Hz` sparse-row fixture.
- Modify `qec-code/tests/code.rs`: add fixture verifier helpers plus positive and negative P=96 tests.

### Task 1: P96 APM Fixture Generator, Fixtures, And Verifier

**Files:**
- Create: `qec-code/tests/fixtures/apm/generate_p96_fixtures.py`
- Create: `qec-code/tests/fixtures/apm/p96_hx.json`
- Create: `qec-code/tests/fixtures/apm/p96_hz.json`
- Modify: `qec-code/tests/code.rs`

**Interfaces:**
- Consumes: `qec-code/tests/fixtures/apm/table_a1_manifest.json`.
- Produces: generator command `python3 qec-code/tests/fixtures/apm/generate_p96_fixtures.py`.
- Produces: check command `python3 qec-code/tests/fixtures/apm/generate_p96_fixtures.py --check`.
- Produces: Rust test `apm_p96_fixture_matches_reference_stats`.
- Produces: Rust test `apm_p96_fixture_rejects_mutated_support`.

- [x] **Step 1: Add failing Rust fixture tests first**

Modify `qec-code/tests/code.rs`:

1. Add `binary::binary_rank` and `sparse_rows_matrix_from_json_str` to the imports, preserving the existing `CssCode` import used by other tests in this file:

```rust
use qec_code::binary::binary_rank;
use qec_code::css::{sparse_rows_matrix_from_json_str, CssCode, SparseRowsMatrix};
```

2. Add these helpers near the existing APM manifest helpers, after `validate_apm_table_a1_manifest`:

```rust
#[derive(Debug, Clone)]
struct ApmSparseFixture {
    num_cols: usize,
    rows: Vec<Vec<usize>>,
}

fn load_apm_sparse_fixture(input: &str) -> ApmSparseFixture {
    let matrix = sparse_rows_matrix_from_json_str(input).unwrap();
    ApmSparseFixture {
        num_cols: matrix.num_cols(),
        rows: matrix.rows().to_vec(),
    }
}

fn column_weights(rows: &[Vec<usize>], num_cols: usize) -> Vec<usize> {
    let mut weights = vec![0; num_cols];
    for row in rows {
        for &col in row {
            weights[col] += 1;
        }
    }
    weights
}

fn assert_apm_p96_fixture_stats(
    hx: &ApmSparseFixture,
    hz: &ApmSparseFixture,
) -> std::result::Result<(), String> {
    if hx.num_cols != 1152 || hz.num_cols != 1152 {
        return Err(format!(
            "expected both matrices to have 1152 columns, got Hx={} Hz={}",
            hx.num_cols, hz.num_cols
        ));
    }
    if hx.rows.len() != 288 || hz.rows.len() != 288 {
        return Err(format!(
            "expected both matrices to have 288 rows, got Hx={} Hz={}",
            hx.rows.len(), hz.rows.len()
        ));
    }
    for (name, rows) in [("Hx", hx.rows.as_slice()), ("Hz", hz.rows.as_slice())] {
        if let Some((row_index, row)) = rows.iter().enumerate().find(|(_, row)| row.len() != 12) {
            return Err(format!(
                "{name} row {row_index} has weight {}, expected 12",
                row.len()
            ));
        }
    }
    for (name, matrix) in [("Hx", hx), ("Hz", hz)] {
        let weights = column_weights(&matrix.rows, matrix.num_cols);
        if let Some((col, weight)) = weights.iter().enumerate().find(|(_, weight)| **weight != 3) {
            return Err(format!(
                "{name} column {col} has weight {weight}, expected 3"
            ));
        }
    }
    for (x_index, x_row) in hx.rows.iter().enumerate() {
        let x_support = x_row.iter().copied().collect::<HashSet<_>>();
        for (z_index, z_row) in hz.rows.iter().enumerate() {
            let overlap = z_row
                .iter()
                .filter(|&&col| x_support.contains(&col))
                .count();
            if overlap % 2 != 0 {
                return Err(format!(
                    "Hx row {x_index} and Hz row {z_index} overlap with odd parity {overlap}"
                ));
            }
        }
    }

    let hx_dense = dense_rows(&hx.rows, hx.num_cols);
    let hz_dense = dense_rows(&hz.rows, hz.num_cols);
    let rank_x = binary_rank(&hx_dense);
    let rank_z = binary_rank(&hz_dense);
    if rank_x + rank_z != 572 {
        return Err(format!(
            "expected rank_x + rank_z == 572, got {rank_x} + {rank_z} = {}",
            rank_x + rank_z
        ));
    }
    let logical_qubits = hx.num_cols - rank_x - rank_z;
    if logical_qubits != 580 {
        return Err(format!("expected k = 580, got {logical_qubits}"));
    }
    Ok(())
}
```

3. Add the tests near the existing APM manifest tests:

```rust
#[test]
fn apm_p96_fixture_matches_reference_stats() {
    let hx = load_apm_sparse_fixture(include_str!("fixtures/apm/p96_hx.json"));
    let hz = load_apm_sparse_fixture(include_str!("fixtures/apm/p96_hz.json"));

    assert_apm_p96_fixture_stats(&hx, &hz).unwrap();
}

#[test]
fn apm_p96_fixture_rejects_mutated_support() {
    let hx = load_apm_sparse_fixture(include_str!("fixtures/apm/p96_hx.json"));
    let mut hz = load_apm_sparse_fixture(include_str!("fixtures/apm/p96_hz.json"));
    let replacement = (0..hz.num_cols)
        .find(|candidate| !hz.rows[0].contains(candidate))
        .unwrap();
    hz.rows[0][0] = replacement;
    hz.rows[0].sort_unstable();

    let err = assert_apm_p96_fixture_stats(&hx, &hz).unwrap_err();
    assert!(
        err.contains("column") || err.contains("overlap") || err.contains("rank"),
        "mutating one support should trip a structural verifier, got: {err}"
    );
}
```

- [x] **Step 2: Run focused test and verify RED**

Run:

```bash
cargo test -p qec-code apm_p96_fixture_matches_reference_stats -q
```

Expected: FAIL because `qec-code/tests/fixtures/apm/p96_hx.json` and `qec-code/tests/fixtures/apm/p96_hz.json` do not exist yet.

- [x] **Step 3: Add the fixture generator**

Create `qec-code/tests/fixtures/apm/generate_p96_fixtures.py` with:

```python
#!/usr/bin/env python3
"""Generate pinned P=96 APM sparse-row fixtures from table_a1_manifest.json.

Regenerate:
    python3 qec-code/tests/fixtures/apm/generate_p96_fixtures.py

Check committed fixtures:
    python3 qec-code/tests/fixtures/apm/generate_p96_fixtures.py --check
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path


SCRIPT_DIR = Path(__file__).resolve().parent
MANIFEST_PATH = SCRIPT_DIR / "table_a1_manifest.json"
HX_PATH = SCRIPT_DIR / "p96_hx.json"
HZ_PATH = SCRIPT_DIR / "p96_hz.json"


def affine_inverse(a: int, b: int, modulus: int) -> tuple[int, int]:
    for candidate in range(modulus):
        if (a * candidate) % modulus == 1:
            return candidate, (-candidate * b) % modulus
    raise ValueError(f"{a} is not invertible modulo {modulus}")


def apply_affine(coefficients: tuple[int, int], value: int, modulus: int) -> int:
    a, b = coefficients
    return (a * value + b) % modulus


def affine_family(entry: dict[str, object], key: str) -> list[tuple[int, int]]:
    family = entry[key]
    if not isinstance(family, list):
        raise ValueError(f"{key} must be an array")
    if len(family) != 6:
        raise ValueError(f"{key} must contain 6 affine maps")
    coefficients: list[tuple[int, int]] = []
    for expected_index, item in enumerate(family):
        if not isinstance(item, dict):
            raise ValueError(f"{key}[{expected_index}] must be an object")
        if item.get("i") != expected_index:
            raise ValueError(f"{key}[{expected_index}].i must be {expected_index}")
        if key == "f":
            coefficients.append((int(item["a"]), int(item["b"])))
        else:
            coefficients.append((int(item["c"]), int(item["d"])))
    return coefficients


def p96_entry() -> dict[str, object]:
    manifest = json.loads(MANIFEST_PATH.read_text(encoding="utf-8"))
    for entry in manifest["entries"]:
        if entry["code_id"] == "apm_kasai:p=96":
            if entry["P"] != 96 or entry["J"] != 3 or entry["L"] != 12 or entry["L2"] != 6:
                raise ValueError("apm_kasai:p=96 manifest dimensions changed")
            shape = entry["expected_code_shape"]
            if shape["n"] != 1152 or shape["mx"] != 288 or shape["mz"] != 288 or shape["k"] != 580:
                raise ValueError("apm_kasai:p=96 expected shape changed")
            return entry
    raise ValueError("missing apm_kasai:p=96 entry")


def build_hx_rows(p: int, f: list[tuple[int, int]], g: list[tuple[int, int]]) -> list[list[int]]:
    rows: list[list[int]] = []
    for block_row in range(3):
        for local_row in range(p):
            row: list[int] = []
            for block_col in range(12):
                family = f if block_col < 6 else g
                family_index = (block_col % 6 - block_row) % 6
                local_col = apply_affine(family[family_index], local_row, p)
                row.append(block_col * p + local_col)
            rows.append(sorted(row))
    return rows


def build_hz_rows(p: int, f: list[tuple[int, int]], g: list[tuple[int, int]]) -> list[list[int]]:
    rows: list[list[int]] = []
    inverse_f = [affine_inverse(a, b, p) for a, b in f]
    inverse_g = [affine_inverse(a, b, p) for a, b in g]
    for block_row in range(3):
        for local_row in range(p):
            row: list[int] = []
            for block_col in range(12):
                family = inverse_g if block_col < 6 else inverse_f
                family_index = (block_row - (block_col % 6)) % 6
                local_col = apply_affine(family[family_index], local_row, p)
                row.append(block_col * p + local_col)
            rows.append(sorted(row))
    return rows


def sparse_rows_json(num_cols: int, rows: list[list[int]]) -> str:
    return json.dumps(
        {"format": "sparse_rows", "num_cols": num_cols, "rows": rows},
        separators=(",", ":"),
    ) + "\n"


def generated_texts() -> tuple[str, str]:
    entry = p96_entry()
    p = int(entry["P"])
    num_cols = int(entry["expected_code_shape"]["n"])
    f = affine_family(entry, "f")
    g = affine_family(entry, "g")
    return (
        sparse_rows_json(num_cols, build_hx_rows(p, f, g)),
        sparse_rows_json(num_cols, build_hz_rows(p, f, g)),
    )


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true", help="fail if committed fixtures differ")
    args = parser.parse_args()

    hx_text, hz_text = generated_texts()
    if args.check:
        mismatches = []
        if not HX_PATH.exists() or HX_PATH.read_text(encoding="utf-8") != hx_text:
            mismatches.append(str(HX_PATH))
        if not HZ_PATH.exists() or HZ_PATH.read_text(encoding="utf-8") != hz_text:
            mismatches.append(str(HZ_PATH))
        if mismatches:
            raise SystemExit("stale generated fixtures: " + ", ".join(mismatches))
        return

    HX_PATH.write_text(hx_text, encoding="utf-8")
    HZ_PATH.write_text(hz_text, encoding="utf-8")


if __name__ == "__main__":
    main()
```

- [x] **Step 4: Generate fixtures and run generator check**

Run:

```bash
python3 qec-code/tests/fixtures/apm/generate_p96_fixtures.py
python3 qec-code/tests/fixtures/apm/generate_p96_fixtures.py --check
```

Expected: first command writes `p96_hx.json` and `p96_hz.json`; second command exits 0 with no output.

- [x] **Step 5: Run focused test and verify GREEN**

Run:

```bash
cargo test -p qec-code apm_p96_fixture_matches_reference_stats -q
```

Expected: PASS. The positive test verifies dimensions, row/column weights, orthogonality, and rank sum.

- [x] **Step 6: Run negative-control test**

Run:

```bash
cargo test -p qec-code apm_p96_fixture_rejects_mutated_support -q
```

Expected: PASS. The test mutates one `Hz` support and verifies the in-memory structural verifier rejects it.

- [x] **Step 7: Format and run required gates**

Run:

```bash
cargo fmt
cargo test -p qec-code apm_p96_fixture_matches_reference_stats -q
cargo test
```

Expected: all commands exit 0. Existing warning-only noise is acceptable only if tests pass.

- [x] **Step 8: Commit implementation**

Run:

```bash
git add qec-code/tests/code.rs \
  qec-code/tests/fixtures/apm/generate_p96_fixtures.py \
  qec-code/tests/fixtures/apm/p96_hx.json \
  qec-code/tests/fixtures/apm/p96_hz.json \
  docs/superpowers/plans/2026-06-23-issue-133-apm-p96-fixtures.md
git commit -m "test: add apm p96 sparse fixtures"
```
