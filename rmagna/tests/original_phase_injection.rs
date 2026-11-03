// SPDX-License-Identifier: MPL-2.0
// Original finite scenarios adapted from itpyi/magna, commit
// 56847478c840395735146ef1e1ccdc3d8aedc10f, tests/native/phase_injection.cpp
// (SHA256 4ee4d9f17ae5d99bcc39a86a53ef84b8d65d20037806961c72a8e80f3b04d7d5).
// License: data/native-core-contracts/LICENSE. Literal amplitude oracle is independent.
#[path = "common/original_mt19937_32.rs"]
mod original_mt19937_32;
use original_mt19937_32::Mt32;
#[derive(Clone)]
struct InputTensor {
    n: usize,
    linear: Vec<u8>,
    upper: Vec<usize>,
    terms: Vec<(usize, u8)>,
    constant: u8,
    power: i64,
    boundary: Vec<(usize, u8)>,
}
struct Stream {
    inner: Mt32,
    calls: u64,
    hash: u64,
}
impl Stream {
    fn new() -> Self {
        Self {
            inner: Mt32::new(1729),
            calls: 0,
            hash: 14695981039346656037,
        }
    }
    fn next(&mut self) -> u32 {
        let v = self.inner.next();
        self.calls += 1;
        for b in v.to_le_bytes() {
            self.hash ^= u64::from(b);
            self.hash = self.hash.wrapping_mul(1099511628211);
        }
        v
    }
}
struct Observer {
    count: u64,
    hash: u64,
}
impl Observer {
    fn new() -> Self {
        Self {
            count: 0,
            hash: 14695981039346656037,
        }
    }
    fn word(&mut self, v: u64) {
        for b in v.to_le_bytes() {
            self.hash ^= u64::from(b);
            self.hash = self.hash.wrapping_mul(1099511628211);
        }
    }
    fn tensor(&mut self, t: &InputTensor) {
        self.count += 1;
        self.word(t.n as u64);
        self.word(t.linear.len() as u64);
        for v in &t.linear {
            self.word(u64::from(*v));
        }
        self.word(t.upper.len() as u64);
        for v in &t.upper {
            self.word(*v as u64);
        }
        self.word(t.terms.len() as u64);
        for (mask, c) in &t.terms {
            self.word(*mask as u64);
            self.word(u64::from(*c));
        }
        self.word(u64::from(t.constant));
        self.word(t.power as u64);
        self.word(t.boundary.len() as u64);
        for (mask, b) in &t.boundary {
            self.word(*mask as u64);
            self.word(u64::from(*b));
        }
        self.word(0);
    }
}
#[test]
fn original_injection_full_randomized_stream_keeps_exact_amplitudes_and_frozen_resources() {
    let mut rng = Stream::new();
    let mut observer = Observer::new();
    let mut base_count = 0;
    let mut update_count = 0;
    let a = ScalarArithmetic::default();
    let db = factors(&a);
    let mut selected = 0;
    let mut shared = 0;
    let mut amplitude_values = 0;
    for n in 2..=6 {
        for trial in 0..160 {
            let mut t = InputTensor {
                n,
                linear: vec![],
                upper: vec![0; n],
                terms: vec![],
                constant: 0,
                power: -3,
                boundary: vec![],
            };
            for j in 0..n {
                t.linear.push((rng.next() % 4) as u8);
                t.boundary.push((1 << j, (rng.next() % 2) as u8));
            }
            for mask in 1..1usize << n {
                let included = if n == 3 {
                    trial & (1usize << (mask - 1)) != 0
                } else {
                    rng.next().is_multiple_of(4)
                };
                if included {
                    t.terms.push((mask, (2 * (rng.next() % 4) + 1) as u8));
                }
            }
            t.constant = (rng.next() % 8) as u8;
            observer.tensor(&t);
            base_count += 1;
            let p = phase(&t);
            let plan = FrozenPhasePlan::new_with_injection(
                &p,
                Some(&db),
                &Default::default(),
                &Default::default(),
                &a,
            )
            .unwrap();
            selected += usize::from(plan.injection_receipt().unwrap().selected);
            check_certificate(&plan, &a);
            amplitude_values += check(&t, &plan, &a);
            let frozen = plan.certificate() as *const _;
            let axes = plan.signature_axes().as_ptr();
            let section = plan.section_lifts().as_ptr();
            let selection = *plan.receipt();
            let injection = plan.injection_receipt().cloned();
            let transport = plan
                .prepare_boundary(&p, &boundary(&t, &a), &Default::default(), &a)
                .unwrap();
            let receipt = transport.usage();
            let result = transport.expand(&p, &boundary(&t, &a), &a).unwrap();
            assert_eq!(result.statistics.map_pullbacks, receipt.shared_maps);
            assert_eq!(result.statistics.shifted_terms, receipt.consistent_terms);
            shared +=
                usize::from(result.statistics.map_pullbacks < result.statistics.shifted_terms);
            check_states(&t, &result.states, &a);

            if n > 4 {
                continue;
            }
            for assignment in 0..4 {
                let mut actual = t.clone();
                for (_, c) in &mut actual.terms {
                    if !rng.next().is_multiple_of(2) {
                        *c = (8 - *c) & 7;
                    }
                }
                for j in 0..n {
                    actual.linear[j] = (rng.next() % 4) as u8;
                    actual.boundary[j].1 = (rng.next() % 2) as u8;
                }
                actual.upper[0] = 1 << (n - 1);
                actual.constant = (rng.next() % 8) as u8;
                actual.power = -5;
                if assignment == 3 {
                    actual.boundary[0].0 = actual.boundary[1].0;
                }
                observer.tensor(&actual);
                update_count += 1;
                amplitude_values += check(&actual, &plan, &a);
                let updated_phase = phase(&actual);
                let updated_boundary = boundary(&actual, &a);
                // Changed columns need a new boundary plan, sharing the same
                // frozen selected resource. Other updates reuse the original.
                let changed_layout;
                let current = if assignment == 3 {
                    changed_layout = plan
                        .prepare_boundary(&p, &updated_boundary, &Default::default(), &a)
                        .unwrap();
                    &changed_layout
                } else {
                    &transport
                };
                let result = current
                    .expand(&updated_phase, &updated_boundary, &a)
                    .unwrap();
                assert_eq!(result.statistics.map_pullbacks, current.usage().shared_maps);
                assert_eq!(
                    result.statistics.shifted_terms,
                    current.usage().consistent_terms
                );
                check_states(&actual, &result.states, &a);
                assert_eq!(frozen, plan.certificate() as *const _);
                assert_eq!(axes, plan.signature_axes().as_ptr());
                assert_eq!(section, plan.section_lifts().as_ptr());
                assert_eq!(&selection, plan.receipt());
                assert_eq!(injection.as_ref(), plan.injection_receipt());
            }
        }
    }
    assert_eq!((base_count, update_count), (800, 1920));
    assert_eq!((rng.calls, rng.hash), (48306, 13662045568938363297));
    assert_eq!((observer.count, observer.hash), (2720, 6342693325637666586));
    assert!(selected > 0);
    assert!(shared > 0);
    assert_eq!(amplitude_values, 37760);
    println!(
        "Actual existing Rust injection/transport values PASS: selected={selected},shared={shared},exact_amplitudes={amplitude_values};800base+1920updates."
    );
    println!(
        "Exact original32bit input generation PASS:800base+1920updated tensors,48306draws/FNV13662045568938363297; structured2720tensorFNV6342693325637666586."
    );
}
use rmagna::catalogue::{
    FrozenPhasePlan, PhaseCatalogue, PhaseDecompositionInput, PhaseInjectionOptions,
    builtin_phase_catalogue,
};
use rmagna::gf2::BitVector;
use rmagna::phase::ParityPhase;
use rmagna::quadratic::QuadraticForm;
use rmagna::scalar::{ExactScalar, ScalarArithmetic};
use rmagna::stabilizer::AffineKet;
fn bits(n: usize, w: usize) -> BitVector {
    BitVector::from_indices(n, (0..n).filter(|j| w >> j & 1 != 0)).unwrap()
}
fn factors(a: &ScalarArithmetic) -> PhaseCatalogue {
    let mut db = PhaseCatalogue::new();
    for n in 1..=2 {
        let table = (0usize..1 << n)
            .map(|x| x.count_ones() as u8)
            .collect::<Vec<_>>();
        let entry = builtin_phase_catalogue().unwrap().get(n, &table).unwrap();
        assert_eq!(entry.terms().len(), 2);
        db.insert(
            PhaseDecompositionInput {
                dimension: n,
                phase_table: table,
                terms: entry.terms().to_vec(),
            },
            &Default::default(),
            a,
        )
        .unwrap();
    }
    db
}
fn phase(t: &InputTensor) -> ParityPhase {
    ParityPhase::new(
        t.n,
        t.constant,
        t.terms.iter().map(|(m, c)| (bits(t.n, *m), *c)),
    )
    .unwrap()
}
fn boundary(t: &InputTensor, a: &ScalarArithmetic) -> AffineKet {
    let columns = (0..t.n)
        .map(|j| {
            BitVector::from_indices(
                t.n,
                t.boundary
                    .iter()
                    .enumerate()
                    .filter_map(|(k, (m, _))| (m >> j & 1 != 0).then_some(k)),
            )
            .unwrap()
        })
        .collect();
    let offset = BitVector::from_indices(
        t.n,
        t.boundary
            .iter()
            .enumerate()
            .filter_map(|(j, (_, b))| (*b != 0).then_some(j)),
    )
    .unwrap();
    let q = QuadraticForm::new(
        0,
        t.linear.clone(),
        t.upper.iter().map(|m| bits(t.n, *m)).collect(),
    )
    .unwrap();
    AffineKet::new(offset, columns, q, a.omega(0, t.power).unwrap()).unwrap()
}
fn check(t: &InputTensor, plan: &FrozenPhasePlan, a: &ScalarArithmetic) -> usize {
    let p = phase(t);
    let b = boundary(t, a);
    let terms = plan.expand(&p, &b, a).unwrap();
    check_states(t, &terms, a)
}
fn check_states(t: &InputTensor, terms: &[AffineKet], a: &ScalarArithmetic) -> usize {
    let prepared = terms
        .iter()
        .map(|t| t.prepare().unwrap())
        .collect::<Vec<_>>();
    let mut expected = vec![ExactScalar::zero(); 1 << t.n];
    for u in 0..1usize << t.n {
        let out = t.boundary.iter().enumerate().fold(0, |v, (j, (m, b))| {
            v | ((((m & u).count_ones() as usize & 1) ^ usize::from(*b)) << j)
        });
        let mut exponent = i64::from(t.constant);
        for (j, c) in t.linear.iter().enumerate() {
            exponent += 2 * i64::from(*c) * ((u >> j & 1) as i64);
        }
        for (j, row) in t.upper.iter().enumerate() {
            for k in j + 1..t.n {
                if row >> k & 1 != 0 {
                    exponent += 4 * ((u >> j & u >> k & 1) as i64);
                }
            }
        }
        for (m, c) in &t.terms {
            exponent += i64::from(*c) * ((m & u).count_ones() as i64 & 1);
        }
        expected[out] = a
            .add(&expected[out], &a.omega(exponent, t.power).unwrap())
            .unwrap();
    }
    for (w, v) in expected.iter().enumerate() {
        let got = prepared.iter().fold(ExactScalar::zero(), |s, p| {
            a.add(&s, &p.amplitude(&bits(t.n, w), a).unwrap()).unwrap()
        });
        assert_eq!(&got, v, "n={},output={w}", t.n);
    }
    expected.len()
}
fn check_certificate(plan: &FrozenPhasePlan, a: &ScalarArithmetic) {
    let c = plan.certificate();
    let n = c.dimension();
    for (w, &p) in c.phase_table().iter().enumerate() {
        let x = bits(n, w);
        let mut actual = ExactScalar::zero();
        for term in c.terms() {
            if term
                .constraints
                .iter()
                .all(|(r, b)| r.dot(&x).unwrap() == *b)
            {
                actual = a
                    .add(
                        &actual,
                        &a.multiply(
                            &term.coefficient,
                            &a.omega(i64::from(term.phase.evaluate(&x).unwrap()), 0)
                                .unwrap(),
                        )
                        .unwrap(),
                    )
                    .unwrap();
            }
        }
        assert_eq!(actual, a.omega(i64::from(p), 0).unwrap());
    }
}

#[test]
fn mt19937_32_matches_independent_stdlib_checkpoints_through_one_million_draws() {
    let checkpoints = [
        (1, 911214221, 6847523963464566296),
        (2, 2673937510, 7805239501987538866),
        (3, 1112474867, 15278365635067378788),
        (31, 2198993963, 16753138331489547560),
        (624, 1052957729, 8487931025809541347),
        (625, 606125636, 4775355504810823541),
        (626, 1166107181, 5558049852327395640),
        (1000, 1644959750, 14415811842231927148),
        (10000, 3927176405, 2701223711261628074),
        (48306, 52882934, 13662045568938363297),
        (100000, 3070908535, 9528025473060844540),
        (1000000, 3566372652, 4142956547260922122),
    ];
    let mut rng = Mt32::new(1729);
    let mut hash = 14695981039346656037_u64;
    let mut next = 0;
    for draw in 1..=1_000_000 {
        let value = rng.next();
        for byte in value.to_le_bytes() {
            hash = (hash ^ u64::from(byte)).wrapping_mul(1099511628211);
        }
        if draw == checkpoints[next].0 {
            assert_eq!((value, hash), (checkpoints[next].1, checkpoints[next].2));
            next += 1;
            if next == checkpoints.len() {
                break;
            }
        }
    }
    assert_eq!(next, 12);
}
fn simple(n: usize, terms: Vec<(usize, u8)>) -> InputTensor {
    InputTensor {
        n,
        linear: vec![0; n],
        upper: vec![0; n],
        terms,
        constant: 0,
        power: 0,
        boundary: (0..n).map(|j| (1 << j, 0)).collect(),
    }
}
#[test]
fn original_injection_controls_keep_values_and_reference_scratch_precedes_tie() {
    let a = ScalarArithmetic::default();
    let db = factors(&a);
    let usage = db.usage();
    for t in [simple(0, vec![]), simple(1, vec![(1, 7)])] {
        let p = FrozenPhasePlan::new_with_injection(
            &phase(&t),
            Some(&db),
            &Default::default(),
            &PhaseInjectionOptions {
                max_work: 0,
                max_geometry_words: 0,
                max_terms: 0,
                max_reference_pairing_scratch_bytes: Some(0),
            },
            &a,
        )
        .unwrap();
        let r = p.injection_receipt().unwrap();
        assert_eq!(r.odd_rotations, 0);
        assert!(!r.constructed && !r.selected);
        assert_eq!(r.budget_skip, None);
        assert_eq!(r.reference_pairing_scratch_bytes, None);
        check_certificate(&p, &a);
        check(&t, &p, &a);
    }
    let t = simple(2, vec![(1, 1), (2, 1)]);
    let pair = FrozenPhasePlan::new_with_injection(
        &phase(&t),
        Some(&db),
        &Default::default(),
        &Default::default(),
        &a,
    )
    .unwrap();
    let r = pair.injection_receipt().unwrap();
    assert_eq!((r.original_terms, r.candidate_terms), (2, Some(2)));
    assert!(!r.constructed && !r.selected);
    assert_eq!(r.budget_skip, None);
    check_certificate(&pair, &a);
    check(&t, &pair, &a);
    let t = simple(3, (1..8).map(|m| (m, 1)).collect());
    for cap in [None, Some(20128), Some(u128::MAX)] {
        let p = FrozenPhasePlan::new_with_injection(
            &phase(&t),
            Some(&db),
            &Default::default(),
            &PhaseInjectionOptions {
                max_reference_pairing_scratch_bytes: cap,
                ..Default::default()
            },
            &a,
        )
        .unwrap();
        let r = p.injection_receipt().unwrap();
        assert_eq!(
            (
                r.original_terms,
                r.odd_rotations,
                r.constraint_rank,
                r.candidate_terms
            ),
            (8, 7, 1, Some(2))
        );
        assert!(r.constructed && r.selected);
        assert_eq!(r.reference_pairing_scratch_bytes, cap.map(|_| 20128));
        assert_eq!(p.num_terms(), 2);
        check_certificate(&p, &a);
        check(&t, &p, &a);
        // The canonical core's Clifford correction remains part of expansion.
        let states = p.expand(&phase(&t), &boundary(&t, &a), &a).unwrap();
        for x in 0..8 {
            let value = states.iter().fold(ExactScalar::zero(), |v, s| {
                a.add(
                    &v,
                    &s.prepare().unwrap().amplitude(&bits(3, x), &a).unwrap(),
                )
                .unwrap()
            });
            assert_eq!(value, a.omega(if x == 0 { 0 } else { 4 }, 0).unwrap());
        }
    }
    let t = simple(3, vec![(1, 1), (2, 1), (4, 1)]);
    for cap in [0, 4096, 11423, 11424, u128::MAX] {
        let p = FrozenPhasePlan::new_with_injection(
            &phase(&t),
            Some(&db),
            &Default::default(),
            &PhaseInjectionOptions {
                max_reference_pairing_scratch_bytes: Some(cap),
                ..Default::default()
            },
            &a,
        )
        .unwrap();
        let r = p.injection_receipt().unwrap();
        assert_eq!(r.reference_pairing_scratch_bytes, Some(11424));
        assert_eq!(r.original_terms, 4);
        assert!(!r.constructed && !r.selected);
        if cap < 11424 {
            assert_eq!(
                r.budget_skip.as_deref(),
                Some("phase_injection.max_reference_pairing_scratch_bytes")
            );
            assert_eq!(r.candidate_terms, None);
            assert!(r.groups.is_empty());
            assert_eq!(r.constraint_rank, 0);
        } else {
            assert_eq!(r.budget_skip, None);
            assert_eq!(r.candidate_terms, Some(4));
            assert!(!r.groups.is_empty());
        }
        assert_eq!(p.num_terms(), 4);
        check_certificate(&p, &a);
        check(&t, &p, &a);
    }
    // Existing independent logical-word admission is a distinct model.
    let p = FrozenPhasePlan::new_with_injection(
        &phase(&t),
        Some(&db),
        &Default::default(),
        &PhaseInjectionOptions {
            max_geometry_words: 512,
            ..Default::default()
        },
        &a,
    )
    .unwrap();
    let r = p.injection_receipt().unwrap();
    assert_eq!(r.budget_skip, None);
    assert_eq!(r.candidate_terms, Some(4));
    assert_eq!(r.reference_pairing_scratch_bytes, None);
    assert_eq!(db.usage(), usage);
    check(&t, &p, &a);
}

#[test]
fn raw_tensor_zero_exits_before_selection_and_nonzero_scalar_errors_preserve_reuse() {
    use rmagna::phase::{
        NativePhaseOptions, PathTensorError, PhaseError, PhasePathTensor, PhaseResourceOptions,
    };
    let a = ScalarArithmetic::default();
    let db = factors(&a);
    let usage = db.usage();
    let t = simple(3, vec![(1, 1), (2, 1), (4, 1)]);
    let p = phase(&t);
    let b = boundary(&t, &a);
    let tensor = PhasePathTensor::new(b.clone(), p.clone()).unwrap();
    let zero = PhasePathTensor::new(
        AffineKet::new(
            b.offset().clone(),
            b.columns().to_vec(),
            b.phase().clone(),
            ExactScalar::zero(),
        )
        .unwrap(),
        p.clone(),
    )
    .unwrap();
    let good = NativePhaseOptions {
        injection: Some(Default::default()),
        ..Default::default()
    };
    let mut refused = good;
    refused.selection.max_quotient_rank = 0;
    refused.selection.max_core_table_bytes = 0;
    assert!(
        zero.prepare_native_resource(0, refused, Some(&db), &a)
            .unwrap()
            .is_none()
    );
    assert!(
        zero.prepare_resource(0, PhaseResourceOptions::default(), Some(&db), &a)
            .unwrap()
            .is_none()
    );
    assert!(
        tensor
            .prepare_native_resource(0, refused, Some(&db), &a)
            .is_err()
    );
    assert!(
        tensor
            .prepare_resource(0, PhaseResourceOptions::default(), None, &a)
            .is_err()
    );
    let resource = tensor
        .prepare_native_resource(8, good, Some(&db), &a)
        .unwrap()
        .unwrap();
    let before = resource
        .native_resource()
        .unwrap()
        .injection_receipt()
        .cloned();
    let original = resource.expand(&p, &b, &a).unwrap();
    let wide = a
        .scalar([257.into(), 0.into(), 0.into(), 0.into()], 0)
        .unwrap();
    let oversized = PhasePathTensor::new(
        AffineKet::new(
            b.offset().clone(),
            b.columns().to_vec(),
            b.phase().clone(),
            wide,
        )
        .unwrap(),
        p.clone(),
    )
    .unwrap();
    let small = ScalarArithmetic::new(8).unwrap();
    assert!(matches!(
        oversized.prepare_native_resource(0, refused, Some(&db), &small),
        Err(PathTensorError::Algebra(PhaseError::Scalar(_)))
    ));
    assert!(matches!(
        oversized.prepare_resource(0, PhaseResourceOptions::default(), None, &small),
        Err(PathTensorError::Algebra(PhaseError::Scalar(_)))
    ));
    assert_eq!(
        resource.native_resource().unwrap().injection_receipt(),
        before.as_ref()
    );
    assert_eq!(resource.expand(&p, &b, &a).unwrap(), original);
    assert_eq!(tensor.boundary(), &b);
    assert_eq!(tensor.phase(), &p);
    assert_eq!(db.usage(), usage);
    assert!(
        oversized
            .prepare_native_resource(8, good, Some(&db), &a)
            .unwrap()
            .is_some()
    );
    let independent = tensor
        .prepare_resource(8, PhaseResourceOptions::default(), None, &a)
        .unwrap()
        .unwrap();
    check_states(&t, &independent.expand(&p, &b, &a).unwrap(), &a);
}
