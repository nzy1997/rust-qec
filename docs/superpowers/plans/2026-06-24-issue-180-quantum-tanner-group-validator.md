# Issue 180 Quantum Tanner Group Validator Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add semantic finite-group validation for parsed `QuantumTannerSpec` group tables and generator element indices.

**Architecture:** Extend `qec-code/src/codes/quantum_tanner.rs` with a small `ValidatedFiniteGroup` value and validator function that sits after JSON parsing and before any future Cayley-complex or CSS construction. Keep parser shape checks intact, reuse the existing `InvalidQuantumTannerGroupTable` error for table axiom failures, and add typed errors for invalid generator indices and invalid safe-accessor element arguments.

**Tech Stack:** Rust 2024, existing `qec-code::QecError`, existing `serde_json` parser tests, quantum Tanner fixtures under `qec-code/tests/fixtures/quantum_tanner/`.

## Global Constraints

- Implement the validator in `qec-code/src/codes/quantum_tanner.rs`.
- Consume parsed `QuantumTannerSpec` values; do not add a second JSON parser.
- Validate that the multiplication table is square and all entries are in range.
- Treat in-range multiplication entries as closure.
- Validate that exactly one two-sided identity exists and that it matches `spec.base_group.identity`.
- Validate that every element has exactly one two-sided inverse under the identity.
- Validate associativity over all triples.
- Validate that `a_generator_indices` and `b_generator_indices` are in range.
- Return a validated finite-group value with identity, inverse lookup, multiplication lookup, and safe generator access.
- Include comments or doc links to `drafts/qLDPC/src/qldpc/objects.py` and `drafts/qLDPC/src/qldpc/codes/quantum.py`.
- Do not add group generation, subgroup search, conjugacy-class enumeration, SmallGroup support, Cayley-complex face enumeration, generator symmetry checks, CSS `Hx`/`Hz` generation, external group database parsing, GAP/Oscar calls, or CLI support.
- Requested focused verification is `cargo test -p qec-code quantum_tanner_group_table_validator -q`.
- Because this Agent Desk sandbox blocks crates.io index access, use `--offline` for local red/green loops when needed, and still run the exact requested command for final reporting.

---

## File Structure

- Modify `qec-code/src/error.rs`: add typed errors for generator index validation and safe accessor element bounds.
- Modify `qec-code/src/codes/quantum_tanner.rs`: add `ValidatedFiniteGroup`, public validator API, helper methods, and semantic validation helpers.
- Modify `qec-code/tests/code.rs`: add focused validator tests for `Z2 x Z2`, catalog `toric_d4`, associativity failure, generator bounds, and safe accessor bounds.
- Keep this plan in `docs/superpowers/plans/2026-06-24-issue-180-quantum-tanner-group-validator.md`.

### Task 1: Finite Group Validator API And Tests

**Files:**
- Modify: `qec-code/src/error.rs`
- Modify: `qec-code/src/codes/quantum_tanner.rs`
- Modify: `qec-code/tests/code.rs`
- Modify: `docs/superpowers/specs/2026-06-24-issue-180-quantum-tanner-group-validator-design.md`
- Modify: `docs/superpowers/plans/2026-06-24-issue-180-quantum-tanner-group-validator.md`

**Interfaces:**
- Consumes: `QuantumTannerSpec`, `ExplicitFiniteGroup`, and fixture `qec-code/tests/fixtures/quantum_tanner/toric_d4.json`.
- Produces: `pub fn validate_quantum_tanner_group_table(spec: &QuantumTannerSpec) -> Result<ValidatedFiniteGroup>`.
- Produces: `ValidatedFiniteGroup` methods `order`, `identity`, `multiply`, `inv`, `a_generators`, `b_generators`, `a_generator`, and `b_generator`.
- Produces: `QecError::InvalidQuantumTannerGeneratorIndex { set, index, element, order }`.
- Produces: `QecError::InvalidQuantumTannerGroupElement { element, order }`.

- [ ] **Step 1: Write the failing validator tests**

Modify the `qec-code/tests/code.rs` quantum Tanner import to include the validator API and typed structs:

```rust
use qec_code::codes::quantum_tanner::{
    ExplicitFiniteGroup, QuantumTannerConstructionMode, QuantumTannerLocalCodes,
    QuantumTannerSpec, quantum_tanner_spec_from_json_str, validate_quantum_tanner_group_table,
};
```

Add these tests and helpers near the existing quantum Tanner parser tests:

```rust
fn quantum_tanner_group_table_validator_spec(
    order: usize,
    identity: usize,
    multiplication_table: Vec<Vec<usize>>,
    a_generator_indices: Vec<usize>,
    b_generator_indices: Vec<usize>,
) -> QuantumTannerSpec {
    let a_width = a_generator_indices.len();
    let b_width = b_generator_indices.len();
    QuantumTannerSpec {
        construction_mode: QuantumTannerConstructionMode::LeftRightCayleyNoCoverV1,
        base_group: ExplicitFiniteGroup {
            name: None,
            element_order: None,
            order,
            identity,
            multiplication_table,
        },
        a_generator_indices,
        b_generator_indices,
        local_codes: QuantumTannerLocalCodes {
            matrix_role: "parity_check".to_owned(),
            field: "GF(2)".to_owned(),
            h_a: vec![vec![1; a_width]],
            h_b: vec![vec![1; b_width]],
        },
    }
}

fn z2xz2_group_table() -> Vec<Vec<usize>> {
    vec![
        vec![0, 1, 2, 3],
        vec![1, 0, 3, 2],
        vec![2, 3, 0, 1],
        vec![3, 2, 1, 0],
    ]
}

#[test]
fn quantum_tanner_group_table_validator_accepts_z2xz2_and_safe_accessors() {
    let spec = quantum_tanner_group_table_validator_spec(
        4,
        0,
        z2xz2_group_table(),
        vec![1, 2],
        vec![3],
    );

    let group = validate_quantum_tanner_group_table(&spec).unwrap();

    assert_eq!(group.order(), 4);
    assert_eq!(group.identity(), 0);
    assert_eq!(group.multiply(1, 2).unwrap(), 3);
    assert_eq!(group.multiply(2, 1).unwrap(), 3);
    assert_eq!(group.multiply(3, 3).unwrap(), 0);
    assert_eq!(group.inv(0).unwrap(), 0);
    assert_eq!(group.inv(1).unwrap(), 1);
    assert_eq!(group.inv(2).unwrap(), 2);
    assert_eq!(group.inv(3).unwrap(), 3);
    assert_eq!(group.a_generators(), &[1, 2]);
    assert_eq!(group.b_generators(), &[3]);
    assert_eq!(group.a_generator(0), Some(1));
    assert_eq!(group.a_generator(1), Some(2));
    assert_eq!(group.a_generator(2), None);
    assert_eq!(group.b_generator(0), Some(3));
    assert_eq!(group.b_generator(1), None);
}

#[test]
fn quantum_tanner_group_table_validator_accepts_toric_d4_catalog_fixture() {
    let spec =
        quantum_tanner_spec_from_json_str(include_str!("fixtures/quantum_tanner/toric_d4.json"))
            .unwrap();

    let group = validate_quantum_tanner_group_table(&spec).unwrap();

    assert_eq!(group.order(), 16);
    assert_eq!(group.identity(), 0);
    assert_eq!(group.multiply(4, 12).unwrap(), 0);
    assert_eq!(group.multiply(12, 4).unwrap(), 0);
    assert_eq!(group.inv(4).unwrap(), 12);
    assert_eq!(group.inv(12).unwrap(), 4);
    assert_eq!(group.multiply(1, 3).unwrap(), 0);
    assert_eq!(group.inv(1).unwrap(), 3);
    assert_eq!(group.inv(3).unwrap(), 1);
    assert_eq!(group.a_generator(0), Some(4));
    assert_eq!(group.a_generator(1), Some(12));
    assert_eq!(group.b_generator(0), Some(1));
    assert_eq!(group.b_generator(1), Some(3));
}

#[test]
fn quantum_tanner_group_table_validator_rejects_square_in_range_non_associative_table() {
    let non_associative_table = vec![
        vec![0, 1, 2, 3],
        vec![1, 0, 2, 3],
        vec![2, 3, 0, 1],
        vec![3, 2, 1, 0],
    ];
    let spec = quantum_tanner_group_table_validator_spec(
        4,
        0,
        non_associative_table,
        vec![1],
        vec![2],
    );

    let error = validate_quantum_tanner_group_table(&spec).unwrap_err();
    let QecError::InvalidQuantumTannerGroupTable { reason } = error else {
        panic!("expected group-table validation error, got {error:?}");
    };
    assert!(
        reason.contains("associativity failed for (1, 2, 2)"),
        "expected the square in-range negative control to fail associativity, got {reason:?}"
    );
}

#[test]
fn quantum_tanner_group_table_validator_rejects_out_of_range_generators_and_elements() {
    let bad_generator_spec = quantum_tanner_group_table_validator_spec(
        4,
        0,
        z2xz2_group_table(),
        vec![4],
        vec![1],
    );

    let error = validate_quantum_tanner_group_table(&bad_generator_spec).unwrap_err();
    assert!(matches!(
        error,
        QecError::InvalidQuantumTannerGeneratorIndex {
            set: "A",
            index: 0,
            element: 4,
            order: 4
        }
    ));

    let valid_spec = quantum_tanner_group_table_validator_spec(
        4,
        0,
        z2xz2_group_table(),
        vec![1],
        vec![2],
    );
    let group = validate_quantum_tanner_group_table(&valid_spec).unwrap();

    assert!(matches!(
        group.multiply(4, 0).unwrap_err(),
        QecError::InvalidQuantumTannerGroupElement {
            element: 4,
            order: 4
        }
    ));
    assert!(matches!(
        group.multiply(0, 4).unwrap_err(),
        QecError::InvalidQuantumTannerGroupElement {
            element: 4,
            order: 4
        }
    ));
    assert!(matches!(
        group.inv(4).unwrap_err(),
        QecError::InvalidQuantumTannerGroupElement {
            element: 4,
            order: 4
        }
    ));
}
```

- [ ] **Step 2: Run the focused test to verify RED**

Run:

```bash
cargo test -p qec-code quantum_tanner_group_table_validator -q --offline
```

Expected: FAIL to compile because `validate_quantum_tanner_group_table`, `InvalidQuantumTannerGeneratorIndex`, and `InvalidQuantumTannerGroupElement` do not exist yet. That proves the tests are wired to the missing validator API.

- [ ] **Step 3: Add typed error variants**

Add these variants to `QecError` in `qec-code/src/error.rs` after `InvalidQuantumTannerLocalCodeMatrix`:

```rust
#[error(
    "invalid quantum Tanner generator {set}[{index}]: element {element} is out of range for group order {order}"
)]
InvalidQuantumTannerGeneratorIndex {
    set: &'static str,
    index: usize,
    element: usize,
    order: usize,
},
#[error("invalid quantum Tanner group element {element}: expected < {order}")]
InvalidQuantumTannerGroupElement { element: usize, order: usize },
```

- [ ] **Step 4: Implement the validator API**

Modify `qec-code/src/codes/quantum_tanner.rs` by keeping the existing parser API and adding the validator value, public methods, and helper functions:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedFiniteGroup {
    identity: usize,
    multiplication_table: Vec<Vec<usize>>,
    inverse_table: Vec<usize>,
    a_generators: Vec<usize>,
    b_generators: Vec<usize>,
}

impl ValidatedFiniteGroup {
    pub fn order(&self) -> usize {
        self.multiplication_table.len()
    }

    pub fn identity(&self) -> usize {
        self.identity
    }

    pub fn multiply(&self, left: usize, right: usize) -> Result<usize> {
        self.validate_element(left)?;
        self.validate_element(right)?;
        Ok(self.multiplication_table[left][right])
    }

    pub fn inv(&self, element: usize) -> Result<usize> {
        self.validate_element(element)?;
        Ok(self.inverse_table[element])
    }

    pub fn a_generators(&self) -> &[usize] {
        &self.a_generators
    }

    pub fn b_generators(&self) -> &[usize] {
        &self.b_generators
    }

    pub fn a_generator(&self, index: usize) -> Option<usize> {
        self.a_generators.get(index).copied()
    }

    pub fn b_generator(&self, index: usize) -> Option<usize> {
        self.b_generators.get(index).copied()
    }

    fn validate_element(&self, element: usize) -> Result<()> {
        let order = self.order();
        if element < order {
            Ok(())
        } else {
            Err(QecError::InvalidQuantumTannerGroupElement { element, order })
        }
    }
}

/// Validate the explicit finite group data used by quantum Tanner construction.
///
/// The group-side expectations mirror qLDPC's `CayleyComplex` vocabulary in
/// `drafts/qLDPC/src/qldpc/objects.py`; later `QTCode` consumption follows
/// `drafts/qLDPC/src/qldpc/codes/quantum.py`.
pub fn validate_quantum_tanner_group_table(
    spec: &QuantumTannerSpec,
) -> Result<ValidatedFiniteGroup> {
    let group = &spec.base_group;
    validate_group_table_shape(
        group.order,
        group.identity,
        &group.multiplication_table,
    )?;
    let identity = find_unique_table_identity(group.order, &group.multiplication_table)?;
    if identity != group.identity {
        return Err(QecError::InvalidQuantumTannerGroupTable {
            reason: format!(
                "declared identity {} does not match table identity {identity}",
                group.identity
            ),
        });
    }
    let inverse_table = build_inverse_table(&group.multiplication_table, identity)?;
    validate_associativity(&group.multiplication_table)?;
    validate_generator_indices("A", &spec.a_generator_indices, group.order)?;
    validate_generator_indices("B", &spec.b_generator_indices, group.order)?;

    Ok(ValidatedFiniteGroup {
        identity,
        multiplication_table: group.multiplication_table.clone(),
        inverse_table,
        a_generators: spec.a_generator_indices.clone(),
        b_generators: spec.b_generator_indices.clone(),
    })
}

fn validate_group_table_shape(order: usize, identity: usize, table: &[Vec<usize>]) -> Result<()> {
    if order == 0 {
        return Err(QecError::InvalidQuantumTannerGroupTable {
            reason: "order must be positive".to_owned(),
        });
    }
    if identity >= order {
        return Err(QecError::InvalidQuantumTannerGroupTable {
            reason: format!("identity {identity} is out of range for order {order}"),
        });
    }
    if table.len() != order {
        return Err(QecError::InvalidQuantumTannerGroupTable {
            reason: format!("expected {order} rows, got {}", table.len()),
        });
    }

    for (row_index, row) in table.iter().enumerate() {
        if row.len() != order {
            return Err(QecError::InvalidQuantumTannerGroupTable {
                reason: format!("row {row_index} has width {}, expected {order}", row.len()),
            });
        }
        for (col_index, &entry) in row.iter().enumerate() {
            if entry >= order {
                return Err(QecError::InvalidQuantumTannerGroupTable {
                    reason: format!(
                        "entry at row {row_index}, column {col_index} is {entry}, expected < {order}"
                    ),
                });
            }
        }
    }

    Ok(())
}

fn find_unique_table_identity(order: usize, table: &[Vec<usize>]) -> Result<usize> {
    let candidates = (0..order)
        .filter(|&candidate| {
            (0..order).all(|element| {
                table[candidate][element] == element && table[element][candidate] == element
            })
        })
        .collect::<Vec<_>>();

    match candidates.as_slice() {
        [identity] => Ok(*identity),
        [] => Err(QecError::InvalidQuantumTannerGroupTable {
            reason: "expected exactly one two-sided identity, found none".to_owned(),
        }),
        many => Err(QecError::InvalidQuantumTannerGroupTable {
            reason: format!("expected exactly one two-sided identity, found {many:?}"),
        }),
    }
}

fn build_inverse_table(table: &[Vec<usize>], identity: usize) -> Result<Vec<usize>> {
    let order = table.len();
    let mut inverse_table = Vec::with_capacity(order);
    for element in 0..order {
        let candidates = (0..order)
            .filter(|&candidate| {
                table[element][candidate] == identity && table[candidate][element] == identity
            })
            .collect::<Vec<_>>();
        match candidates.as_slice() {
            [inverse] => inverse_table.push(*inverse),
            [] => {
                return Err(QecError::InvalidQuantumTannerGroupTable {
                    reason: format!(
                        "element {element} has no two-sided inverse under identity {identity}"
                    ),
                });
            }
            many => {
                return Err(QecError::InvalidQuantumTannerGroupTable {
                    reason: format!(
                        "element {element} has multiple two-sided inverses under identity {identity}: {many:?}"
                    ),
                });
            }
        }
    }
    Ok(inverse_table)
}

fn validate_associativity(table: &[Vec<usize>]) -> Result<()> {
    let order = table.len();
    for a in 0..order {
        for b in 0..order {
            for c in 0..order {
                let left = table[table[a][b]][c];
                let right = table[a][table[b][c]];
                if left != right {
                    return Err(QecError::InvalidQuantumTannerGroupTable {
                        reason: format!(
                            "associativity failed for ({a}, {b}, {c}): ({a} * {b}) * {c} = {left}, but {a} * ({b} * {c}) = {right}"
                        ),
                    });
                }
            }
        }
    }
    Ok(())
}

fn validate_generator_indices(set: &'static str, generators: &[usize], order: usize) -> Result<()> {
    for (index, &element) in generators.iter().enumerate() {
        if element >= order {
            return Err(QecError::InvalidQuantumTannerGeneratorIndex {
                set,
                index,
                element,
                order,
            });
        }
    }
    Ok(())
}
```

Then update the existing parser helper `validate_group_table` so it calls the new shared shape helper before preserving the v1 parser's `identity == 0` contract:

```rust
fn validate_group_table(order: usize, identity: usize, table: &[Vec<usize>]) -> Result<()> {
    validate_group_table_shape(order, identity, table)?;
    if identity != 0 {
        return Err(QecError::InvalidQuantumTannerGroupTable {
            reason: format!("identity must be 0 in v1, got {identity}"),
        });
    }
    Ok(())
}
```

- [ ] **Step 5: Run the focused test to verify GREEN**

Run:

```bash
cargo test -p qec-code quantum_tanner_group_table_validator -q --offline
```

Expected: PASS with the validator tests executed.

- [ ] **Step 6: Run the existing parser test**

Run:

```bash
cargo test -p qec-code quantum_tanner_spec_json_accepts_toric_d4_and_rejects_bad_table -q --offline
```

Expected: PASS, preserving #178 behavior.

- [ ] **Step 7: Format touched Rust files**

Run:

```bash
rustfmt qec-code/src/error.rs qec-code/src/codes/quantum_tanner.rs qec-code/tests/code.rs
```

Expected: command exits 0 and only formats touched Rust files.

- [ ] **Step 8: Run final focused verification**

Run:

```bash
cargo test -p qec-code quantum_tanner_group_table_validator -q --offline
```

Expected: PASS.

Run the exact requested command:

```bash
cargo test -p qec-code quantum_tanner_group_table_validator -q
```

Expected in a network-enabled environment: PASS. In this sandbox, if Cargo tries to update the crates.io index and fails, record the network failure and rely on the offline pass as the code verification evidence.

- [ ] **Step 9: Run broader verification**

Run:

```bash
cargo test --offline
```

Expected: PASS for the workspace suite using cached dependencies.

Run:

```bash
cargo test
```

Expected in a network-enabled environment: PASS. In this sandbox, if Cargo tries to update the crates.io index and fails, record the network failure.

- [ ] **Step 10: Check whitespace and scope**

Run:

```bash
git diff --check
```

Expected: no whitespace errors.

Review the diff and confirm it only includes the validator, validator tests, typed errors, and required Superpowers spec/plan files. There must be no constructor, CLI, face enumeration, CSS generation, group search, or unrelated refactor.

- [ ] **Step 11: Commit implementation**

Run:

```bash
git add qec-code/src/error.rs qec-code/src/codes/quantum_tanner.rs qec-code/tests/code.rs docs/superpowers/specs/2026-06-24-issue-180-quantum-tanner-group-validator-design.md docs/superpowers/plans/2026-06-24-issue-180-quantum-tanner-group-validator.md
git commit -m "feat: validate quantum tanner group tables"
```

Expected: one implementation commit containing the validator and its tests.
