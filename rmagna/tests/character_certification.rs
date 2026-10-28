use rmagna::catalogue::{
    CatalogueLimits, CertificateVerificationMethod, CliffordIndicator, FrozenPhasePlan,
    PhaseCatalogue, PhaseDecompositionInput, PhaseSelectionOptions, PhaseSelectionRoute,
};
use rmagna::gf2::BitVector;
use rmagna::phase::ParityPhase;
use rmagna::scalar::{ExactScalar, ScalarArithmetic};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

struct ObservedAllocator;
thread_local! {
    static WATCH_SIZE: Cell<usize> = const { Cell::new(0) };
    static DENSE_ALLOCS: Cell<usize> = const { Cell::new(0) };
}
fn observe(size: usize) {
    let _ = WATCH_SIZE.try_with(|watch| {
        if size != 0 && watch.get() == size {
            let _ = DENSE_ALLOCS.try_with(|hits| hits.set(hits.get() + 1));
        }
    });
}
// Observation is per thread and never changes allocation behavior.
unsafe impl GlobalAlloc for ObservedAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        observe(layout.size());
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        observe(layout.size());
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        observe(size);
        unsafe { System.realloc(ptr, layout, size) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}
#[global_allocator]
static ALLOCATOR: ObservedAllocator = ObservedAllocator;

fn bits(n: usize, mask: usize) -> BitVector {
    BitVector::from_indices(n, (0..n).filter(|j| mask >> j & 1 != 0)).unwrap()
}
// Independent integer products modulo omega^4+1, with no production transform.
fn mul(a: [i64; 4], b: [i64; 4]) -> [i64; 4] {
    let mut out = [0; 4];
    for (i, x) in a.into_iter().enumerate() {
        for (j, y) in b.into_iter().enumerate() {
            let k = i + j;
            out[k % 4] += x * y * if k >= 4 { -1 } else { 1 };
        }
    }
    out
}
fn numerators(n: usize) -> Vec<[i64; 4]> {
    (0..1 << n)
        .map(|mask| {
            (0..n).fold([1, 0, 0, 0], |c, j| {
                mul(c, [1, if mask >> j & 1 == 0 { 1 } else { -1 }, 0, 0])
            })
        })
        .collect()
}
fn positive_t(n: usize, a: &ScalarArithmetic) -> PhaseDecompositionInput {
    let terms = numerators(n)
        .into_iter()
        .enumerate()
        .map(|(mask, c)| CliffordIndicator {
            coefficient: a.scalar(c.map(Into::into), -(n as i64)).unwrap(),
            constraints: vec![],
            phase: ParityPhase::new(n, 0, [(bits(n, mask), 4)]).unwrap(),
        })
        .collect();
    PhaseDecompositionInput {
        dimension: n,
        phase_table: (0..1_usize << n)
            .map(|x| (x.count_ones() % 8) as u8)
            .collect(),
        terms,
    }
}
fn inverse_work(input: &PhaseDecompositionInput) -> u128 {
    input.phase_table.len() as u128 * (input.dimension + 2) as u128
        + 4 * input.terms.len() as u128
        + input
            .terms
            .iter()
            .map(|t| t.phase.terms().len() as u128)
            .sum::<u128>()
}
fn certify(input: PhaseDecompositionInput) -> PhaseCatalogue {
    let mut db = PhaseCatalogue::new();
    db.insert(
        input,
        &CatalogueLimits::default(),
        &ScalarArithmetic::default(),
    )
    .unwrap();
    db
}

#[test]
fn twelve_t_closed_product_certifies_all_coefficients_and_assignments_with_bounded_scratch() {
    let a = ScalarArithmetic::default();
    let input = positive_t(12, &a);
    let count = 4096;
    let coefficients = numerators(12);
    // Literal signed inverse sums validate every assignment independently of
    // both production transforms, using only bounded small integer numerators.
    for word in 0..count {
        let mut sum = [0_i64; 4];
        for (mask, c) in coefficients.iter().enumerate() {
            let sign = if (mask & word).count_ones() % 2 == 0 {
                1
            } else {
                -1
            };
            for j in 0..4 {
                sum[j] += sign * c[j];
            }
        }
        let p = (word.count_ones() % 8) as usize;
        let mut expected = [0; 4];
        expected[p % 4] = if p < 4 { count as i64 } else { -(count as i64) };
        assert_eq!(sum, expected, "assignment {word}");
    }
    let work = inverse_work(&input);
    assert_eq!(work, 77_823);
    assert!(work < 200_000);
    let mut db = PhaseCatalogue::new();
    let limits = CatalogueLimits {
        max_verification_work: work,
        ..Default::default()
    };
    let too_small = CatalogueLimits {
        max_verification_work: work - 1,
        ..limits
    };
    let refused = input.clone();
    WATCH_SIZE.with(|v| v.set(count * std::mem::size_of::<ExactScalar>()));
    DENSE_ALLOCS.with(|v| v.set(0));
    let result = db.insert(refused, &too_small, &a);
    WATCH_SIZE.with(|v| v.set(0));
    assert_eq!(
        DENSE_ALLOCS.with(Cell::get),
        0,
        "proof scratch allocated before admission"
    );
    assert_eq!(result.unwrap_err().field, "catalogue.max_verification_work");
    assert_eq!(db.usage().entries, 0);
    let admitted = input.clone();
    WATCH_SIZE.with(|v| v.set(count * std::mem::size_of::<ExactScalar>()));
    DENSE_ALLOCS.with(|v| v.set(0));
    let result = db.insert(admitted, &limits, &a);
    WATCH_SIZE.with(|v| v.set(0));
    result.unwrap();
    assert!(
        DENSE_ALLOCS.with(Cell::get) >= 1,
        "scratch observation control did not fire"
    );
    let entry = db.get(12, &input.phase_table).unwrap();
    assert_eq!(
        entry.verification().method,
        CertificateVerificationMethod::InverseWalsh
    );
    assert_eq!(entry.verification().work, work);
    assert_eq!(db.usage().verification_work, work);
    let forward =
        PhaseCatalogue::from_walsh_table(12, &input.phase_table, 49_152, &limits, &a).unwrap();
    let terms = forward.get(12, &input.phase_table).unwrap().terms();
    assert_eq!(terms.len(), count);
    for (mask, (actual, expected)) in terms.iter().zip(&input.terms).enumerate() {
        assert_eq!(
            actual.coefficient, expected.coefficient,
            "coefficient {mask}"
        );
        assert_eq!(actual.phase, expected.phase, "axis {mask}");
    }
    assert_eq!(forward.usage(), db.usage());
    assert_eq!(
        PhaseCatalogue::from_walsh_table(12, &input.phase_table, 49_152, &too_small, &a)
            .unwrap_err()
            .field,
        "catalogue.max_verification_work"
    );
}

#[test]
fn actual_coefficient_or_axis_corruption_and_invalid_dimensions_are_transactional() {
    let a = ScalarArithmetic::default();
    let input = positive_t(5, &a);
    let mut db = certify(positive_t(1, &a));
    let usage = db.usage();
    let original = db.get(1, &[0, 1]).unwrap() as *const _;
    let mut coefficient = input.clone();
    coefficient.terms[7].coefficient = a
        .add(&coefficient.terms[7].coefficient, &ExactScalar::one())
        .unwrap();
    let mut axis = input.clone();
    axis.terms[7].phase = ParityPhase::new(5, 0, [(bits(5, 6), 4)]).unwrap();
    let mut dimension = input.clone();
    dimension.terms[7].phase = ParityPhase::new(4, 0, []).unwrap();
    let mut constraint = input.clone();
    constraint.terms[7].constraints.push((bits(4, 0), false));
    let mut shape = input.clone();
    shape.phase_table.pop();
    let mut phase = input.clone();
    phase.phase_table[3] = 8;
    for bad in [coefficient, axis, dimension, constraint, shape, phase] {
        assert!(db.insert(bad, &CatalogueLimits::default(), &a).is_err());
        assert_eq!(db.usage(), usage);
        assert_eq!(db.entries().count(), 1);
        assert_eq!(db.get(1, &[0, 1]).unwrap() as *const _, original);
        assert!(db.get(5, &input.phase_table).is_none());
    }
}

#[test]
fn reordered_duplicate_axes_cancellation_zero_coefficients_and_global_phases_are_exact() {
    let a = ScalarArithmetic::default();
    let mut input = positive_t(5, &a);
    for (mask, term) in input.terms.iter_mut().enumerate() {
        term.coefficient = a
            .multiply(&term.coefficient, &a.omega(-3, 0).unwrap())
            .unwrap();
        term.phase = ParityPhase::new(
            5,
            3,
            [
                (bits(5, 2), 4),
                (bits(5, mask ^ 1), 4),
                (bits(5, 1), 4),
                (bits(5, 2), 4),
            ],
        )
        .unwrap();
    }
    for coefficient in [
        a.integer(7).unwrap(),
        a.integer(-7).unwrap(),
        ExactScalar::zero(),
    ] {
        input.terms.push(CliffordIndicator {
            coefficient,
            constraints: vec![],
            phase: ParityPhase::new(5, 5, [(bits(5, 23), 4)]).unwrap(),
        });
    }
    input.terms.reverse();
    let expected = input.clone();
    let db = certify(input);
    let entry = db.get(5, &expected.phase_table).unwrap();
    assert_eq!(
        entry.verification().method,
        CertificateVerificationMethod::InverseWalsh
    );
    assert_eq!(entry.verification().work, inverse_work(&expected));
    assert_eq!(entry.terms().len(), expected.terms.len());
    for (actual, expected) in entry.terms().iter().zip(&expected.terms) {
        assert_eq!(actual.coefficient, expected.coefficient);
        assert_eq!(actual.phase, expected.phase);
    }
}

#[test]
fn general_clifford_constraints_and_cheaper_constants_keep_pointwise_proofs() {
    let a = ScalarArithmetic::default();
    let phase = ParityPhase::new(4, 3, [(bits(4, 1), 2)]).unwrap();
    let input = PhaseDecompositionInput {
        dimension: 4,
        phase_table: (0..16).map(|x| 3 + 2 * (x & 1)).collect(),
        terms: vec![CliffordIndicator {
            coefficient: ExactScalar::one(),
            constraints: vec![],
            phase,
        }],
    };
    let db = certify(input.clone());
    let entry = db.get(4, &input.phase_table).unwrap();
    assert_eq!(
        entry.verification().method,
        CertificateVerificationMethod::Pointwise
    );
    assert_eq!(entry.verification().work, 48);
    let constrained = PhaseDecompositionInput {
        dimension: 4,
        phase_table: vec![0; 16],
        terms: [false, true]
            .map(|bit| CliffordIndicator {
                coefficient: ExactScalar::one(),
                constraints: vec![(bits(4, 1), bit)],
                phase: ParityPhase::new(4, 0, []).unwrap(),
            })
            .to_vec(),
    };
    let db = certify(constrained);
    assert_eq!(
        db.get(4, &[0; 16]).unwrap().verification().method,
        CertificateVerificationMethod::Pointwise
    );
    assert_eq!(db.usage().verification_work, 80);
    for n in [0, 1, 5, 12] {
        let db = PhaseCatalogue::from_walsh_table(
            n,
            &vec![3; 1 << n],
            u128::MAX,
            &CatalogueLimits::default(),
            &a,
        )
        .unwrap();
        let entry = db.entries().next().unwrap();
        assert_eq!(
            entry.verification().method,
            CertificateVerificationMethod::Pointwise
        );
        assert_eq!(entry.verification().work, 2 * (1 << n));
    }
}

#[test]
fn arithmetic_exhaustion_and_cumulative_work_reject_without_partial_insertion() {
    let a = ScalarArithmetic::default();
    let mut input = PhaseDecompositionInput {
        dimension: 4,
        phase_table: vec![0; 16],
        terms: vec![],
    };
    for value in std::iter::once(1)
        .chain(std::iter::repeat_n(1, 32))
        .chain(std::iter::repeat_n(-1, 32))
    {
        input.terms.push(CliffordIndicator {
            coefficient: a.integer(value).unwrap(),
            constraints: vec![],
            phase: ParityPhase::new(4, 0, []).unwrap(),
        });
    }
    let valid = certify(input.clone());
    assert_eq!(
        valid.entries().next().unwrap().verification().method,
        CertificateVerificationMethod::InverseWalsh
    );
    let mut db = PhaseCatalogue::new();
    let error = db
        .insert(
            input,
            &CatalogueLimits::default(),
            &ScalarArithmetic::new(1).unwrap(),
        )
        .unwrap_err();
    assert!(error.field.ends_with(".character"), "{error}");
    assert_eq!(db.usage().entries, 0);
    let input = positive_t(5, &a);
    let work = inverse_work(&input);
    let mut db = certify(positive_t(1, &a));
    let prior = db.usage();
    let limits = CatalogueLimits {
        max_verification_work: prior.verification_work + work - 1,
        ..Default::default()
    };
    assert_eq!(
        db.insert(input.clone(), &limits, &a).unwrap_err().field,
        "catalogue.max_verification_work"
    );
    assert_eq!(db.usage(), prior);
    db.insert(
        input,
        &CatalogueLimits {
            max_verification_work: limits.max_verification_work + 1,
            ..limits
        },
        &a,
    )
    .unwrap();
    assert_eq!(db.usage().verification_work, prior.verification_work + work);
    let mut bad = positive_t(5, &a);
    bad.terms[0].coefficient = ExactScalar::zero();
    bad.terms[0].phase = ParityPhase::new(4, 0, []).unwrap();
    assert!(
        PhaseCatalogue::new()
            .insert(bad, &CatalogueLimits::default(), &a)
            .is_err()
    );
}

#[test]
fn direct_source_readmission_uses_saved_method_and_cost_with_tighter_limits() {
    let a = ScalarArithmetic::default();
    let input = positive_t(5, &a);
    let db = certify(input.clone());
    let saved = db.get(5, &input.phase_table).unwrap().verification();
    let reference = ParityPhase::new(5, 0, (0..5).map(|j| (bits(5, 1 << j), 1))).unwrap();
    let mut options = PhaseSelectionOptions::default();
    options.construction.max_verification_work = saved.work;
    let plan = FrozenPhasePlan::new(&reference, Some(&db), &options, &a).unwrap();
    assert_eq!(
        plan.receipt().route,
        PhaseSelectionRoute::CertifiedCatalogue
    );
    assert_eq!(plan.certificate().verification(), saved);
    assert_eq!(plan.receipt().certification.verification_work, saved.work);
    options.construction.max_verification_work -= 1;
    assert_eq!(
        FrozenPhasePlan::new(&reference, Some(&db), &options, &a)
            .unwrap_err()
            .field,
        "catalogue.max_verification_work"
    );
    assert!(
        FrozenPhasePlan::new(
            &reference,
            Some(&db),
            &PhaseSelectionOptions::default(),
            &ScalarArithmetic::new(1).unwrap()
        )
        .is_err()
    );
    assert_eq!(db.get(5, &input.phase_table).unwrap().verification(), saved);
}

#[test]
fn json_and_character_product_admission_use_the_selected_proof_bound() {
    let a = ScalarArithmetic::default();
    let terms = std::iter::once(1).chain(std::iter::repeat_n(1,32)).chain(std::iter::repeat_n(-1,32))
        .map(|value|serde_json::json!({"coefficient":[value,0,0,0],"constraints":[],"clifford_parities":[]})).collect::<Vec<_>>();
    let document=serde_json::json!({"schema_version":1,"entries":[{"dimension":4,"core_phase_mod8":vec![0;16],"terms":terms}]}).to_string();
    let limits = CatalogueLimits {
        max_verification_work: 356,
        ..Default::default()
    };
    let db = PhaseCatalogue::from_json(&document, &limits, &a).unwrap();
    assert_eq!(db.usage().verification_work, 356);
    assert_eq!(
        db.entries().next().unwrap().verification().method,
        CertificateVerificationMethod::InverseWalsh
    );
    assert!(
        PhaseCatalogue::from_json(
            &document,
            &CatalogueLimits {
                max_verification_work: 355,
                ..limits
            },
            &a
        )
        .is_err()
    );
    let source =
        PhaseCatalogue::from_walsh_table(2, &[0, 1, 1, 2], 8, &CatalogueLimits::default(), &a)
            .unwrap();
    let product = source
        .paired_t_product(6, &CatalogueLimits::default(), &a)
        .unwrap()
        .unwrap();
    let work = product.usage().verification_work;
    assert_eq!(work, 912);
    assert_eq!(
        product.entries().next().unwrap().verification().method,
        CertificateVerificationMethod::InverseWalsh
    );
    assert_eq!(
        source
            .paired_t_product(
                6,
                &CatalogueLimits {
                    max_verification_work: work,
                    ..Default::default()
                },
                &a
            )
            .unwrap()
            .unwrap()
            .usage(),
        product.usage()
    );
    assert_eq!(
        source
            .paired_t_product(
                6,
                &CatalogueLimits {
                    max_verification_work: work - 1,
                    ..Default::default()
                },
                &a
            )
            .unwrap_err()
            .field,
        "catalogue.max_verification_work"
    );
}
