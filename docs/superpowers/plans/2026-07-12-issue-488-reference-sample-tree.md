# Issue 488 Reference Sample Tree Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an independently tested compressed representation for reference measurement output.

**Architecture:** Implement `ReferenceSampleTree` as a standalone `rstim::reference_sample_tree` module. Keep it independent from circuit execution and the existing flat reference-sample API; tests exercise structural compression behavior directly.

**Tech Stack:** Rust 2024 Cargo workspace, `rstim` integration tests, no new dependencies.

## Global Constraints

- The tree representation is `prefix_bits: Vec<bool>`, `suffix_children: Vec<ReferenceSampleTree>`, and `repetitions: u64`.
- Provide `size`, `decompress_into`, `simplified`, `try_factorize(2|3|5)`, and deterministic structural equality.
- Port the relevant Stim v1.15.0 simplification, factorization, and decompression behavior.
- A tree representing `(10)` repeated 50 times must have size 100 and decompress to exactly `1010...10`.
- Reversing child order during decompression must fail the prefix-plus-two-children fixture.
- Merging non-identical adjacent children during factorization must fail the factorization fixture.
- Do not execute circuits, compare tableau states, or change the public flat reference-sample API.

---

### Task 1: Add ReferenceSampleTree And Focused Tests

**Files:**
- Create: `rstim/src/reference_sample_tree.rs`
- Modify: `rstim/src/lib.rs`
- Create: `rstim/tests/reference_sample_tree.rs`

**Interfaces:**
- Consumes: no production interfaces beyond the crate module system.
- Produces: `rstim::reference_sample_tree::ReferenceSampleTree` with public fields and public methods `empty`, `size`, `decompress_into`, `simplified`, and `try_factorize`.

- [ ] **Step 1: Write the failing integration tests**

Create `rstim/tests/reference_sample_tree.rs` with:

```rust
use rstim::reference_sample_tree::ReferenceSampleTree;

fn leaf(bits: &[bool], repetitions: u64) -> ReferenceSampleTree {
    ReferenceSampleTree {
        prefix_bits: bits.to_vec(),
        suffix_children: Vec::new(),
        repetitions,
    }
}

fn decompress(tree: &ReferenceSampleTree) -> Vec<bool> {
    let mut out = Vec::new();
    tree.decompress_into(&mut out);
    out
}

fn repeated_pair_10(count: usize) -> Vec<bool> {
    let mut out = Vec::with_capacity(count * 2);
    for _ in 0..count {
        out.push(true);
        out.push(false);
    }
    out
}

#[test]
fn decompresses_prefix_and_children_in_order() {
    let tree = ReferenceSampleTree {
        prefix_bits: vec![true, true, false, true],
        suffix_children: vec![
            leaf(&[true, false, true], 2),
            leaf(&[false, false], 1),
        ],
        repetitions: 1,
    };

    assert_eq!(
        decompress(&tree),
        vec![
            true, true, false, true, true, false, true, true, false, true, false, false,
        ]
    );
}

#[test]
fn identical_children_factor_into_repetitions() {
    let raw = ReferenceSampleTree {
        prefix_bits: Vec::new(),
        suffix_children: vec![
            leaf(&[true, false], 1),
            leaf(&[true, false], 1),
            leaf(&[true, false], 1),
            leaf(&[true, false], 1),
        ],
        repetitions: 1,
    };

    assert_eq!(raw.simplified(), leaf(&[true, false], 4));
}

#[test]
fn nested_repetitions_preserve_flat_bits() {
    let tree = ReferenceSampleTree {
        prefix_bits: Vec::new(),
        suffix_children: vec![ReferenceSampleTree {
            prefix_bits: vec![true],
            suffix_children: vec![leaf(&[false], 1)],
            repetitions: 50,
        }],
        repetitions: 1,
    };

    assert_eq!(tree.size(), 100);
    assert_eq!(decompress(&tree), repeated_pair_10(50));
    assert_eq!(tree.simplified(), leaf(&[true, false], 50));
}

#[test]
fn factorization_matches_stim_v1_15_cases() {
    let a = leaf(&[true], 1);
    let b = leaf(&[false, true], 2);
    let c = leaf(&[true, false, false], 1);

    let mut by_two = ReferenceSampleTree {
        prefix_bits: Vec::new(),
        suffix_children: vec![a.clone(), b.clone(), c.clone(), a.clone(), b.clone(), c.clone()],
        repetitions: 7,
    };
    by_two.try_factorize(2);
    assert_eq!(by_two.suffix_children, vec![a.clone(), b.clone(), c.clone()]);
    assert_eq!(by_two.repetitions, 14);

    let mut by_three = ReferenceSampleTree {
        prefix_bits: Vec::new(),
        suffix_children: vec![a.clone(), b.clone(), a.clone(), b.clone(), a.clone(), b.clone()],
        repetitions: 1,
    };
    by_three.try_factorize(3);
    assert_eq!(by_three.suffix_children, vec![a.clone(), b.clone()]);
    assert_eq!(by_three.repetitions, 3);

    let mut by_five = ReferenceSampleTree {
        prefix_bits: Vec::new(),
        suffix_children: vec![c.clone(), c.clone(), c.clone(), c.clone(), c.clone()],
        repetitions: 2,
    };
    by_five.try_factorize(5);
    assert_eq!(by_five.suffix_children, vec![c.clone()]);
    assert_eq!(by_five.repetitions, 10);

    let mut prefixed = ReferenceSampleTree {
        prefix_bits: vec![true],
        suffix_children: vec![a.clone(), a.clone()],
        repetitions: 1,
    };
    let prefixed_before = prefixed.clone();
    prefixed.try_factorize(2);
    assert_eq!(prefixed, prefixed_before);

    let mut non_periodic = ReferenceSampleTree {
        prefix_bits: Vec::new(),
        suffix_children: vec![a.clone(), b.clone(), a, c],
        repetitions: 1,
    };
    let non_periodic_before = non_periodic.clone();
    non_periodic.try_factorize(2);
    assert_eq!(non_periodic, non_periodic_before);
}

#[test]
fn size_matches_decompressed_length() {
    let tree = ReferenceSampleTree {
        prefix_bits: vec![true],
        suffix_children: vec![
            leaf(&[false, true], 3),
            ReferenceSampleTree {
                prefix_bits: vec![false],
                suffix_children: vec![leaf(&[true, true], 2)],
                repetitions: 4,
            },
        ],
        repetitions: 5,
    };

    let decompressed = decompress(&tree);
    assert_eq!(tree.size(), decompressed.len());
    assert_eq!(tree.size(), 135);
}

#[test]
fn empty_and_equality_are_structural() {
    let empty1 = ReferenceSampleTree {
        prefix_bits: Vec::new(),
        suffix_children: Vec::new(),
        repetitions: 0,
    };
    let empty2 = ReferenceSampleTree::default();

    assert!(empty1.empty());
    assert_eq!(empty1, empty2);
    assert_ne!(empty1, leaf(&[], 1));
    assert_ne!(empty1, leaf(&[false], 0));
    assert_ne!(
        empty1,
        ReferenceSampleTree {
            prefix_bits: Vec::new(),
            suffix_children: vec![ReferenceSampleTree::default()],
            repetitions: 0,
        }
    );

    let nested_empty = ReferenceSampleTree {
        prefix_bits: Vec::new(),
        suffix_children: vec![ReferenceSampleTree::default()],
        repetitions: 9,
    };
    assert!(nested_empty.empty());
}

#[test]
fn simplified_matches_stim_v1_15_empty_and_repetition_case() {
    let raw = ReferenceSampleTree {
        prefix_bits: Vec::new(),
        suffix_children: vec![
            ReferenceSampleTree {
                prefix_bits: Vec::new(),
                suffix_children: Vec::new(),
                repetitions: 1,
            },
            ReferenceSampleTree {
                prefix_bits: vec![true, false, true],
                suffix_children: vec![ReferenceSampleTree::default()],
                repetitions: 0,
            },
            leaf(&[true, true, true], 2),
        ],
        repetitions: 3,
    };

    assert_eq!(raw.simplified(), leaf(&[true, true, true], 6));
}
```

- [ ] **Step 2: Run the focused test to verify RED**

Run:

```sh
cargo test -p rstim --test reference_sample_tree -- --nocapture
```

Expected: FAIL because `rstim::reference_sample_tree` does not exist yet.

- [ ] **Step 3: Add the module export**

Add this line to `rstim/src/lib.rs` near the other `pub mod` declarations:

```rust
pub mod reference_sample_tree;
```

- [ ] **Step 4: Implement the tree module**

Create `rstim/src/reference_sample_tree.rs` with:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceSampleTree {
    pub prefix_bits: Vec<bool>,
    pub suffix_children: Vec<ReferenceSampleTree>,
    pub repetitions: u64,
}

impl Default for ReferenceSampleTree {
    fn default() -> Self {
        Self {
            prefix_bits: Vec::new(),
            suffix_children: Vec::new(),
            repetitions: 0,
        }
    }
}

impl ReferenceSampleTree {
    pub fn empty(&self) -> bool {
        if self.repetitions == 0 {
            return true;
        }
        if !self.prefix_bits.is_empty() {
            return false;
        }
        self.suffix_children.iter().all(Self::empty)
    }

    pub fn size(&self) -> usize {
        let mut body_size = self.prefix_bits.len();
        for child in &self.suffix_children {
            body_size = body_size
                .checked_add(child.size())
                .expect("reference sample tree size overflow");
        }
        let repetitions = usize::try_from(self.repetitions)
            .expect("reference sample tree repetitions do not fit in usize");
        body_size
            .checked_mul(repetitions)
            .expect("reference sample tree size overflow")
    }

    pub fn decompress_into(&self, output: &mut Vec<bool>) {
        for _ in 0..self.repetitions {
            output.extend(self.prefix_bits.iter().copied());
            for child in &self.suffix_children {
                child.decompress_into(output);
            }
        }
    }

    pub fn simplified(&self) -> Self {
        let mut flat = Vec::new();
        self.flatten_and_simplify_into(&mut flat);
        match flat.len() {
            0 => Self::default(),
            1 => flat.pop().expect("single simplified tree"),
            _ => {
                let first_is_payload =
                    flat[0].repetitions == 1 && flat[0].suffix_children.is_empty();
                if first_is_payload {
                    let mut iter = flat.into_iter();
                    let mut result = iter.next().expect("first simplified tree");
                    result.suffix_children = iter.collect();
                    result
                } else {
                    Self {
                        prefix_bits: Vec::new(),
                        suffix_children: flat,
                        repetitions: 1,
                    }
                }
            }
        }
    }

    pub fn try_factorize(&mut self, period_factor: usize) {
        if period_factor == 0
            || !self.prefix_bits.is_empty()
            || self.suffix_children.len() % period_factor != 0
        {
            return;
        }

        let period_len = self.suffix_children.len() / period_factor;
        for k in period_len..self.suffix_children.len() {
            if self.suffix_children[k - period_len] != self.suffix_children[k] {
                return;
            }
        }

        self.suffix_children.truncate(period_len);
        self.repetitions = self
            .repetitions
            .checked_mul(period_factor as u64)
            .expect("reference sample tree repetitions overflow");
    }

    fn flatten_and_simplify_into(&self, out: &mut Vec<Self>) {
        if self.repetitions == 0 {
            return;
        }

        let mut flattened = Vec::new();
        if !self.prefix_bits.is_empty() {
            flattened.push(Self {
                prefix_bits: self.prefix_bits.clone(),
                suffix_children: Vec::new(),
                repetitions: 1,
            });
        }
        for child in &self.suffix_children {
            child.flatten_and_simplify_into(&mut flattened);
        }

        let mut fused: Vec<Self> = Vec::new();
        for src in flattened {
            if let Some(dst) = fused.last_mut() {
                if dst.prefix_bits == src.prefix_bits
                    && dst.suffix_children == src.suffix_children
                {
                    dst.repetitions = dst
                        .repetitions
                        .checked_add(src.repetitions)
                        .expect("reference sample tree repetitions overflow");
                    continue;
                }
                if src.repetitions == 1 && dst.repetitions == 1 && dst.suffix_children.is_empty()
                {
                    dst.prefix_bits.extend(src.prefix_bits);
                    dst.suffix_children = src.suffix_children;
                    continue;
                }
            }
            fused.push(src);
        }

        if self.repetitions == 1 {
            out.extend(fused);
        } else if fused.len() == 1 {
            let mut only = fused.pop().expect("single fused tree");
            only.repetitions = only
                .repetitions
                .checked_mul(self.repetitions)
                .expect("reference sample tree repetitions overflow");
            out.push(only);
        } else if fused.is_empty() {
        } else if fused[0].suffix_children.is_empty() && fused[0].repetitions == 1 {
            let mut iter = fused.into_iter();
            let mut result = iter.next().expect("first fused tree");
            result.repetitions = self.repetitions;
            result.suffix_children = iter.collect();
            out.push(result);
        } else {
            out.push(Self {
                prefix_bits: Vec::new(),
                suffix_children: fused,
                repetitions: self.repetitions,
            });
        }
    }
}
```

- [ ] **Step 5: Run the focused test to verify GREEN**

Run:

```sh
cargo test -p rstim --test reference_sample_tree -- --nocapture
```

Expected: PASS, including all required test names.

- [ ] **Step 6: Run the full workspace test gate**

Run:

```sh
cargo test
```

Expected: PASS.

- [ ] **Step 7: Commit**

Run:

```sh
git add rstim/src/lib.rs rstim/src/reference_sample_tree.rs rstim/tests/reference_sample_tree.rs
git commit -m "feat: add reference sample tree"
```
