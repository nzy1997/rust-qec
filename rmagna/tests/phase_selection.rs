use rmagna::catalogue::{
    CatalogueLimits, CliffordIndicator, FrozenPhasePlan, PhaseCatalogue, PhaseDecompositionInput,
    PhaseSelectionOptions, PhaseSelectionRoute,
};
use rmagna::gf2::BitVector;
use rmagna::phase::{ParityPhase, PhaseError};
use rmagna::quadratic::QuadraticForm;
use rmagna::scalar::{ExactScalar, ScalarArithmetic};
use rmagna::stabilizer::AffineKet;
use serde_json::Value;
use sha2::{Digest, Sha256};

fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/phase_selection_native.json")).unwrap()
}
fn bv(width: usize, word: u64) -> BitVector {
    BitVector::from_indices(width, (0..width).filter(|j| word >> j & 1 != 0)).unwrap()
}
fn word(row: &BitVector) -> u64 {
    row.ones().fold(0, |word, j| word | 1 << j)
}
fn parity(word: u64) -> u8 {
    (word.count_ones() & 1) as u8
}
fn phase(row: &Value) -> ParityPhase {
    let n = row["n"].as_u64().unwrap() as usize;
    ParityPhase::new(
        n,
        row["constant"].as_u64().unwrap() as u8,
        row["terms"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| (bv(n, t[0].as_u64().unwrap()), t[1].as_u64().unwrap() as u8)),
    )
    .unwrap()
}
fn computational(dimension: usize, table: Vec<u8>, cancel: bool) -> PhaseDecompositionInput {
    let arithmetic = ScalarArithmetic::default();
    let mut terms: Vec<_> = table
        .iter()
        .enumerate()
        .map(|(x, &p)| CliffordIndicator {
            coefficient: arithmetic.omega(i64::from(p), 0).unwrap(),
            constraints: (0..dimension)
                .map(|j| (bv(dimension, 1 << j), x >> j & 1 != 0))
                .collect(),
            phase: ParityPhase::new(dimension, 0, []).unwrap(),
        })
        .collect();
    if cancel {
        for coefficient in [ExactScalar::one(), ExactScalar::one().negated()] {
            terms.push(CliffordIndicator {
                coefficient,
                constraints: vec![],
                phase: ParityPhase::new(dimension, 0, []).unwrap(),
            });
        }
    }
    PhaseDecompositionInput {
        dimension,
        phase_table: table,
        terms,
    }
}
fn source(row: &Value) -> Option<PhaseCatalogue> {
    let mode = row["database"].as_str().unwrap();
    if mode == "absent" {
        return None;
    }
    let mut inputs = Vec::new();
    if ["one", "both", "direct"].contains(&mode) {
        inputs.push(computational(1, vec![0, 1], false));
    }
    if ["two", "both", "direct"].contains(&mode) {
        inputs.push(computational(2, vec![0, 1, 1, 2], false));
    }
    if mode == "direct" {
        let rank = row["axes"].as_array().unwrap().len();
        let table: Vec<_> = row["table"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p.as_u64().unwrap() as u8)
            .collect();
        inputs.retain(|input| input.dimension != rank || input.phase_table != table);
        inputs.push(computational(rank, table, true));
    }
    let mut catalogue = PhaseCatalogue::new();
    for input in inputs {
        catalogue
            .insert(
                input,
                &CatalogueLimits::default(),
                &ScalarArithmetic::default(),
            )
            .unwrap();
    }
    Some(catalogue)
}
fn expected_scalar(raw: &Value, arithmetic: &ScalarArithmetic) -> ExactScalar {
    arithmetic
        .scalar(
            std::array::from_fn(|j| raw[0][j].as_i64().unwrap().into()),
            raw[1].as_i64().unwrap(),
        )
        .unwrap()
}
fn assert_native(row: &Value, plan: &FrozenPhasePlan) {
    assert_eq!(plan.receipt().route.name(), row["route"].as_str().unwrap());
    assert_eq!(
        plan.signature_axes().iter().map(word).collect::<Vec<_>>(),
        row["axes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        plan.section_lifts().iter().map(word).collect::<Vec<_>>(),
        row["lifts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        plan.certificate().phase_table(),
        row["table"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p.as_u64().unwrap() as u8)
            .collect::<Vec<_>>()
    );
    let expected = row["exact_entries"].as_array().unwrap();
    assert_eq!(plan.num_terms(), expected.len());
    assert_eq!(plan.certificate().terms().len(), expected.len());
    let arithmetic = ScalarArithmetic::default();
    for (term, raw) in plan.certificate().terms().iter().zip(expected) {
        assert_eq!(term.coefficient, expected_scalar(&raw[0], &arithmetic));
        assert_eq!(
            term.constraints,
            raw[1]
                .as_array()
                .unwrap()
                .iter()
                .map(|pair| (
                    bv(plan.quotient_rank(), pair[0].as_u64().unwrap()),
                    pair[1].as_u64().unwrap() != 0
                ))
                .collect::<Vec<_>>()
        );
        let expected_phase = ParityPhase::new(
            plan.quotient_rank(),
            0,
            raw[2].as_array().unwrap().iter().map(|pair| {
                (
                    bv(plan.quotient_rank(), pair[0].as_u64().unwrap()),
                    2 * pair[1].as_u64().unwrap() as u8,
                )
            }),
        )
        .unwrap();
        assert_eq!(term.phase, expected_phase);
    }
}
fn boundary(n: usize, update: u8) -> QuadraticForm {
    QuadraticForm::new(
        0,
        (0..n).map(|j| (j as u8 + update) % 4).collect(),
        (0..n)
            .map(|j| BitVector::from_indices(n, (j + 1..n).filter(|k| (j + k) % 2 != 0)).unwrap())
            .collect(),
    )
    .unwrap()
}
fn updated(row: &Value, update: u8) -> ParityPhase {
    let base = phase(row);
    let mut terms = base.terms().to_vec();
    terms.push((
        BitVector::from_indices(base.num_variables(), 0..base.num_variables()).unwrap(),
        2 * update,
    ));
    ParityPhase::new(base.num_variables(), (base.constant() + update) & 7, terms).unwrap()
}
fn labelled(n: usize, q: QuadraticForm, coefficient: ExactScalar) -> AffineKet {
    AffineKet::new(
        bv(3, 1),
        (0..n).map(|j| bv(3, [1, 3, 1, 0][j % 4])).collect(),
        q,
        coefficient,
    )
    .unwrap()
}
fn literal(
    row: &Value,
    update: u8,
    arithmetic: &ScalarArithmetic,
) -> (ExactScalar, Vec<ExactScalar>) {
    let n = row["n"].as_u64().unwrap() as usize;
    let prefactor = arithmetic
        .scalar([1.into(), 1.into(), 1.into(), (-1).into()], -2)
        .unwrap();
    let mut scalar = ExactScalar::zero();
    let mut amplitudes = vec![ExactScalar::zero(); 8];
    for x in 0..1_u64 << n {
        let original = row["constant"].as_u64().unwrap()
            + row["terms"]
                .as_array()
                .unwrap()
                .iter()
                .map(|term| {
                    term[1].as_u64().unwrap() * u64::from(parity(term[0].as_u64().unwrap() & x))
                })
                .sum::<u64>();
        let mut q = 0_u64;
        let mut label = 1;
        for j in 0..n {
            if x >> j & 1 == 0 {
                continue;
            }
            q += (j as u64 + u64::from(update)) % 4;
            label ^= [1, 3, 1, 0][j % 4];
            for k in j + 1..n {
                if x >> k & 1 != 0 {
                    q += 2 * ((j + k) % 2) as u64;
                }
            }
        }
        let exponent =
            original + u64::from(update) + 2 * u64::from(update) * u64::from(parity(x)) + 2 * q;
        let value = arithmetic
            .multiply(&prefactor, &arithmetic.omega(exponent as i64, 0).unwrap())
            .unwrap();
        scalar = arithmetic.add(&scalar, &value).unwrap();
        amplitudes[label] = arithmetic.add(&amplitudes[label], &value).unwrap();
    }
    (scalar, amplitudes)
}
#[test]
fn native_routes_terms_and_literal_transports_match_with_injection_ineligible() {
    let fixture = fixture();
    assert_eq!(
        fixture["upstream_commit"],
        "56847478c840395735146ef1e1ccdc3d8aedc10f"
    );
    assert_eq!(
        fixture["generator_sha256"],
        format!(
            "{:x}",
            Sha256::digest(include_bytes!(
                "../../benchmarks/magna_reproduction/capture_phase_selection.py"
            ))
        )
    );
    assert_eq!(fixture["cases"].as_array().unwrap().len(), 60);
    let arithmetic = ScalarArithmetic::default();
    let prefactor = arithmetic
        .scalar([1.into(), 1.into(), 1.into(), (-1).into()], -2)
        .unwrap();
    let mut routes = [0; 3];
    for row in fixture["cases"].as_array().unwrap() {
        let original = phase(row);
        let source = source(row);
        let usage = source.as_ref().map(PhaseCatalogue::usage);
        let plan = FrozenPhasePlan::new(
            &original,
            source.as_ref(),
            &PhaseSelectionOptions::default(),
            &arithmetic,
        )
        .unwrap();
        assert_eq!(source.as_ref().map(PhaseCatalogue::usage), usage);
        assert_native(row, &plan);
        routes[match plan.receipt().route {
            PhaseSelectionRoute::CertifiedCatalogue => 0,
            PhaseSelectionRoute::PositiveTProduct => 1,
            PhaseSelectionRoute::Walsh => 2,
        }] += 1;
        drop(source);
        drop(original);
        for update in 0..4 {
            let n = plan.num_variables();
            let phase = updated(row, update);
            let q = boundary(n, update);
            let (expected, amplitudes) = literal(row, update, &arithmetic);
            assert_eq!(
                plan.evaluate(&phase, &q, &prefactor, &arithmetic).unwrap(),
                expected,
                "{} update {update}",
                row["name"]
            );
            let terms = plan
                .expand(&phase, &labelled(n, q, prefactor.clone()), &arithmetic)
                .unwrap();
            for (label, expected) in amplitudes.iter().enumerate() {
                let mut actual = ExactScalar::zero();
                for term in &terms {
                    actual = arithmetic
                        .add(
                            &actual,
                            &term
                                .prepare()
                                .unwrap()
                                .amplitude(&bv(3, label as u64), &arithmetic)
                                .unwrap(),
                        )
                        .unwrap();
                }
                assert_eq!(
                    &actual, expected,
                    "{} update {update} label {label}",
                    row["name"]
                );
            }
        }
        if plan.num_variables() > 0 {
            let mut changed = phase(row).terms().to_vec();
            changed.push((bv(plan.num_variables(), 1), 1));
            let changed = ParityPhase::new(plan.num_variables(), 0, changed).unwrap();
            let q = boundary(plan.num_variables(), 0);
            assert!(matches!(
                plan.evaluate(&changed, &q, &ExactScalar::zero(), &arithmetic),
                Err(PhaseError::NotClifford)
            ));
            assert!(matches!(
                plan.expand(
                    &changed,
                    &labelled(plan.num_variables(), q, ExactScalar::zero()),
                    &arithmetic
                ),
                Err(PhaseError::NotClifford)
            ));
        }
    }
    assert_eq!(routes, [14, 10, 36]);
}

#[test]
fn selected_route_limits_are_exact_and_failures_never_reselect() {
    let fixture = fixture();
    let arithmetic = ScalarArithmetic::default();
    for name in ["independent-3-direct", "independent-3-both", "ccz-both"] {
        let row = fixture["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["name"] == name)
            .unwrap();
        let reference = phase(row);
        let source = source(row);
        let original_usage = source.as_ref().unwrap().usage();
        let defaults = PhaseSelectionOptions::default();
        let baseline =
            FrozenPhasePlan::new(&reference, source.as_ref(), &defaults, &arithmetic).unwrap();
        let r = *baseline.receipt();
        let mut exact = defaults;
        exact.max_quotient_rank = r.quotient_rank;
        exact.max_core_table_bytes = r.logical_core_table_bytes;
        exact.max_signature_work = r.signature_work_bound;
        exact.max_transform_work = r.transform_work;
        exact.preparation.core.max_dimension = r.quotient_rank;
        exact.preparation.core.max_table_values = r.table_values;
        exact.preparation.core.max_work = r.core_construction_work;
        exact.preparation.max_terms = r.retained_terms;
        exact.preparation.max_geometry_words = r.geometry_words;
        exact.construction.max_entries = 1;
        exact.construction.max_dimension = r.quotient_rank;
        exact.construction.max_table_values = r.table_values;
        exact.construction.max_terms = r.retained_terms;
        exact.construction.max_constraint_rows = r.certification.constraint_rows;
        exact.construction.max_parity_rows = r.certification.parity_rows;
        exact.construction.max_verification_work = r.certification.verification_work;
        assert_native(
            row,
            &FrozenPhasePlan::new(&reference, source.as_ref(), &exact, &arithmetic).unwrap(),
        );
        let mut failures = Vec::new();
        macro_rules! lower {
            ($field:ident,$value:expr,$error:literal) => {{
                let mut option = exact;
                option.$field = $value;
                failures.push((option, $error));
            }};
        }
        lower!(max_quotient_rank, r.quotient_rank - 1, "max_quotient_rank");
        lower!(
            max_core_table_bytes,
            r.logical_core_table_bytes - 1,
            "max_core_table_bytes"
        );
        lower!(
            max_signature_work,
            r.signature_work_bound - 1,
            "max_signature_work"
        );
        let mut option = exact;
        option.preparation.core.max_work -= 1;
        failures.push((option, "phase_core.max_work"));
        let mut option = exact;
        option.preparation.max_terms -= 1;
        failures.push((option, "max_terms"));
        let mut option = exact;
        option.preparation.max_geometry_words -= 1;
        failures.push((option, "max_geometry_words"));
        let mut option = exact;
        option.construction.max_entries = 0;
        failures.push((option, "max_entries"));
        let mut option = exact;
        option.construction.max_terms -= 1;
        failures.push((option, "max_terms"));
        let mut option = exact;
        option.construction.max_verification_work -= 1;
        failures.push((option, "max_verification_work"));
        if r.certification.constraint_rows > 0 {
            let mut option = exact;
            option.construction.max_constraint_rows -= 1;
            failures.push((option, "max_constraint_rows"));
        }
        if r.certification.parity_rows > 0 {
            let mut option = exact;
            option.construction.max_parity_rows -= 1;
            failures.push((option, "max_parity_rows"));
        }
        if r.transform_work > 0 {
            lower!(
                max_transform_work,
                r.transform_work - 1,
                "max_transform_work"
            );
        }
        for (options, field) in failures {
            let error = FrozenPhasePlan::new(&reference, source.as_ref(), &options, &arithmetic)
                .unwrap_err();
            assert!(error.field.contains(field), "{name}: {error}");
            assert!(!error.message.is_empty());
        }
        assert_eq!(source.as_ref().unwrap().usage(), original_usage);
    }
    let rows = fixture["cases"].as_array().unwrap();
    let row = rows
        .iter()
        .find(|row| row["name"] == "independent-3-direct")
        .unwrap();
    let mut options = PhaseSelectionOptions::default();
    options.construction.max_terms = 8;
    let error =
        FrozenPhasePlan::new(&phase(row), source(row).as_ref(), &options, &arithmetic).unwrap_err();
    assert!(error.message.contains("selected catalogue route"));
    let row = rows
        .iter()
        .find(|row| row["name"] == "independent-3-both")
        .unwrap();
    options = PhaseSelectionOptions::default();
    options.construction.max_constraint_rows = 0;
    let error =
        FrozenPhasePlan::new(&phase(row), source(row).as_ref(), &options, &arithmetic).unwrap_err();
    assert!(error.message.contains("selected positive_t_product route"));
    let row = rows
        .iter()
        .find(|row| row["name"] == "independent-3-two")
        .unwrap();
    let plan =
        FrozenPhasePlan::new(&phase(row), source(row).as_ref(), &options, &arithmetic).unwrap();
    assert_eq!(plan.receipt().route, PhaseSelectionRoute::Walsh);
}

#[test]
fn wide_rank_tiny_table_and_malformed_certificates_reject_before_expansion() {
    let arithmetic = ScalarArithmetic::default();
    let n = usize::BITS as usize;
    let phase = ParityPhase::new(
        n,
        0,
        (0..n).map(|j| (BitVector::from_indices(n, [j]).unwrap(), 1)),
    )
    .unwrap();
    let options = PhaseSelectionOptions {
        max_quotient_rank: n,
        max_core_table_bytes: 1,
        ..PhaseSelectionOptions::default()
    };
    let error = FrozenPhasePlan::new(&phase, None, &options, &arithmetic).unwrap_err();
    assert_eq!(error.field, "phase_selection.max_core_table_bytes");
    assert!(
        error
            .message
            .contains(&(4_u128 * (1_u128 << n)).to_string())
    );
    let mut catalogue = PhaseCatalogue::new();
    let invalid = PhaseDecompositionInput {
        dimension: 1,
        phase_table: vec![0, 1],
        terms: vec![CliffordIndicator {
            coefficient: ExactScalar::one(),
            constraints: vec![],
            phase: ParityPhase::new(1, 0, []).unwrap(),
        }],
    };
    assert!(
        catalogue
            .insert(invalid, &CatalogueLimits::default(), &arithmetic)
            .is_err()
    );
    assert_eq!(catalogue.usage().entries, 0);
    assert!(catalogue.get(1, &[0, 1]).is_none());
    let phase = ParityPhase::new(5, 7, [(bv(5, 9), 2)]).unwrap();
    let plan = FrozenPhasePlan::new(
        &phase,
        Some(&catalogue),
        &PhaseSelectionOptions::default(),
        &arithmetic,
    )
    .unwrap();
    assert_eq!(plan.quotient_rank(), 0);
    assert_eq!(plan.num_terms(), 1);
    assert_eq!(plan.receipt().route, PhaseSelectionRoute::PositiveTProduct);
    assert_eq!(plan.section_lifts(), &[]);
    let mut sum = ExactScalar::zero();
    for x in 0..32 {
        sum = arithmetic
            .add(
                &sum,
                &arithmetic
                    .omega(7 + 2 * i64::from(parity(9 & x)), 0)
                    .unwrap(),
            )
            .unwrap();
    }
    assert_eq!(
        plan.evaluate(
            &phase,
            &QuadraticForm::new(0, vec![0; 5], vec![bv(5, 0); 5]).unwrap(),
            &ExactScalar::one(),
            &arithmetic
        )
        .unwrap(),
        sum
    );
}

#[test]
fn native_coordinates_keep_original_rows_and_zero_free_sections() {
    // Native calibration: the second original row 3 reduces to 1 against row 2.
    // Keeping that reduced row changes the complete-table key and its section.
    let arithmetic = ScalarArithmetic::default();
    let coefficients = [[2, 1, 0, 1], [2, -1, 0, -1], [0, 1, 0, -1], [0, -1, 0, 1]];
    for mode in ["absent", "empty", "one", "two", "both", "direct"] {
        let row = serde_json::json!({"name":"retained-original","n":4,"constant":5,
            "terms":[[1,1],[3,1],[9,2]],"database":mode,"axes":[2,3],"table":[0,1,0,3]});
        let reference = phase(&row);
        assert_eq!(
            reference
                .quotient()
                .unwrap()
                .complement()
                .iter()
                .map(word)
                .collect::<Vec<_>>(),
            vec![1, 2]
        );
        let source = source(&row);
        let plan = FrozenPhasePlan::new(
            &reference,
            source.as_ref(),
            &PhaseSelectionOptions::default(),
            &arithmetic,
        )
        .unwrap();
        assert_eq!(
            plan.signature_axes().iter().map(word).collect::<Vec<_>>(),
            vec![2, 3]
        );
        assert_eq!(
            plan.section_lifts().iter().map(word).collect::<Vec<_>>(),
            vec![3, 1]
        );
        assert_eq!(plan.certificate().phase_table(), &[0, 1, 0, 3]);
        if mode == "direct" {
            assert_eq!(
                plan.receipt().route,
                PhaseSelectionRoute::CertifiedCatalogue
            );
            assert_eq!(plan.num_terms(), 6);
        } else {
            assert_eq!(plan.receipt().route, PhaseSelectionRoute::Walsh);
            assert_eq!(plan.num_terms(), 4);
            for (term, coefficients) in plan.certificate().terms().iter().zip(coefficients) {
                assert_eq!(
                    term.coefficient,
                    arithmetic.scalar(coefficients.map(Into::into), -2).unwrap()
                );
            }
        }
        drop(source);
        for update in 0..4 {
            let updated = updated(&row, update);
            let q = boundary(4, update);
            let prefactor = arithmetic
                .scalar([1.into(), 1.into(), 1.into(), (-1).into()], -2)
                .unwrap();
            let (scalar, amplitudes) = literal(&row, update, &arithmetic);
            assert_eq!(
                plan.evaluate(&updated, &q, &prefactor, &arithmetic)
                    .unwrap(),
                scalar
            );
            let terms = plan
                .expand(&updated, &labelled(4, q, prefactor), &arithmetic)
                .unwrap();
            for (label, expected) in amplitudes.iter().enumerate() {
                let mut sum = ExactScalar::zero();
                for term in &terms {
                    sum = arithmetic
                        .add(
                            &sum,
                            &term
                                .prepare()
                                .unwrap()
                                .amplitude(&bv(3, label as u64), &arithmetic)
                                .unwrap(),
                        )
                        .unwrap();
                }
                assert_eq!(&sum, expected);
            }
        }
    }
    let reference = ParityPhase::new(4, 5, [(bv(4, 1), 1), (bv(4, 3), 1), (bv(4, 9), 2)]).unwrap();
    let error = FrozenPhasePlan::new(
        &reference,
        None,
        &PhaseSelectionOptions::default(),
        &ScalarArithmetic::new(1).unwrap(),
    )
    .unwrap_err();
    assert!(error.message.contains("selected walsh route"));
    assert!(error.message.contains("coefficient"));
}
