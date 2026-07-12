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
