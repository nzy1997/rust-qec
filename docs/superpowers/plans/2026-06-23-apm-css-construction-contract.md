# APM CSS Construction Contract Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a crate-local APM-CSS construction contract note and a focused verifier test for its examples.

**Architecture:** Keep the actual developer note in `qec-code/doc/apm_css.md`, near the crate and its fixture vocabulary. Keep all executable checks in `qec-code/tests/code.rs` as test-local helpers that read the merged Table A1 manifest and exercise the documented arithmetic without adding public generator APIs.

**Tech Stack:** Rust 2024, qec-code integration tests, serde_json, Markdown documentation.

## Global Constraints

- Actual contract note path is exactly `qec-code/doc/apm_css.md`.
- Add focused test `apm_contract_doc_examples_compile` in `qec-code/tests/code.rs`.
- Do not implement a native APM generator.
- Do not generate or commit `Hx` or `Hz` sparse matrix fixtures.
- Do not add public Rust APIs or new dependencies.
- Use `qec-code/tests/fixtures/apm/table_a1_manifest.json` as the checked fixture source from #132.
- Cite #132 as the fixture manifest source and #133 as the known-answer sparse fixture target.
- Use offline cargo commands in this sandbox when registry access would otherwise be needed.

---

## File Structure

- Create `qec-code/doc/apm_css.md`: developer-facing construction contract with equations, parameter names, dimensions, sparse-row expectations, fixture links, and validation checklist.
- Modify `qec-code/tests/code.rs`: add test-local `DocumentedAffineMap` helpers plus `apm_contract_doc_examples_compile`.

### Task 1: APM CSS Contract Note And Verifier

**Files:**
- Create: `qec-code/doc/apm_css.md`
- Modify: `qec-code/tests/code.rs`

**Interfaces:**
- Consumes: `qec-code/tests/fixtures/apm/table_a1_manifest.json`
- Produces: test-local `DocumentedAffineMap { a: u64, b: u64, modulus: u64 }`
- Produces: test-local `parse_documented_affine_map(a: u64, b: u64, modulus: u64) -> Result<DocumentedAffineMap, String>`
- Produces: test-local `affine_commutation_residual(lhs: DocumentedAffineMap, rhs: DocumentedAffineMap) -> u64`
- Produces: test `apm_contract_doc_examples_compile`

- [ ] **Step 1: Add the failing verifier test and test-local helpers**

In `qec-code/tests/code.rs`, add the following helper code after `validate_apm_table_a1_manifest`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DocumentedAffineMap {
    a: u64,
    b: u64,
    modulus: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DocumentedApmShape {
    n: u64,
    mx: u64,
    mz: u64,
}

fn gcd_u64(mut lhs: u64, mut rhs: u64) -> u64 {
    while rhs != 0 {
        let next = lhs % rhs;
        lhs = rhs;
        rhs = next;
    }
    lhs
}

fn parse_documented_affine_map(
    a: u64,
    b: u64,
    modulus: u64,
) -> Result<DocumentedAffineMap, String> {
    if modulus == 0 {
        return Err("affine map modulus must be positive".to_owned());
    }
    if gcd_u64(a, modulus) != 1 {
        return Err(format!(
            "affine slope {a} is not a unit modulo {modulus}"
        ));
    }
    Ok(DocumentedAffineMap {
        a: a % modulus,
        b: b % modulus,
        modulus,
    })
}

fn mod_i128(value: i128, modulus: u64) -> u64 {
    let modulus = modulus as i128;
    value.rem_euclid(modulus) as u64
}

fn affine_commutation_residual(
    lhs: DocumentedAffineMap,
    rhs: DocumentedAffineMap,
) -> u64 {
    assert_eq!(
        lhs.modulus, rhs.modulus,
        "affine residual requires a shared modulus"
    );
    mod_i128(
        lhs.a as i128 * rhs.b as i128 + lhs.b as i128
            - rhs.a as i128 * lhs.b as i128
            - rhs.b as i128,
        lhs.modulus,
    )
}

fn documented_apm_shape(p: u64, j: u64, l: u64) -> DocumentedApmShape {
    DocumentedApmShape {
        n: p * l,
        mx: p * j,
        mz: p * j,
    }
}

fn apm_entry_by_code_id<'a>(manifest: &'a Value, code_id: &str) -> &'a Value {
    manifest["entries"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["code_id"] == code_id)
        .unwrap()
}

fn u64_json(value: &Value) -> u64 {
    value.as_u64().unwrap()
}

fn documented_manifest_map(
    entry: &Value,
    label: &str,
    modulus: u64,
) -> Result<DocumentedAffineMap, String> {
    let label = label.strip_prefix("column_component:").unwrap_or(label);
    let (family, index) = label.split_at(1);
    let index: usize = index.parse().unwrap();
    let map = &entry[family][index];
    match family {
        "f" => parse_documented_affine_map(u64_json(&map["a"]), u64_json(&map["b"]), modulus),
        "g" => parse_documented_affine_map(u64_json(&map["c"]), u64_json(&map["d"]), modulus),
        _ => panic!("unknown APM family label {label}"),
    }
}
```

Then add this test near the existing APM manifest tests:

```rust
#[test]
fn apm_contract_doc_examples_compile() {
    let doc = include_str!("../doc/apm_css.md");
    assert!(doc.contains("AffineMap { a, b, modulus }"));
    assert!(doc.contains("Delta"));
    assert!(doc.contains("Gamma"));
    assert!(doc.contains("qec-code/tests/fixtures/apm/table_a1_manifest.json"));

    let manifest: Value =
        serde_json::from_str(include_str!("fixtures/apm/table_a1_manifest.json")).unwrap();
    let p96 = apm_entry_by_code_id(&manifest, "apm_kasai:p=96");

    assert_eq!(
        documented_apm_shape(
            u64_json(&p96["P"]),
            u64_json(&p96["J"]),
            u64_json(&p96["L"])
        ),
        DocumentedApmShape {
            n: 1152,
            mx: 288,
            mz: 288,
        }
    );

    let gamma_pair = &p96["required_commuting_pairs"][0];
    let gamma_modulus = u64_json(&gamma_pair["modulus"]);
    let gamma_left = documented_manifest_map(
        p96,
        gamma_pair["left"].as_str().unwrap(),
        gamma_modulus,
    )
    .unwrap();
    let gamma_right = documented_manifest_map(
        p96,
        gamma_pair["right"].as_str().unwrap(),
        gamma_modulus,
    )
    .unwrap();
    assert_eq!(affine_commutation_residual(gamma_left, gamma_right), 0);

    let noncommuting_pair = &p96["required_noncommuting_pairs"][0];
    let noncommuting_left = documented_manifest_map(
        p96,
        &format!("f{}", u64_json(&noncommuting_pair["left_index"])),
        u64_json(&p96["P"]),
    )
    .unwrap();
    let noncommuting_right = documented_manifest_map(
        p96,
        &format!("g{}", u64_json(&noncommuting_pair["right_index"])),
        u64_json(&p96["P"]),
    )
    .unwrap();
    assert_ne!(
        affine_commutation_residual(noncommuting_left, noncommuting_right),
        0
    );

    let invalid = parse_documented_affine_map(2, 0, 96).unwrap_err();
    assert!(invalid.contains("not a unit modulo 96"));
}
```

- [ ] **Step 2: Run the focused test and verify RED**

Run:

```bash
cargo test -p qec-code apm_contract_doc_examples_compile -q --offline
```

Expected: FAIL at compile time because `qec-code/doc/apm_css.md` does not exist yet. That proves the verifier is wired to the missing note.

- [ ] **Step 3: Add the contract note**

Create `qec-code/doc/apm_css.md` with these required sections and values:

```markdown
# APM-CSS Construction Contract

This note is the implementation contract for the APM-CSS Table A1 fixtures from
arXiv:2604.16209. It translates the paper and the Kasai reference-code
vocabulary into the data model used by `qec-code`.

## Fixture Scope

- Source manifest: `qec-code/tests/fixtures/apm/table_a1_manifest.json` from
  <https://github.com/nzy1997/rstim/issues/132>.
- Known-answer sparse fixture target:
  <https://github.com/nzy1997/rstim/issues/133>.
- Paper source: <https://arxiv.org/abs/2604.16209>, Appendix A and Table A1.
- Construction background: <https://arxiv.org/abs/2601.08824>, active
  orthogonality and affine permutation construction.
- Reference-code paths when available locally:
  `drafts/construct_apm_css_code/README.md` and
  `drafts/construct_apm_css_code/apm_g8_mod.cpp`.

## Data Model

Use `AffineMap { a, b, modulus }` for every affine permutation:

```rust
struct AffineMap {
    a: u64,
    b: u64,
    modulus: u64,
}
```

It represents `x -> a*x + b (mod modulus)`. A map is valid only when
`gcd(a, modulus) == 1`; a non-unit slope such as `{ a: 2, b: 0, modulus: 96 }`
must be rejected.

Manifest `f_i=(a_i,b_i)` entries map directly to `AffineMap`. Manifest
`g_i=(c_i,d_i)` entries map to the same struct with `a=c_i` and `b=d_i`.

## Shape

For the checked Table A1 instances, `J=3`, `L=12`, and `L2=L/2=6`.
The active matrices use the top `J` block rows from the parent block-circulant
template, so each side has `J*P` active check rows and `L*P` data columns:

```text
n  = L * P
mx = J * P
mz = J * P
```

For `P=96,J=3,L=12`, this gives `n=1152` and `mx=mz=288`. For `P=192`, this
gives `n=2304` and `mx=mz=576`.

## Delta And Gamma

Let the active block-row set be `A = {0, 1, ..., J-1}` for the standard top-row
choice. The active difference set is:

```text
Delta = { (r - s) mod L | r in A, s in A }
```

The construction needs affine block pairs that contribute to active
differences in `Delta` to commute. In this repository vocabulary, `Gamma` is
the explicit list of such checked pairs carried by
`required_commuting_pairs` in the manifest. For the checked P=96 and P=192
entries those pairs are column-component constraints evaluated modulo
`column_component_modulus`, not necessarily modulo the full `P`.

The latent/noncommuting controls are the manifest
`required_noncommuting_pairs`; for Table A1 they are interpreted as
`f[left_index]` against `g[right_index]` over the full modulus `P`.

## Commutation Residual

For two affine maps `u(x)=a*x+b` and `v(x)=c*x+d` over the same modulus `M`,
the maps commute exactly when `u(v(x)) == v(u(x))`. The linear terms match
automatically, so the implementation residual is:

```text
residual(u, v) = (a*d + b - c*b - d) mod M
```

The maps commute iff `residual == 0`.

For the P=96 manifest entry, `required_commuting_pairs[0]` is a Gamma pair
checked modulo `32` and has residual zero. A documented noncommuting control is
`f0` against `g3` over modulus `96`; its residual is nonzero.

## Sparse-Row Output Contract

The future generator should emit `SparseRowsMatrix` JSON compatible with
`qec-code/src/css.rs`:

- `Hx.num_cols == Hz.num_cols == n`
- `Hx.rows.len() == mx`
- `Hz.rows.len() == mz`
- every row support is sorted, unique, and in range
- for the Table A1 fixtures, every row has weight `L=12`
- every data column has X degree `J=3` and Z degree `J=3`
- `Hx * Hz^T == 0 mod 2`

The P=96 known-answer matrices belong to #133; this note does not generate or
pin them.

## Validation Checklist

- Parse every `f` and `g` entry as an affine map and reject non-unit slopes.
- Check `L2 == L/2`.
- Check `n`, `mx`, and `mz` from `P`, `J`, and `L`.
- Check every manifest Gamma pair has zero residual under its documented
  modulus.
- Check at least one manifest noncommuting pair has nonzero residual under the
  full `P`.
- Keep distance values as upper-bound metadata from Table A1, not exact
  minimum-distance claims.
```

- [ ] **Step 4: Run the focused test and verify GREEN**

Run:

```bash
cargo test -p qec-code apm_contract_doc_examples_compile -q --offline
```

Expected: PASS. The test should report `1 passed` in `qec-code/tests/code.rs`
and all other test binaries filtered out.

- [ ] **Step 5: Run formatting**

Run:

```bash
cargo fmt --all
```

Expected: no output and no unrelated formatting churn.

- [ ] **Step 6: Run the full verification gates**

Run:

```bash
cargo test -p qec-code apm_contract_doc_examples_compile -q --offline
cargo test --offline
```

Expected: both pass. `cargo test --offline` may still print the pre-existing
`rmatching/tests/coverage.rs` warnings about `saw_same_tree`.

- [ ] **Step 7: Commit implementation**

Run:

```bash
git add qec-code/tests/code.rs qec-code/doc/apm_css.md docs/superpowers/plans/2026-06-23-apm-css-construction-contract.md
git commit -m "docs: add apm css construction contract"
```
