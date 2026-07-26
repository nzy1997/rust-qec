# Issue 555 Deterministic Regular Classical Matrix Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a versioned deterministic generator for regular binary parity-check matrices in `qec-code`.

**Architecture:** A new `regular_classical` module owns the v1 stream, unbiased bounded-index helper, configuration validation, row-slot sampler, canonical row output, and rustdoc-linked algorithm contract. `QecError` receives typed variants for invalid config, stub mismatch, unsupported algorithm version, overflow, and retry exhaustion.

**Tech Stack:** Rust 2024, standard library only, `thiserror` for existing error rendering, Cargo integration tests.

## Global Constraints

- Inputs must include `column_count`, `row_count`, `column_weight`, `row_weight`, `seed`, `algorithm_version`, and `retry_limit`.
- Version 1 must expose repository-owned `SplitMix64V1` and an unbiased bounded-index helper.
- Version 1 must use no external algebra or RNG crate for correctness.
- Stub counts must be checked before sampling and before random words are consumed.
- Zero dimensions, zero weights, zero retry limit, unsupported algorithm versions, mismatched stub counts, and stub-count overflow must return typed errors.
- Retry exhaustion must return a typed exhaustion error.
- The canonical seed-7 fixture for `n=6`, `m=4`, `column_weight=2`, `row_weight=3`, version 1 is `[[0, 1, 2], [0, 3, 4], [1, 3, 5], [2, 4, 5]]`.
- Seed 8 for the same dimensions must produce a different valid matrix.
- The v1 algorithm, versioning policy, stream words, bounded rejection rule, stub ordering, column traversal, duplicate-edge rejection, canonicalization, and retry progression must be documented.
- Required verification commands from issue #555 plus `cargo test -p qec-code` and `cargo test` must run before PR creation.

---

## File Structure

- Create `qec-code/src/regular_classical.rs`: public config, v1 stream, bounded helper, matrix generator, validation helpers, and rustdoc include.
- Create `qec-code/doc/regular_classical.md`: exact algorithm contract and golden stream words.
- Create `qec-code/tests/regular_classical.rs`: integration tests for fixture, invalid inputs, primitive stream/helper behavior, and retry exhaustion.
- Modify `qec-code/src/lib.rs`: export `regular_classical`.
- Modify `qec-code/src/error.rs`: add typed `QecError` variants for this feature.
- Modify `qec-code/README.md`: link and summarize the deterministic regular matrix docs.

---

### Task 1: Public Contract Tests

**Files:**
- Create: `qec-code/tests/regular_classical.rs`

**Interfaces:**
- Consumes: intended public API names from the spec.
- Produces: failing tests that define the public contract for Task 2.

- [ ] **Step 1: Write failing integration tests**

Create `qec-code/tests/regular_classical.rs` with tests named exactly:

```rust
use qec_code::QecError;
use qec_code::regular_classical::{
    REGULAR_CLASSICAL_MATRIX_ALGORITHM_V1, RegularClassicalMatrixConfig, SplitMix64V1,
    bounded_index_v1, deterministic_regular_matrix,
};

fn fixture_config(seed: u64) -> RegularClassicalMatrixConfig {
    RegularClassicalMatrixConfig {
        column_count: 6,
        row_count: 4,
        column_weight: 2,
        row_weight: 3,
        seed,
        algorithm_version: REGULAR_CLASSICAL_MATRIX_ALGORITHM_V1,
        retry_limit: 16,
    }
}

fn assert_regular_degrees(rows: &[Vec<usize>], column_count: usize, row_weight: usize, column_weight: usize) {
    assert_eq!(rows.len(), 4);
    let mut column_degrees = vec![0; column_count];
    for row in rows {
        assert_eq!(row.len(), row_weight);
        let mut sorted = row.clone();
        sorted.sort_unstable();
        assert_eq!(&sorted, row);
        for &column in row {
            assert!(column < column_count);
            column_degrees[column] += 1;
        }
    }
    assert_eq!(column_degrees, vec![column_weight; column_count]);
}

#[test]
fn deterministic_regular_matrix_matches_v1_fixture() {
    let first = deterministic_regular_matrix(fixture_config(7)).unwrap();
    let second = deterministic_regular_matrix(fixture_config(7)).unwrap();
    assert_eq!(first, second);
    assert_eq!(
        first,
        vec![
            vec![0, 1, 2],
            vec![0, 3, 4],
            vec![1, 3, 5],
            vec![2, 4, 5],
        ]
    );
    assert_regular_degrees(&first, 6, 3, 2);

    let seed8 = deterministic_regular_matrix(fixture_config(8)).unwrap();
    assert_ne!(seed8, first);
    assert_regular_degrees(&seed8, 6, 3, 2);
}

#[test]
fn deterministic_regular_matrix_rejects_invalid_degrees() {
    let mut config = fixture_config(7);
    config.column_count = 0;
    assert!(matches!(
        deterministic_regular_matrix(config),
        Err(QecError::InvalidRegularClassicalMatrixConfig { option: "column_count", .. })
    ));

    let mut config = fixture_config(7);
    config.row_count = 0;
    assert!(matches!(
        deterministic_regular_matrix(config),
        Err(QecError::InvalidRegularClassicalMatrixConfig { option: "row_count", .. })
    ));

    let mut config = fixture_config(7);
    config.column_weight = 0;
    assert!(matches!(
        deterministic_regular_matrix(config),
        Err(QecError::InvalidRegularClassicalMatrixConfig { option: "column_weight", .. })
    ));

    let mut config = fixture_config(7);
    config.row_weight = 0;
    assert!(matches!(
        deterministic_regular_matrix(config),
        Err(QecError::InvalidRegularClassicalMatrixConfig { option: "row_weight", .. })
    ));

    let mut config = fixture_config(7);
    config.retry_limit = 0;
    assert!(matches!(
        deterministic_regular_matrix(config),
        Err(QecError::InvalidRegularClassicalMatrixConfig { option: "retry_limit", .. })
    ));

    let mut config = fixture_config(7);
    config.algorithm_version = 2;
    assert!(matches!(
        deterministic_regular_matrix(config),
        Err(QecError::UnsupportedRegularClassicalMatrixAlgorithm { algorithm_version: 2 })
    ));

    let mut config = fixture_config(7);
    config.column_count = 5;
    assert!(matches!(
        deterministic_regular_matrix(config),
        Err(QecError::RegularClassicalMatrixStubCountMismatch {
            column_stubs: 10,
            row_stubs: 12,
        })
    ));
}

#[test]
fn splitmix64_v1_seed7_matches_golden_words() {
    let mut stream = SplitMix64V1::new(7);
    let words = (0..8).map(|_| stream.next_u64()).collect::<Vec<_>>();
    assert_eq!(
        words,
        vec![
            0x63CBE1E459320DD7,
            0x044C3CD7F43C661C,
            0xE6984080BAB12A02,
            0x953AEB70673E29CB,
            0x73D33B666A1E21DA,
            0x3FDABE86CBBEAA11,
            0x77CBC4A133C2D0F6,
            0x53FCD6513D02BEFE,
        ]
    );

    let mut zero_bound_stream = SplitMix64V1::new(7);
    assert_eq!(bounded_index_v1(&mut zero_bound_stream, 0), None);
    assert_eq!(zero_bound_stream.state(), 7);

    let mut bounded = SplitMix64V1::new(7);
    let values = (0..8)
        .map(|_| bounded_index_v1(&mut bounded, 10).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(values, vec![7, 4, 6, 3, 4, 5, 8, 2]);

    let mut rejection = SplitMix64V1::new(7);
    assert_eq!(
        bounded_index_v1(&mut rejection, (1u64 << 63) + 1),
        Some(7_392_729_709_960_833_537)
    );
}

#[test]
fn deterministic_regular_matrix_retry_limit_one_returns_exhausted() {
    let config = RegularClassicalMatrixConfig {
        column_count: 3,
        row_count: 3,
        column_weight: 2,
        row_weight: 2,
        seed: 1,
        algorithm_version: REGULAR_CLASSICAL_MATRIX_ALGORITHM_V1,
        retry_limit: 1,
    };

    assert!(matches!(
        deterministic_regular_matrix(config),
        Err(QecError::RegularClassicalMatrixGenerationExhausted {
            retry_limit: 1,
            attempts: 1,
            ..
        })
    ));
}
```

- [ ] **Step 2: Verify the focused fixture test fails before implementation**

Run:

```bash
cargo test -p qec-code --test regular_classical deterministic_regular_matrix_matches_v1_fixture -- --exact
```

Expected: fails to compile because `qec_code::regular_classical` and the new
error variants do not exist.

- [ ] **Step 3: Commit the failing tests if the repo convention allows**

Do not commit the failing tests alone for this run. They should be committed
with the implementation after green verification.

---

### Task 2: Regular Classical Module Implementation

**Files:**
- Create: `qec-code/src/regular_classical.rs`
- Modify: `qec-code/src/lib.rs`
- Modify: `qec-code/src/error.rs`

**Interfaces:**
- Consumes: tests from Task 1.
- Produces: the `regular_classical` API and typed errors.

- [ ] **Step 1: Add typed errors to `qec-code/src/error.rs`**

Add variants to `QecError`:

```rust
#[error("invalid regular classical matrix option {option}: {reason}")]
InvalidRegularClassicalMatrixConfig {
    option: &'static str,
    reason: String,
},
#[error("unsupported regular classical matrix algorithm version {algorithm_version}")]
UnsupportedRegularClassicalMatrixAlgorithm { algorithm_version: u32 },
#[error("regular classical matrix stub-count overflow for {side}")]
RegularClassicalMatrixStubCountOverflow { side: &'static str },
#[error(
    "regular classical matrix stub-count mismatch: column stubs {column_stubs}, row stubs {row_stubs}"
)]
RegularClassicalMatrixStubCountMismatch {
    column_stubs: usize,
    row_stubs: usize,
},
#[error(
    "regular classical matrix generation exhausted retry limit {retry_limit} after {attempts} attempts for algorithm version {algorithm_version} seed {seed}"
)]
RegularClassicalMatrixGenerationExhausted {
    retry_limit: usize,
    attempts: usize,
    algorithm_version: u32,
    seed: u64,
},
```

- [ ] **Step 2: Create `qec-code/src/regular_classical.rs` implementation**

Implement the exact public API:

```rust
#![doc = include_str!("../doc/regular_classical.md")]

use crate::error::{QecError, Result};

pub const REGULAR_CLASSICAL_MATRIX_ALGORITHM_V1: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegularClassicalMatrixConfig {
    pub column_count: usize,
    pub row_count: usize,
    pub column_weight: usize,
    pub row_weight: usize,
    pub seed: u64,
    pub algorithm_version: u32,
    pub retry_limit: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SplitMix64V1 {
    state: u64,
}

impl SplitMix64V1 {
    pub fn new(seed: u64) -> Self { Self { state: seed } }
    pub fn state(&self) -> u64 { self.state }
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E3779B97F4A7C15);
        let mut value = self.state;
        value = (value ^ (value >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94D049BB133111EB);
        value ^ (value >> 31)
    }
}

pub fn bounded_index_v1(stream: &mut SplitMix64V1, upper_bound: u64) -> Option<u64> {
    if upper_bound == 0 {
        return None;
    }
    let threshold = upper_bound.wrapping_neg() % upper_bound;
    loop {
        let value = stream.next_u64();
        if value >= threshold {
            return Some(value % upper_bound);
        }
    }
}
```

Add validation helpers and generator logic:

```rust
pub fn deterministic_regular_matrix(
    config: RegularClassicalMatrixConfig,
) -> Result<Vec<Vec<usize>>> {
    validate_config(config)?;
    let mut stream = SplitMix64V1::new(config.seed);
    for attempt in 1..=config.retry_limit {
        if let Some(rows) = try_regular_matrix_attempt(config, &mut stream)? {
            return Ok(rows);
        }
        if attempt == config.retry_limit {
            return Err(QecError::RegularClassicalMatrixGenerationExhausted {
                retry_limit: config.retry_limit,
                attempts: attempt,
                algorithm_version: config.algorithm_version,
                seed: config.seed,
            });
        }
    }
    unreachable!("retry_limit is validated to be nonzero");
}
```

`try_regular_matrix_attempt` must build row slots in ascending row order,
iterate columns ascending, skip row slots whose row already appears in the
current column, reject the attempt if no non-duplicate row slot remains, sort
each row, then sort rows lexicographically.

- [ ] **Step 3: Export the module**

Modify `qec-code/src/lib.rs`:

```rust
pub mod regular_classical;
```

- [ ] **Step 4: Run Task 1 tests**

Run:

```bash
cargo test -p qec-code --test regular_classical
```

Expected: all tests in `regular_classical.rs` pass.

---

### Task 3: Algorithm Documentation

**Files:**
- Create: `qec-code/doc/regular_classical.md`
- Modify: `qec-code/README.md`

**Interfaces:**
- Consumes: the v1 implementation behavior from Task 2.
- Produces: source-controlled docs for future family generators.

- [ ] **Step 1: Create `qec-code/doc/regular_classical.md`**

Document:

```markdown
# Deterministic Regular Classical Matrices

`qec_code::regular_classical` is the repository-owned deterministic sampler for
regular binary parity-check matrices. Algorithm version 1 is immutable: any
behavioral change gets a new algorithm version instead of changing version 1.

## SplitMix64V1

The stream starts with `state = seed`. Every `next_u64()` call updates
`state = state + 0x9E3779B97F4A7C15 mod 2^64`, applies the two SplitMix64
multiply/xor finalizer rounds, and returns `z xor (z >> 31)`. For seed 7, the
first eight words are `0x63CBE1E459320DD7`, `0x044C3CD7F43C661C`,
`0xE6984080BAB12A02`, `0x953AEB70673E29CB`, `0x73D33B666A1E21DA`,
`0x3FDABE86CBBEAA11`, `0x77CBC4A133C2D0F6`, and `0x53FCD6513D02BEFE`.

## Bounded Index

For `upper_bound > 0`, compute `threshold = 2^64 mod upper_bound`, draw
`x = next_u64()` until `x >= threshold`, and return `x mod upper_bound`.
For `upper_bound == 0`, return `None` without consuming the stream.

## Matrix Generation

Validate version, nonzero dimensions, nonzero weights, nonzero retry limit,
checked stub products, and equal stub counts before constructing the stream.
Each attempt starts from row slots ordered by row index, visits columns in
ascending order, and selects non-duplicate rows for the current column through
the bounded helper. If the current column has no non-duplicate remaining row
slot, reject the attempt. Retries continue from the current stream state. A
successful matrix sorts supports within each row and then sorts rows
lexicographically; the seed-7 fixture is `[[0, 1, 2], [0, 3, 4], [1, 3, 5],
[2, 4, 5]]`.
```

- [ ] **Step 2: Link the docs from `qec-code/README.md`**

Add a short section after the dependency status:

```markdown
## Deterministic regular matrices

`qec_code::regular_classical` provides the versioned pure-Rust generator for
regular binary parity-check matrices used by random code families. The exact
version-1 stream, bounded-index rule, retry behavior, and seed-7 fixture are
documented in [`doc/regular_classical.md`](doc/regular_classical.md).
```

- [ ] **Step 3: Run doc-related checks**

Run:

```bash
cargo test -p qec-code --doc
cargo test -p qec-code --test regular_classical splitmix64_v1_seed7_matches_golden_words -- --exact
```

Expected: both pass.

---

### Task 4: Final Verification, Review, and Commit

**Files:**
- Modify only files from Tasks 1-3 unless a compile error reveals a required
  adjacent change.

**Interfaces:**
- Consumes: all implemented behavior and documentation.
- Produces: committed PR-ready branch.

- [ ] **Step 1: Run exact issue verification commands**

Run:

```bash
cargo test -p qec-code --test regular_classical deterministic_regular_matrix_matches_v1_fixture -- --exact
cargo test -p qec-code --test regular_classical deterministic_regular_matrix_rejects_invalid_degrees -- --exact
cargo test -p qec-code --test regular_classical splitmix64_v1_seed7_matches_golden_words -- --exact
cargo test -p qec-code --test regular_classical deterministic_regular_matrix_retry_limit_one_returns_exhausted -- --exact
```

Expected: all pass.

- [ ] **Step 2: Run crate and workspace verification**

Run:

```bash
cargo test -p qec-code
cargo test
```

Expected: both pass.

- [ ] **Step 3: Review changed files**

Run:

```bash
git status -sb
git diff --stat origin/master...HEAD
git diff --check
```

Expected: only #555 files changed and `git diff --check` reports no whitespace
errors.

- [ ] **Step 4: Commit implementation**

Stage only intended files:

```bash
git add qec-code/src/regular_classical.rs qec-code/src/lib.rs qec-code/src/error.rs qec-code/doc/regular_classical.md qec-code/README.md qec-code/tests/regular_classical.rs docs/superpowers/plans/2026-07-26-issue-555-deterministic-regular-classical-matrix.md
git commit -m "feat: add deterministic regular matrix generator"
```

- [ ] **Step 5: Use finishing workflow**

Use `superpowers:finishing-a-development-branch`. Choose option 2,
`Push and create a Pull Request`, because the Agent Desk prompt explicitly
requires a PR and the Standing Answer Policy says to choose the recommended
option when offered.
