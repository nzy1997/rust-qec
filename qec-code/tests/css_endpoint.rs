use qec_code::Pauli;
use qec_code::css_endpoint::{
    CssEndpointDecoder, EndpointError, EndpointLimits, EndpointScore, PauliFrame, Plane,
};
use qec_code::phased_pauli::{Phase, PhasedPauli, SignedStabilizerGroup, ValidatedLogicalBasis};

fn op(n: usize, x: u64, z: u64) -> PhasedPauli {
    PhasedPauli::new(
        Pauli::from_xz_bits(
            (0..n).map(|q| ((x >> q) & 1) as u8).collect(),
            (0..n).map(|q| ((z >> q) & 1) as u8).collect(),
        )
        .unwrap(),
        Phase::from_exponent((x & z).count_ones() as u8 % 2),
    )
}
fn limits(n: usize, masks: usize) -> EndpointLimits {
    EndpointLimits {
        max_physical_qubits: n,
        max_masks_per_plane: masks,
    }
}
fn steane() -> (SignedStabilizerGroup, ValidatedLogicalBasis) {
    // Constructor under test uses the built-in code; the reference below uses
    // literal independent parity masks, not that generator or decoder's columns.
    let code = qec_code::codes::steane::Steane::new().unwrap();
    let checks = code
        .code()
        .stabilizers()
        .iter()
        .cloned()
        .map(|p| PhasedPauli::new(p, Phase::PlusOne))
        .collect();
    let g = SignedStabilizerGroup::new(7, checks).unwrap();
    let b = g
        .validate_logical_basis(vec![op(7, 127, 0)], vec![op(7, 0, 127)])
        .unwrap();
    (g, b)
}
fn literal_syndrome(mask: u64) -> usize {
    [105u64, 90, 116].iter().enumerate().fold(0, |s, (i, r)| {
        s | (((mask & r).count_ones() as usize % 2) << i)
    })
}
fn literal_recovery(syndrome: usize) -> u64 {
    if syndrome == 0 {
        return 0;
    }
    (0..7)
        .map(|q| 1 << q)
        .find(|m| literal_syndrome(*m) == syndrome)
        .unwrap()
}
#[test]
fn steane_exhaustive_frames_match_independent_hamming_reference() {
    let (g, b) = steane();
    let d = CssEndpointDecoder::new(&g, &b, 1, limits(7, 8)).unwrap();
    assert_eq!(d.x_table().entries().len(), 8);
    assert_eq!(d.z_table().entries().len(), 8);
    for x in 0..128u64 {
        for z in 0..128u64 {
            let rx = literal_recovery(literal_syndrome(x));
            let rz = literal_recovery(literal_syndrome(z));
            let EndpointScore::Covered {
                correction,
                residual,
                correct,
            } = d.score(PauliFrame { x, z }).unwrap()
            else {
                panic!("Steane CSS syndrome must be covered");
            };
            assert_eq!(correction, PauliFrame { x: rx, z: rz });
            assert_eq!(residual.syndrome, 0);
            let logical = u128::from((z ^ rz).count_ones() % 2)
                | (u128::from((x ^ rx).count_ones() % 2) << 1);
            assert_eq!(residual.logical, logical);
            assert_eq!(correct, logical == 0);
        }
    }
    // Includes X_i Z_j with i != j (weight two), beyond a single-Pauli sphere.
    for x in std::iter::once(0).chain((0..7).map(|q| 1 << q)) {
        for z in std::iter::once(0).chain((0..7).map(|q| 1 << q)) {
            assert!(matches!(
                d.score(PauliFrame { x, z }).unwrap(),
                EndpointScore::Covered { correct: true, .. }
            ));
        }
    }
}
#[test]
fn consistent_coset_duplicates_choose_support_lexicographically() {
    let g = SignedStabilizerGroup::new(2, vec![op(2, 3, 0), op(2, 0, 3), op(2, 3, 0)]).unwrap();
    let b = g.validate_logical_basis(vec![], vec![]).unwrap();
    let d = CssEndpointDecoder::new(&g, &b, 1, limits(2, 3)).unwrap();
    assert_eq!(d.x_table().enumerated_masks(), 3);
    assert_eq!(d.x_table().entries().len(), 2);
    assert_eq!(d.x_table().entries()[&2].mask, 1);
    // Independent exhaustive 16-Pauli Bell reference: every residual is one of
    // I, XX, ZZ, XXZZ. Duplicate declared checks preserve their bit positions.
    for x in 0..4 {
        for z in 0..4 {
            let EndpointScore::Covered {
                correction,
                correct,
                ..
            } = d.score(PauliFrame { x, z }).unwrap()
            else {
                panic!();
            };
            assert!([0, 3].contains(&(x ^ correction.x)));
            assert!([0, 3].contains(&(z ^ correction.z)));
            assert!(correct);
        }
    }
}
#[test]
fn too_small_distance_rejects_conflicting_logical_labels() {
    let g = SignedStabilizerGroup::new(3, vec![op(3, 0, 3), op(3, 0, 6)]).unwrap();
    let b = g
        .validate_logical_basis(vec![op(3, 7, 0)], vec![op(3, 0, 1)])
        .unwrap();
    assert!(matches!(
        CssEndpointDecoder::new(&g, &b, 1, limits(3, 4)),
        Err(EndpointError::LogicalCollision {
            plane: Plane::Z,
            syndrome: 0,
            first_mask: 0,
            second_mask: 1,
            ..
        })
    ));
}
#[test]
fn uncovered_wrong_covered_and_invalid_frames_are_distinct() {
    let (g, b) = steane();
    let d = CssEndpointDecoder::new(&g, &b, 0, limits(7, 1)).unwrap();
    assert!(matches!(
        d.score(PauliFrame { x: 1, z: 0 }).unwrap(),
        EndpointScore::Uncovered {
            x_covered: false,
            z_covered: true,
            ..
        }
    ));
    assert!(matches!(
        d.score(PauliFrame { x: 127, z: 0 }).unwrap(),
        EndpointScore::Covered { correct: false, .. }
    ));
    assert_eq!(
        d.score(PauliFrame { x: 128, z: 0 }),
        Err(EndpointError::FrameWidth { n: 7 })
    );
}
#[test]
fn bounds_and_mixed_checks_reject_before_enumeration() {
    let (g, b) = steane();
    assert_eq!(
        CssEndpointDecoder::new(&g, &b, 1, limits(0, 8)).unwrap_err(),
        EndpointError::InvalidWidthCap(0)
    );
    assert_eq!(
        CssEndpointDecoder::new(&g, &b, 1, limits(65, 8)).unwrap_err(),
        EndpointError::InvalidWidthCap(65)
    );
    assert_eq!(
        CssEndpointDecoder::new(&g, &b, 1, limits(6, 8)).unwrap_err(),
        EndpointError::WidthLimit {
            actual: 7,
            limit: 6
        }
    );
    assert_eq!(
        CssEndpointDecoder::new(&g, &b, 8, limits(7, 8)).unwrap_err(),
        EndpointError::RadiusLimit { radius: 8, n: 7 }
    );
    assert_eq!(
        CssEndpointDecoder::new(&g, &b, 1, limits(7, 7)).unwrap_err(),
        EndpointError::MaskLimit {
            required: 8,
            limit: 7
        }
    );
    assert_eq!(
        CssEndpointDecoder::new(&g, &b, 1, limits(7, 0)).unwrap_err(),
        EndpointError::InvalidMaskCap(0)
    );
    assert_eq!(
        CssEndpointDecoder::new(&g, &b, 1, limits(7, 1_000_001)).unwrap_err(),
        EndpointError::InvalidMaskCap(1_000_001)
    );
    let mixed = SignedStabilizerGroup::new(2, vec![op(2, 1, 2)]).unwrap();
    assert!(matches!(
        CssEndpointDecoder::new(&mixed, &b, 1, limits(2, 3)),
        Err(EndpointError::NonCssCheck { index: 0 })
    ));
    let redundant = SignedStabilizerGroup::new(1, vec![op(1, 0, 1); 129]).unwrap();
    assert_eq!(
        CssEndpointDecoder::new(&redundant, &b, 0, limits(1, 1)).unwrap_err(),
        EndpointError::CheckLimit { actual: 129 }
    );
    // Both counts and queries use the high coordinate without shifting by64.
    let large =
        SignedStabilizerGroup::new(64, (0..64).map(|q| op(64, 0, 1 << q)).collect()).unwrap();
    let empty = large.validate_logical_basis(vec![], vec![]).unwrap();
    assert_eq!(
        CssEndpointDecoder::new(&large, &empty, 64, limits(64, 1_000_000)).unwrap_err(),
        EndpointError::MaskLimit {
            required: 1u128 << 64,
            limit: 1_000_000
        }
    );
    let large_d = CssEndpointDecoder::new(&large, &empty, 0, limits(64, 1)).unwrap();
    assert_eq!(large_d.x_columns()[63].syndrome, 1u128 << 63);
}
#[test]
fn logical_basis_is_revalidated_against_the_supplied_code() {
    let (g, b) = steane();
    let other = SignedStabilizerGroup::new(7, (0..7).map(|q| op(7, 0, 1 << q)).collect()).unwrap();
    assert!(matches!(
        CssEndpointDecoder::new(&other, &b, 0, limits(7, 1)),
        Err(EndpointError::Algebra(_))
    ));
    assert!(CssEndpointDecoder::new(&g, &b, 0, limits(7, 1)).is_ok());
}
fn monomial(vars: &[usize]) -> u64 {
    (0..64)
        .filter(|q| vars.iter().all(|v| q & (1 << v) != 0))
        .fold(0, |m, q| m | (1 << q))
}
#[test]
fn qrm64_three_error_planes_have_43745_compatible_masks_each() {
    // Independently declared RM(2,6) evaluations and complementary cubic pairs.
    let mut small = vec![monomial(&[])];
    let mut triples = vec![];
    for a in 0..6 {
        small.push(monomial(&[a]));
        for b in a + 1..6 {
            small.push(monomial(&[a, b]));
            for c in b + 1..6 {
                triples.push(vec![a, b, c]);
            }
        }
    }
    assert_eq!(small.len(), 22);
    assert_eq!(triples.len(), 20);
    let checks = small
        .iter()
        .map(|m| op(64, *m, 0))
        .chain(small.iter().map(|m| op(64, 0, *m)))
        .collect();
    let g = SignedStabilizerGroup::new(64, checks).unwrap();
    assert_eq!(g.rank(), 44);
    let lx = triples.iter().map(|v| op(64, monomial(v), 0)).collect();
    let lz = triples
        .iter()
        .map(|v| {
            op(
                64,
                0,
                monomial(&(0..6).filter(|q| !v.contains(q)).collect::<Vec<_>>()),
            )
        })
        .collect();
    let b = g.validate_logical_basis(lx, lz).unwrap();
    let d = CssEndpointDecoder::new(&g, &b, 3, limits(64, 43_745)).unwrap();
    for table in [d.x_table(), d.z_table()] {
        assert_eq!(table.enumerated_masks(), 43_745);
        assert_eq!(table.entries().len(), 43_745);
    }
    // All entries score exactly on each plane; product-decoding then covers
    // every quantum-weight<=3 Pauli because each binary plane has weight<=3.
    for entry in d.x_table().entries().values() {
        assert!(matches!(
            d.score(PauliFrame {
                x: entry.mask,
                z: 0
            })
            .unwrap(),
            EndpointScore::Covered { correct: true, .. }
        ));
    }
    for entry in d.z_table().entries().values() {
        assert!(matches!(
            d.score(PauliFrame {
                x: 0,
                z: entry.mask
            })
            .unwrap(),
            EndpointScore::Covered { correct: true, .. }
        ));
    }
    for q in 0..64 {
        let f = PauliFrame {
            x: (1 << q) | (1 << ((q + 7) % 64)),
            z: (1 << q) | (1 << ((q + 19) % 64)),
        };
        assert!(matches!(
            d.score(f).unwrap(),
            EndpointScore::Covered { correct: true, .. }
        ));
    }
}

#[test]
fn weight_two_ties_use_support_list_order_instead_of_integer_order() {
    let g = SignedStabilizerGroup::new(
        4,
        vec![op(4, 15, 0), op(4, 0, 3), op(4, 0, 6), op(4, 0, 12)],
    )
    .unwrap();
    let b = g.validate_logical_basis(vec![], vec![]).unwrap();
    let d = CssEndpointDecoder::new(&g, &b, 2, limits(4, 11)).unwrap();
    // [0,3] precedes [1,2] lexicographically, although mask9 > mask6.
    assert_eq!(d.x_table().entries()[&10].mask, 9);
    assert_eq!(d.x_table().entries()[&10].logical, 0);
}
