# Issue 177 Quantum Tanner Contract Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a developer-facing quantum Tanner construction contract and a doc-backed test that verifies the documented examples and negative controls.

**Architecture:** Keep the durable contract in `qec-code/doc/quantum_tanner.md`, matching the existing `qec-code/doc/apm_css.md` pattern. Keep executable checks in `qec-code/tests/code.rs` as test-local helpers that include the Markdown, extract marked JSON examples, and verify the documented `Z4 x Z4` counting convention without adding parser or generator APIs.

**Tech Stack:** Rust 2024 integration tests, `serde_json::Value`, Markdown documentation, existing `qec-code` `sparse_rows` vocabulary.

## Global Constraints

- Actual contract note path is exactly `qec-code/doc/quantum_tanner.md`.
- Add focused test `quantum_tanner_contract_examples_compile` in `qec-code/tests/code.rs`.
- The contract must define explicit finite-group multiplication table input, `A`/`B` generator indices, local GF(2) matrices, construction mode vocabulary, expected sparse CSS output, and the boundary that external tools generate data while `qec-code` consumes validated explicit data only.
- The contract must distinguish the base group from construction-mode covers and state that v1 `A`/`B` indices refer to base-group elements.
- The v1 supported mode is `lr_cayley_no_cover_v1`; unsupported cover modes must be rejected with a typed error by future implementation.
- The contract must define canonical face records to physical-qubit ids.
- The documented `Z4 x Z4` toric Tanner example must have `n = 16`, `k = 2`, and expected distance `4`, with a construction-mode/count explanation.
- The document must include a bad non-symmetric generator example.
- Do not implement a parser, group validator, Cayley-complex enumeration, CLI, GAP/Oscar integration, SmallGroup support, or code search.
- Use qLDPC and QuantumExpanders.jl as algorithm/vocabulary references only; do not copy implementation code mechanically.

---

## File Structure

- Create `qec-code/doc/quantum_tanner.md`: the construction contract, reference notes, input schema, validation boundary, output contract, mode vocabulary, face canonicalization, and examples.
- Modify `qec-code/tests/code.rs`: add a doc-backed integration test and private helpers near the existing `apm_contract_doc_examples_compile` test.
- Keep this plan in `docs/superpowers/plans/2026-06-24-issue-177-quantum-tanner-contract.md` for traceability.

### Task 1: Quantum Tanner Contract And Doc-Backed Test

**Files:**
- Create: `qec-code/doc/quantum_tanner.md`
- Modify: `qec-code/tests/code.rs`
- Modify: `docs/superpowers/plans/2026-06-24-issue-177-quantum-tanner-contract.md`

**Interfaces:**
- Consumes: existing `serde_json::Value` import in `qec-code/tests/code.rs`.
- Produces: integration test `quantum_tanner_contract_examples_compile`.

- [ ] **Step 1: Write the failing doc-backed test**

Add these helpers and the test near `apm_contract_doc_examples_compile` in `qec-code/tests/code.rs`:

```rust
fn extract_marked_json(doc: &str, marker: &str) -> Result<Value, String> {
    let marker_text = format!("<!-- {marker} -->");
    let after_marker = doc
        .split_once(&marker_text)
        .map(|(_, after)| after)
        .ok_or_else(|| format!("missing marker {marker_text}"))?;
    let fence_start = after_marker
        .find("```json")
        .ok_or_else(|| format!("missing json fence after {marker_text}"))?;
    let json_start = fence_start + "```json".len();
    let json_tail = &after_marker[json_start..];
    let json_end = json_tail
        .find("```")
        .ok_or_else(|| format!("missing closing json fence after {marker_text}"))?;
    serde_json::from_str(json_tail[..json_end].trim())
        .map_err(|error| format!("invalid json after {marker_text}: {error}"))
}

fn usize_array(value: &Value, path: &str) -> Vec<usize> {
    value
        .as_array()
        .unwrap_or_else(|| panic!("{path}: expected array"))
        .iter()
        .map(|entry| {
            entry
                .as_u64()
                .unwrap_or_else(|| panic!("{path}: expected unsigned integer")) as usize
        })
        .collect()
}

fn usize_matrix(value: &Value, path: &str) -> Vec<Vec<usize>> {
    value
        .as_array()
        .unwrap_or_else(|| panic!("{path}: expected matrix"))
        .iter()
        .enumerate()
        .map(|(row_index, row)| usize_array(row, &format!("{path}[{row_index}]")))
        .collect()
}

fn assert_group_table_shape(table: &[Vec<usize>], order: usize) {
    assert_eq!(table.len(), order, "multiplication table row count");
    for row in table {
        assert_eq!(row.len(), order, "multiplication table column count");
        for &entry in row {
            assert!(entry < order, "table entry {entry} out of range for order {order}");
        }
    }
}

fn inverse_index(table: &[Vec<usize>], identity: usize, element: usize) -> Option<usize> {
    (0..table.len()).find(|&candidate| {
        table[element][candidate] == identity && table[candidate][element] == identity
    })
}

fn generators_are_symmetric(table: &[Vec<usize>], identity: usize, generators: &[usize]) -> bool {
    generators.iter().all(|&generator| {
        inverse_index(table, identity, generator)
            .map(|inverse| generators.contains(&inverse))
            .unwrap_or(false)
    })
}

fn documented_face_count(
    table: &[Vec<usize>],
    a_generators: &[usize],
    b_generators: &[usize],
) -> usize {
    let mut faces = std::collections::BTreeSet::new();
    for g in 0..table.len() {
        for &a in a_generators {
            for &b in b_generators {
                let ag = table[a][g];
                let gb = table[g][b];
                let agb = table[ag][b];
                let mut face = vec![g, ag, gb, agb];
                face.sort_unstable();
                face.dedup();
                assert_eq!(face.len(), 4, "face must be nondegenerate");
                faces.insert(face);
            }
        }
    }
    faces.len()
}

#[test]
fn quantum_tanner_contract_examples_compile() {
    let doc = include_str!("../doc/quantum_tanner.md");
    assert!(doc.contains("drafts/qLDPC/src/qldpc/codes/quantum.py"));
    assert!(doc.contains("drafts/qLDPC/src/qldpc/objects.py"));
    assert!(doc.contains("drafts/qLDPC/src/qldpc/codes/quantum_test.py"));
    assert!(doc.contains("https://github.com/qLDPCOrg/qLDPC"));
    assert!(doc.contains("https://github.com/QuantumSavory/QuantumExpanders.jl"));
    assert!(doc.contains("lr_cayley_no_cover_v1"));
    assert!(doc.contains("lr_cayley_bipartite_double_cover_v1"));
    assert!(doc.contains("lr_cayley_quadripartite_cover_v1"));
    assert!(doc.contains("UnsupportedConstructionMode"));
    assert!(doc.contains("<!-- quantum_tanner_contract:toric_d4_counting_convention -->"));
    assert!(doc.contains("n = |G| * |A| * |B| / 4 = 16 * 2 * 2 / 4 = 16"));
    assert!(doc.contains("<!-- quantum_tanner_contract:bad_non_symmetric_generator -->"));

    let toric = extract_marked_json(doc, "quantum_tanner_contract:toric_d4").unwrap();
    assert_eq!(toric["example_id"].as_str(), Some("toric_d4"));
    assert_eq!(toric["construction_mode"].as_str(), Some("lr_cayley_no_cover_v1"));

    let group = &toric["base_group"];
    assert_eq!(group["name"].as_str(), Some("Z4xZ4"));
    assert_eq!(group["identity"].as_u64(), Some(0));
    let table = usize_matrix(&group["multiplication_table"], "base_group.multiplication_table");
    assert_group_table_shape(&table, 16);

    let a_generators = usize_array(&toric["a_generator_indices"], "a_generator_indices");
    let b_generators = usize_array(&toric["b_generator_indices"], "b_generator_indices");
    assert!(generators_are_symmetric(&table, 0, &a_generators));
    assert!(generators_are_symmetric(&table, 0, &b_generators));

    let expected = &toric["expected_css"];
    assert_eq!(expected["n"].as_u64(), Some(16));
    assert_eq!(expected["k"].as_u64(), Some(2));
    assert_eq!(expected["expected_distance"].as_u64(), Some(4));
    assert_eq!(
        documented_face_count(&table, &a_generators, &b_generators),
        expected["n"].as_u64().unwrap() as usize
    );

    let local_a = usize_matrix(&toric["local_codes"]["h_a"], "local_codes.h_a");
    let local_b = usize_matrix(&toric["local_codes"]["h_b"], "local_codes.h_b");
    assert!(local_a.iter().all(|row| row.len() == a_generators.len()));
    assert!(local_b.iter().all(|row| row.len() == b_generators.len()));
    assert!(local_a.iter().flatten().all(|&bit| bit <= 1));
    assert!(local_b.iter().flatten().all(|&bit| bit <= 1));

    let bad = extract_marked_json(
        doc,
        "quantum_tanner_contract:bad_non_symmetric_generator",
    )
    .unwrap();
    let bad_a = usize_array(&bad["a_generator_indices"], "bad.a_generator_indices");
    assert!(!generators_are_symmetric(&table, 0, &bad_a));
    assert_eq!(
        bad["expected_error"].as_str(),
        Some("NonSymmetricGeneratorSet")
    );
}
```

- [ ] **Step 2: Run the focused test to verify RED**

Run: `cargo test -p qec-code quantum_tanner_contract_examples_compile -q`

Expected: FAIL at compile time because `qec-code/doc/quantum_tanner.md` does not exist yet. That proves the verifier is wired to the missing note.

- [ ] **Step 3: Add `qec-code/doc/quantum_tanner.md`**

Create the document with these required sections and values:

```markdown
# Quantum Tanner Construction Contract

This note is the v1 implementation contract for future quantum Tanner support in
`qec-code`. It is a consumer contract: external tools may generate finite-group
data, generator sets, or local-code choices, but `qec-code` only consumes
validated explicit data and emits deterministic CSS sparse-row matrices.

## References

- Local qLDPC reference: `drafts/qLDPC/src/qldpc/codes/quantum.py`, especially
  `QTCode`, `QTCode.get_subgraphs`, and `QTCode.get_subcodes`.
- Local qLDPC Cayley-complex reference: `drafts/qLDPC/src/qldpc/objects.py`,
  especially `CayleyComplex`, cover handling, total no-conjugacy, and face
  semantics.
- Local qLDPC test reference: `drafts/qLDPC/src/qldpc/codes/quantum_test.py`,
  especially `test_toric_tanner_code`.
- Upstream qLDPC: <https://github.com/qLDPCOrg/qLDPC>.
- QuantumExpanders.jl vocabulary: <https://github.com/QuantumSavory/QuantumExpanders.jl>.

## Scope Boundary

`qec-code` v1 accepts explicit finite data. It must not call GAP, Oscar,
SmallGroup, Morgenstern constructors, Ramanujan graph search, random-code
search, or external qLDPC/Julia code at runtime.

## Input Contract

Define `group_order`, `identity`, and `multiplication_table` as a rectangular
`group_order x group_order` table of zero-based element indices. Entry
`multiplication_table[left][right]` is the product `left * right`. The identity
index is `0` in v1.

`a_generator_indices` and `b_generator_indices` are arrays of base-group element
indices. They are not covered-element ids in v1. Each set must be symmetric:
for every listed element, its inverse under the multiplication table must also
be listed.

Local codes are binary GF(2) parity-check matrices `h_a` and `h_b`. `h_a` row
width must equal `|A|`; `h_b` row width must equal `|B|`. Future implementation
may derive the tensor-dual local sectors used by the quantum Tanner CSS
construction, but this input contract does not accept nonbinary matrices.

## Construction Modes

Supported in v1:

- `lr_cayley_no_cover_v1`: use the validated base group directly.

Reserved but unsupported in v1:

- `lr_cayley_bipartite_double_cover_v1`
- `lr_cayley_quadripartite_cover_v1`

Future code must reject unsupported mode strings with `UnsupportedConstructionMode`.

## Face Canonicalization

For `lr_cayley_no_cover_v1`, an oriented face record is `(g, a, b)` with
`g` in the base group, `a` in `A`, and `b` in `B`. Its vertices are
`{g, a*g, g*b, a*g*b}` using the multiplication table. The canonical face key is
the sorted, duplicate-free list of those four vertex ids. Distinct canonical
face keys are assigned physical-qubit ids by lexicographic order.

<!-- quantum_tanner_contract:toric_d4_counting_convention -->
For the `Z4 x Z4` toric Tanner example below, the mode is
`lr_cayley_no_cover_v1`, so no construction-mode cover changes the vertex set.
There are `|G| * |A| * |B| = 16 * 2 * 2 = 64` oriented face records. Each square
is reached from four orientations, so the physical-qubit count is
`n = |G| * |A| * |B| / 4 = 16 * 2 * 2 / 4 = 16`.

## Output Contract

The generator should return two `sparse_rows` matrices compatible with
`qec-code/src/css.rs`:

- `Hx.format == "sparse_rows"` and `Hz.format == "sparse_rows"`
- `Hx.num_cols == Hz.num_cols == n`
- row supports sorted, unique, and in range
- deterministic row and column order from the canonical face ids and source
  vertex order
- `Hx * Hz^T == 0 mod 2`

## Error Vocabulary

Typed future errors should include:

- `InvalidGroupTable`
- `InvalidGeneratorIndex`
- `NonSymmetricGeneratorSet`
- `InvalidLocalCodeMatrix`
- `UnsupportedConstructionMode`
- `DegenerateFace`
- `NonOrthogonalCssOutput`

## Example: `toric_d4`

<!-- quantum_tanner_contract:toric_d4 -->
```json
{
  "example_id": "toric_d4",
  "construction_mode": "lr_cayley_no_cover_v1",
  "base_group": {
    "name": "Z4xZ4",
    "element_order": "id = 4*x + y for (x,y) in Z4 x Z4",
    "order": 16,
    "identity": 0,
    "multiplication_table": [
      [
        0,
        1,
        2,
        3,
        4,
        5,
        6,
        7,
        8,
        9,
        10,
        11,
        12,
        13,
        14,
        15
      ],
      [
        1,
        2,
        3,
        0,
        5,
        6,
        7,
        4,
        9,
        10,
        11,
        8,
        13,
        14,
        15,
        12
      ],
      [
        2,
        3,
        0,
        1,
        6,
        7,
        4,
        5,
        10,
        11,
        8,
        9,
        14,
        15,
        12,
        13
      ],
      [
        3,
        0,
        1,
        2,
        7,
        4,
        5,
        6,
        11,
        8,
        9,
        10,
        15,
        12,
        13,
        14
      ],
      [
        4,
        5,
        6,
        7,
        8,
        9,
        10,
        11,
        12,
        13,
        14,
        15,
        0,
        1,
        2,
        3
      ],
      [
        5,
        6,
        7,
        4,
        9,
        10,
        11,
        8,
        13,
        14,
        15,
        12,
        1,
        2,
        3,
        0
      ],
      [
        6,
        7,
        4,
        5,
        10,
        11,
        8,
        9,
        14,
        15,
        12,
        13,
        2,
        3,
        0,
        1
      ],
      [
        7,
        4,
        5,
        6,
        11,
        8,
        9,
        10,
        15,
        12,
        13,
        14,
        3,
        0,
        1,
        2
      ],
      [
        8,
        9,
        10,
        11,
        12,
        13,
        14,
        15,
        0,
        1,
        2,
        3,
        4,
        5,
        6,
        7
      ],
      [
        9,
        10,
        11,
        8,
        13,
        14,
        15,
        12,
        1,
        2,
        3,
        0,
        5,
        6,
        7,
        4
      ],
      [
        10,
        11,
        8,
        9,
        14,
        15,
        12,
        13,
        2,
        3,
        0,
        1,
        6,
        7,
        4,
        5
      ],
      [
        11,
        8,
        9,
        10,
        15,
        12,
        13,
        14,
        3,
        0,
        1,
        2,
        7,
        4,
        5,
        6
      ],
      [
        12,
        13,
        14,
        15,
        0,
        1,
        2,
        3,
        4,
        5,
        6,
        7,
        8,
        9,
        10,
        11
      ],
      [
        13,
        14,
        15,
        12,
        1,
        2,
        3,
        0,
        5,
        6,
        7,
        4,
        9,
        10,
        11,
        8
      ],
      [
        14,
        15,
        12,
        13,
        2,
        3,
        0,
        1,
        6,
        7,
        4,
        5,
        10,
        11,
        8,
        9
      ],
      [
        15,
        12,
        13,
        14,
        3,
        0,
        1,
        2,
        7,
        4,
        5,
        6,
        11,
        8,
        9,
        10
      ]
    ]
  },
  "a_generator_indices": [
    4,
    12
  ],
  "b_generator_indices": [
    1,
    3
  ],
  "local_codes": {
    "matrix_role": "parity_check",
    "field": "GF(2)",
    "h_a": [
      [
        1,
        1
      ]
    ],
    "h_b": [
      [
        1,
        1
      ]
    ]
  },
  "expected_css": {
    "n": 16,
    "k": 2,
    "expected_distance": 4
  }
}
```

## Bad Example: Non-Symmetric Generator Set

This example removes the inverse of generator `4`, so `A` is not symmetric and
must be rejected before face enumeration.

<!-- quantum_tanner_contract:bad_non_symmetric_generator -->
```json
{
  "example_id": "bad_non_symmetric_generator",
  "construction_mode": "lr_cayley_no_cover_v1",
  "base_group": "same as toric_d4",
  "a_generator_indices": [
    4
  ],
  "b_generator_indices": [
    1,
    3
  ],
  "expected_error": "NonSymmetricGeneratorSet"
}
```

## Non-Goals

Do not implement GAP/Oscar integration, SmallGroup lookup, group search,
Morgenstern/Ramanujan constructors, qLDPC runtime calls, Julia runtime calls,
CLI parsing, or matrix generation in this documentation issue.
```

The JSON blocks above are the concrete examples to paste into the document.

- [ ] **Step 4: Run the focused test to verify GREEN**

Run: `cargo test -p qec-code quantum_tanner_contract_examples_compile -q`

Expected: PASS.

- [ ] **Step 5: Run the full crate tests required by Agent Desk**

Run: `cargo test -p qec-code quantum_tanner_contract_examples_compile -q`

Expected: PASS.

Run: `cargo test`

Expected: PASS for the workspace test suite.

- [ ] **Step 6: Commit the implementation**

```bash
git add qec-code/doc/quantum_tanner.md qec-code/tests/code.rs docs/superpowers/plans/2026-06-24-issue-177-quantum-tanner-contract.md
git commit -m "docs: add quantum tanner contract"
```

---

## Self-Review

- Spec coverage: Task 1 covers the contract document, the doc-backed test, the bad generator negative control, and the toric counting convention.
- Placeholder scan: no template placeholders or unresolved implementation notes remain in this plan.
- Type consistency: the test uses only `serde_json::Value`, local helper functions, and existing imports already present in `qec-code/tests/code.rs`.
