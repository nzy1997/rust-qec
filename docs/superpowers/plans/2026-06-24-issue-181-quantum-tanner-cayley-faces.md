# Issue 181 Quantum Tanner Cayley Faces Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add deterministic Cayley-complex physical-face, oriented-face, and X/Z local-incidence enumeration for validated explicit quantum Tanner group data.

**Architecture:** Extend the existing `qec-code/src/codes/quantum_tanner.rs` module, because it already owns the parser, finite-group validator, and local-code helpers. The enumerator consumes `QuantumTannerConstructionMode` plus `ValidatedFiniteGroup`, validates generator-set semantics, canonicalizes physical face ids by sorted vertex keys, and returns plain Rust records for future CSS sparse-row generation.

**Tech Stack:** Rust 2024, existing `qec-code` crate, `QecError` typed errors, integration tests in `qec-code/tests/code.rs`.

## Global Constraints

- Supported v1 construction mode is exactly `lr_cayley_no_cover_v1`.
- Reserved cover modes must remain rejected with `UnsupportedQuantumTannerConstructionMode`.
- Do not compute local-code tensor products, generate CSS matrices, compute distance, add CLI support, or implement GAP/Oscar/SmallGroup generation.
- Face vertices are `g`, `a*g`, `g*b`, and `a*g*b` using the validated multiplication table.
- Physical-qubit ids are assigned by lexicographically sorting distinct canonical face keys.
- X incidence labels are `(a, b)` at source `g`.
- Z incidence labels are `(a^-1, b)` at source `a*g`.
- Preserve caller-provided `A` and `B` coordinate order for local coordinates.
- Validate generator arrays as nonempty, duplicate-free, and symmetric before enumerating faces.

---

## File Structure

- Modify `qec-code/src/error.rs`: add typed construction errors for invalid generator sets and degenerate faces.
- Modify `qec-code/src/codes/quantum_tanner.rs`: add output structs and `enumerate_quantum_tanner_cayley_faces`.
- Modify `qec-code/tests/code.rs`: import the enumerator and add the issue oracle plus negative controls.

### Task 1: Cayley Face And Incidence Enumerator

**Files:**
- Modify: `qec-code/src/error.rs`
- Modify: `qec-code/src/codes/quantum_tanner.rs`
- Modify: `qec-code/tests/code.rs`

**Interfaces:**
- Consumes: `QuantumTannerConstructionMode`, `ValidatedFiniteGroup`, existing `QecError`, `quantum_tanner_spec_from_json_str`, and `validate_quantum_tanner_group_table`.
- Produces:
  - `QuantumTannerCayleyComplex`
  - `QuantumTannerCayleyFace`
  - `QuantumTannerOrientedFace`
  - `QuantumTannerLocalIncidence`
  - `enumerate_quantum_tanner_cayley_faces(construction_mode: QuantumTannerConstructionMode, group: &ValidatedFiniteGroup) -> Result<QuantumTannerCayleyComplex>`

- [ ] **Step 1: Write the failing test**

In `qec-code/tests/code.rs`, extend the quantum Tanner import block to include
`enumerate_quantum_tanner_cayley_faces`, then add this test after the group
validator tests:

```rust
#[test]
fn quantum_tanner_cayley_faces_match_toric_d4_counts() {
    let spec =
        quantum_tanner_spec_from_json_str(include_str!("fixtures/quantum_tanner/toric_d4.json"))
            .unwrap();
    let group = validate_quantum_tanner_group_table(&spec).unwrap();

    let complex =
        enumerate_quantum_tanner_cayley_faces(spec.construction_mode, &group).unwrap();

    assert_eq!(complex.faces.len(), 16);
    assert_eq!(complex.oriented_faces.len(), 64);
    assert_eq!(complex.x_incidence.len(), 64);
    assert_eq!(complex.z_incidence.len(), 64);
    assert_eq!(
        complex
            .faces
            .iter()
            .map(|face| (face.id, face.vertices))
            .take(4)
            .collect::<Vec<_>>(),
        vec![
            (0, [0, 1, 4, 5]),
            (1, [0, 1, 12, 13]),
            (2, [0, 3, 4, 7]),
            (3, [0, 3, 12, 15]),
        ]
    );

    for source_vertex in 0..group.order() {
        let x_local = complex
            .x_incidence
            .iter()
            .filter(|record| record.source_vertex == source_vertex)
            .map(|record| (record.a_index, record.a_generator, record.b_index, record.b_generator))
            .collect::<Vec<_>>();
        let z_local = complex
            .z_incidence
            .iter()
            .filter(|record| record.source_vertex == source_vertex)
            .map(|record| (record.a_index, record.a_generator, record.b_index, record.b_generator))
            .collect::<Vec<_>>();
        assert_eq!(
            x_local,
            vec![(0, 4, 0, 1), (0, 4, 1, 3), (1, 12, 0, 1), (1, 12, 1, 3)]
        );
        assert_eq!(z_local, x_local);
    }

    let x_identity = complex
        .x_incidence
        .iter()
        .filter(|record| record.source_vertex == 0)
        .map(|record| (record.a_generator, record.b_generator, record.face_id))
        .collect::<Vec<_>>();
    assert_eq!(x_identity, vec![(4, 1, 0), (4, 3, 2), (12, 1, 1), (12, 3, 3)]);

    let z_source_four = complex
        .z_incidence
        .iter()
        .filter(|record| record.source_vertex == 4)
        .map(|record| (record.a_generator, record.b_generator, record.face_id))
        .collect::<Vec<_>>();
    assert_eq!(z_source_four, vec![(4, 1, 8), (4, 3, 9), (12, 1, 0), (12, 3, 2)]);

    let x_face = complex
        .x_incidence
        .iter()
        .find(|record| {
            record.source_vertex == 0 && record.a_generator == 4 && record.b_generator == 1
        })
        .unwrap()
        .face_id;
    let z_face = complex
        .z_incidence
        .iter()
        .find(|record| {
            record.source_vertex == 4 && record.a_generator == 12 && record.b_generator == 1
        })
        .unwrap()
        .face_id;
    assert_eq!(x_face, z_face);

    let non_symmetric_spec = quantum_tanner_spec_from_json_str(include_str!(
        "fixtures/quantum_tanner/invalid_non_symmetric_a.json"
    ))
    .unwrap();
    let non_symmetric_group = validate_quantum_tanner_group_table(&non_symmetric_spec).unwrap();
    assert!(matches!(
        enumerate_quantum_tanner_cayley_faces(
            non_symmetric_spec.construction_mode,
            &non_symmetric_group
        )
        .unwrap_err(),
        QecError::InvalidQuantumTannerGeneratorSet { set: "A", .. }
    ));

    let unsupported_mode = toric_d4_json_with(|fixture| {
        fixture["construction_mode"] = Value::String("lr_cayley_quadripartite_cover_v1".to_owned());
    });
    assert!(matches!(
        quantum_tanner_spec_from_json_str(&unsupported_mode).unwrap_err(),
        QecError::UnsupportedQuantumTannerConstructionMode { mode }
            if mode == "lr_cayley_quadripartite_cover_v1"
    ));
}
```

- [ ] **Step 2: Run the test to verify RED**

Run:

```bash
cargo test -p qec-code quantum_tanner_cayley_faces_match_toric_d4_counts -q
```

Expected: FAIL to compile because `enumerate_quantum_tanner_cayley_faces` is not
defined yet.

- [ ] **Step 3: Add typed errors**

In `qec-code/src/error.rs`, add these variants after
`InvalidQuantumTannerGeneratorIndex`:

```rust
#[error("invalid quantum Tanner generator set {set}: {reason}")]
InvalidQuantumTannerGeneratorSet {
    set: &'static str,
    reason: String,
},
#[error(
    "degenerate quantum Tanner face at root {root} with a={a}, b={b}: vertices {vertices:?}"
)]
DegenerateQuantumTannerFace {
    root: usize,
    a: usize,
    b: usize,
    vertices: Vec<usize>,
},
```

- [ ] **Step 4: Add output structs**

In `qec-code/src/codes/quantum_tanner.rs`, add these public structs after
`QuantumTannerLocalCodeTensorDual`:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuantumTannerCayleyComplex {
    pub faces: Vec<QuantumTannerCayleyFace>,
    pub oriented_faces: Vec<QuantumTannerOrientedFace>,
    pub x_incidence: Vec<QuantumTannerLocalIncidence>,
    pub z_incidence: Vec<QuantumTannerLocalIncidence>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuantumTannerCayleyFace {
    pub id: usize,
    pub vertices: [usize; 4],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuantumTannerOrientedFace {
    pub root_vertex: usize,
    pub a_index: usize,
    pub b_index: usize,
    pub a_generator: usize,
    pub b_generator: usize,
    pub vertices: [usize; 4],
    pub face_id: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuantumTannerLocalIncidence {
    pub source_vertex: usize,
    pub a_index: usize,
    pub b_index: usize,
    pub a_generator: usize,
    pub b_generator: usize,
    pub face_id: usize,
}
```

- [ ] **Step 5: Add the enumerator implementation**

In `qec-code/src/codes/quantum_tanner.rs`, import `BTreeMap` and `BTreeSet`:

```rust
use std::collections::{BTreeMap, BTreeSet};
```

Then add the enumerator and helpers after `quantum_tanner_local_code_tensor_dual`:

```rust
pub fn enumerate_quantum_tanner_cayley_faces(
    construction_mode: QuantumTannerConstructionMode,
    group: &ValidatedFiniteGroup,
) -> Result<QuantumTannerCayleyComplex> {
    match construction_mode {
        QuantumTannerConstructionMode::LeftRightCayleyNoCoverV1 => {}
    }

    validate_construction_generators("A", group.a_generators(), group)?;
    validate_construction_generators("B", group.b_generators(), group)?;

    let mut face_keys = BTreeSet::new();
    let mut pending_oriented = Vec::new();
    for root_vertex in 0..group.order() {
        for (a_index, &a_generator) in group.a_generators().iter().enumerate() {
            for (b_index, &b_generator) in group.b_generators().iter().enumerate() {
                let vertices =
                    oriented_face_vertices(group, root_vertex, a_generator, b_generator)?;
                face_keys.insert(vertices);
                pending_oriented.push((
                    root_vertex,
                    a_index,
                    b_index,
                    a_generator,
                    b_generator,
                    vertices,
                ));
            }
        }
    }

    let faces = face_keys
        .iter()
        .enumerate()
        .map(|(id, &vertices)| QuantumTannerCayleyFace { id, vertices })
        .collect::<Vec<_>>();
    let face_ids = faces
        .iter()
        .map(|face| (face.vertices, face.id))
        .collect::<BTreeMap<_, _>>();

    let inverse_a_indices = inverse_generator_indices(group.a_generators(), group)?;
    let mut oriented_faces = Vec::with_capacity(pending_oriented.len());
    let mut x_incidence = Vec::with_capacity(pending_oriented.len());
    let mut z_incidence = Vec::with_capacity(pending_oriented.len());

    for (root_vertex, a_index, b_index, a_generator, b_generator, vertices) in pending_oriented {
        let face_id = face_ids[&vertices];
        oriented_faces.push(QuantumTannerOrientedFace {
            root_vertex,
            a_index,
            b_index,
            a_generator,
            b_generator,
            vertices,
            face_id,
        });
        x_incidence.push(QuantumTannerLocalIncidence {
            source_vertex: root_vertex,
            a_index,
            b_index,
            a_generator,
            b_generator,
            face_id,
        });

        let z_source_vertex = group.multiply(a_generator, root_vertex)?;
        let z_a_generator = group.inv(a_generator)?;
        let z_a_index = inverse_a_indices[&a_generator];
        z_incidence.push(QuantumTannerLocalIncidence {
            source_vertex: z_source_vertex,
            a_index: z_a_index,
            b_index,
            a_generator: z_a_generator,
            b_generator,
            face_id,
        });
    }

    x_incidence.sort_by_key(local_incidence_sort_key);
    z_incidence.sort_by_key(local_incidence_sort_key);

    Ok(QuantumTannerCayleyComplex {
        faces,
        oriented_faces,
        x_incidence,
        z_incidence,
    })
}
```

Add these helpers nearby:

```rust
fn validate_construction_generators(
    set: &'static str,
    generators: &[usize],
    group: &ValidatedFiniteGroup,
) -> Result<()> {
    if generators.is_empty() {
        return Err(QecError::InvalidQuantumTannerGeneratorSet {
            set,
            reason: "generator set must be nonempty".to_owned(),
        });
    }

    let mut seen = BTreeSet::new();
    for (index, &generator) in generators.iter().enumerate() {
        if !seen.insert(generator) {
            return Err(QecError::InvalidQuantumTannerGeneratorSet {
                set,
                reason: format!("duplicate generator {generator} at coordinate {index}"),
            });
        }
    }

    for &generator in generators {
        let inverse = group.inv(generator)?;
        if !seen.contains(&inverse) {
            return Err(QecError::InvalidQuantumTannerGeneratorSet {
                set,
                reason: format!("generator {generator} is missing inverse {inverse}"),
            });
        }
    }

    Ok(())
}

fn oriented_face_vertices(
    group: &ValidatedFiniteGroup,
    root: usize,
    a: usize,
    b: usize,
) -> Result<[usize; 4]> {
    let ag = group.multiply(a, root)?;
    let gb = group.multiply(root, b)?;
    let agb = group.multiply(ag, b)?;
    let mut vertices = [root, ag, gb, agb];
    vertices.sort_unstable();
    if vertices.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(QecError::DegenerateQuantumTannerFace {
            root,
            a,
            b,
            vertices: vertices.to_vec(),
        });
    }
    Ok(vertices)
}

fn inverse_generator_indices(
    generators: &[usize],
    group: &ValidatedFiniteGroup,
) -> Result<BTreeMap<usize, usize>> {
    let mut inverse_indices = BTreeMap::new();
    for (index, &generator) in generators.iter().enumerate() {
        let inverse = group.inv(generator)?;
        let inverse_index = generators.iter().position(|&value| value == inverse).ok_or_else(|| {
            QecError::InvalidQuantumTannerGeneratorSet {
                set: "A",
                reason: format!("generator {generator} is missing inverse {inverse}"),
            }
        })?;
        inverse_indices.insert(generator, inverse_index);
    }
    Ok(inverse_indices)
}

fn local_incidence_sort_key(
    record: &QuantumTannerLocalIncidence,
) -> (usize, usize, usize, usize) {
    (
        record.source_vertex,
        record.a_index,
        record.b_index,
        record.face_id,
    )
}
```

- [ ] **Step 6: Run the focused test to verify GREEN**

Run:

```bash
cargo test -p qec-code quantum_tanner_cayley_faces_match_toric_d4_counts -q
```

Expected: PASS.

- [ ] **Step 7: Run related tests**

Run:

```bash
cargo test -p qec-code quantum_tanner -q
```

Expected: PASS.

- [ ] **Step 8: Format and diff-check touched files**

Run:

```bash
rustfmt qec-code/src/error.rs qec-code/src/codes/quantum_tanner.rs qec-code/tests/code.rs
git diff --check
```

Expected: rustfmt exits 0 and `git diff --check` reports no whitespace errors.

- [ ] **Step 9: Commit**

In a normal writable git checkout, run:

```bash
git add qec-code/src/error.rs qec-code/src/codes/quantum_tanner.rs qec-code/tests/code.rs docs/superpowers/specs/2026-06-24-issue-181-quantum-tanner-cayley-faces-design.md docs/superpowers/plans/2026-06-24-issue-181-quantum-tanner-cayley-faces.md
git commit -m "feat: enumerate quantum tanner cayley faces"
```

In the Agent Desk sandbox for this run, local git index writes are blocked
because the real git directory lives outside the writable root. Use the GitHub
connector at finish time to create the branch commit with the same file set.

## Plan Self-Review

- Spec coverage: the plan covers no-cover face enumeration, generator-set
  validation, deterministic physical ids, X/Z incidence labels, toric count and
  order oracle, non-symmetric generator rejection, and unsupported mode
  rejection.
- Placeholder scan: no placeholders or deferred implementation steps remain.
- Type consistency: struct and function names match across tests,
  implementation snippets, and produced interfaces.
