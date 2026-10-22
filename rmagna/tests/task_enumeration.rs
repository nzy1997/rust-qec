use num_rational::BigRational;
use num_traits::{One, Zero};
use rmagna::circuit::{ParseOptions, parse_circuit};
use rmagna::field::{ExactField, FieldArithmetic};
use rmagna::noise::NoiseSampling;
use rmagna::runtime::{CausalStrategy, ControllerRegistry};
use rmagna::task::{
    EvaluationOptions, TaskEnumerationOptions, TaskEvaluator, TaskOptions, parse_task,
};
use std::path::PathBuf;

struct Files(PathBuf);
impl Files {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let p = std::env::temp_dir().join(format!(
            "rmagna-sector-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir(&p).unwrap();
        Self(p)
    }
    fn put(&self, name: &str, text: &str) {
        std::fs::write(self.0.join(name), text).unwrap();
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn evaluator(
    files: &Files,
    physical: &str,
    task: &str,
    registry: &ControllerRegistry,
    options: EvaluationOptions,
    sampling: &NoiseSampling,
) -> TaskEvaluator {
    let parse = ParseOptions {
        controllers: registry.names(),
        ..Default::default()
    };
    let physical = parse_circuit(physical, &parse).unwrap();
    let task = parse_task(task, &files.0, &TaskOptions::default()).unwrap();
    let modules = task.load_modules(&physical, &parse).unwrap();
    TaskEvaluator::new_with_sampling(&physical, &modules, registry, options, sampling).unwrap()
}
fn r(n: i64, d: i64) -> BigRational {
    BigRational::new(n.into(), d.into())
}
fn field(x: BigRational) -> ExactField {
    FieldArithmetic::default().from_rational(x).unwrap()
}
fn opts(weight: usize) -> TaskEnumerationOptions {
    let mut o = TaskEnumerationOptions::default();
    o.noise.max_weight = weight;
    o
}
fn less_equal(x: &ExactField, y: &ExactField) -> bool {
    FieldArithmetic::default()
        .subtract(y, x)
        .unwrap()
        .real_sign()
        .unwrap()
        != std::cmp::Ordering::Less
}

#[test]
fn every_linear_sector_matches_independent_categorical_boolean_and_signed_moment_sums() {
    let files = Files::new();
    let registry = ControllerRegistry::default();
    files.put("zero.magna", "I 0 1");
    let e = evaluator(
        &files,
        "X_ERROR(1/4) 0\nX_ERROR(1/5) 1\nM(1/7) 0\nM 1\nDETECTOR rec[-2]\nPOSTSELECT det[0]\nOBSERVABLE_INCLUDE(0) rec[-2]\nOBSERVABLE_INCLUDE(1) rec[-1]",
        r#"{"outputs":[0,1],"ideal_state":"zero.magna","observables":[[0,1],[0,2],[0,3]]}"#,
        &registry,
        EvaluationOptions::default(),
        &NoiseSampling::Physical,
    );
    let mut oracle = vec![
        [
            BigRational::zero(),
            BigRational::zero(),
            BigRational::zero(),
            BigRational::zero(),
            BigRational::zero(),
            BigRational::zero()
        ];
        4
    ];
    let mut counts = [0_usize; 4];
    for mask in 0_usize..8 {
        let x0 = mask & 1 != 0;
        let x1 = mask & 2 != 0;
        let flip = mask & 4 != 0;
        let p = if x0 { r(1, 4) } else { r(3, 4) }
            * if x1 { r(1, 5) } else { r(4, 5) }
            * if flip { r(1, 7) } else { r(6, 7) };
        let row = &mut oracle[mask.count_ones() as usize];
        counts[mask.count_ones() as usize] += 1;
        row[0] += &p;
        if !(x0 ^ flip) {
            row[1] += &p;
            if x0 || x1 {
                row[2] += &p;
            }
            for (j, negative) in [x0, x1, x0 ^ x1].into_iter().enumerate() {
                row[3 + j] += if negative { -&p } else { p.clone() };
            }
        }
    }
    let full_a: BigRational = oracle.iter().map(|row| row[1].clone()).sum();
    let full_e: BigRational = oracle.iter().map(|row| row[2].clone()).sum();
    for cut in 0..=4 {
        let result = e.enumerate_physical(&opts(cut)).unwrap();
        assert_eq!(result.by_weight.len(), cut.min(4) + 1);
        let mut totals = std::array::from_fn::<_, 6, _>(|_| BigRational::zero());
        for row in &result.by_weight {
            let want = oracle
                .get(row.weight)
                .cloned()
                .unwrap_or_else(|| std::array::from_fn(|_| BigRational::zero()));
            assert_eq!(row.probability.value(), &want[0]);
            assert_eq!(row.patterns, counts.get(row.weight).copied().unwrap_or(0));
            assert_eq!(row.histories, row.patterns);
            assert_eq!(row.masses.accepted, field(want[1].clone()));
            assert_eq!(row.masses.error, field(want[2].clone()));
            assert_eq!(
                row.masses.moments,
                want[3..].iter().cloned().map(field).collect::<Vec<_>>()
            );
            for (x, y) in totals.iter_mut().zip(want) {
                *x += y;
            }
        }
        assert_eq!(result.covered_probability.value(), &totals[0]);
        assert_eq!(
            result.omitted_probability.value(),
            &(BigRational::one() - &totals[0])
        );
        assert_eq!(result.masses.accepted, field(totals[1].clone()));
        assert_eq!(result.masses.error, field(totals[2].clone()));
        assert_eq!(
            result.masses.moments,
            totals[3..].iter().cloned().map(field).collect::<Vec<_>>()
        );
        assert_eq!(
            result.masses.target_success,
            Some(field(&totals[1] - &totals[2]))
        );
        assert_eq!(result.histories, result.patterns);
        assert_eq!(
            (
                result.marginal_queries,
                result.replay_attempts,
                result.network_compilations
            ),
            (0, 0, 0)
        );
        for (interval, exact) in [
            (&result.full_acceptance, &full_a),
            (&result.full_error, &full_e),
        ] {
            assert!(less_equal(&interval.lower, &field(exact.clone())));
            assert!(less_equal(&field(exact.clone()), &interval.upper));
        }
        let interval = result.conditional_error.unwrap();
        assert!(less_equal(&interval.lower, &field(&full_e / &full_a)));
        assert!(less_equal(&field(&full_e / &full_a), &interval.upper));
    }
}

#[test]
fn guarded_potential_patterns_are_weighted_once_including_inactive_and_forced_choices() {
    let files = Files::new();
    let registry = ControllerRegistry::default();
    for strategy in [CausalStrategy::FixedTemplate, CausalStrategy::ColdPrefix] {
        for rate in ["0", "1/3", "1"] {
            for inactive in [false, true] {
                let source = if inactive {
                    format!(
                        "CONTROL[XOR] -> ctrl[0]\nX_ERROR({rate}) ctrl[0] 1\nM 1\nOBSERVABLE_INCLUDE(0) rec[-1]"
                    )
                } else {
                    format!(
                        "H 0\nM 0\nX_ERROR({rate}) rec[-1] 1\nM 1\nOBSERVABLE_INCLUDE(0) rec[-1]"
                    )
                };
                let mut evaluation = EvaluationOptions::default();
                evaluation.runtime.strategy = strategy;
                let e = evaluator(
                    &files,
                    &source,
                    r#"{"error_observables":[0]}"#,
                    &registry,
                    evaluation,
                    &NoiseSampling::Physical,
                );
                for cut in [0, 1] {
                    let result = e.enumerate_physical(&opts(cut)).unwrap();
                    let p: BigRational = rate.parse().unwrap();
                    let covered = if cut == 0 {
                        BigRational::one() - &p
                    } else {
                        BigRational::one()
                    };
                    let err = if inactive || cut == 0 {
                        BigRational::zero()
                    } else {
                        &p / r(2, 1)
                    };
                    assert_eq!(result.covered_probability.value(), &covered);
                    assert_eq!(result.masses.accepted, field(covered.clone()));
                    assert_eq!(result.masses.error, field(err.clone()));
                    assert!(result.masses.moments.is_empty());
                    assert_eq!(
                        result.histories,
                        result.patterns * if inactive { 1 } else { 2 }
                    );
                    if strategy == CausalStrategy::FixedTemplate {
                        assert_eq!(result.network_compilations, 0);
                    }
                    if rate == "1/3" && !inactive && cut == 1 {
                        assert_eq!(result.masses.error, field(r(1, 6)));
                    }
                }
            }
        }
    }
}

#[test]
fn projected_and_entangled_partial_targets_and_recovery_keep_exact_normalization() {
    let files = Files::new();
    let registry = ControllerRegistry::default();
    files.put("projected.magna", "H 1\nT 1\nH 1\nCX 1 0");
    files.put("bell.magna", "H 0\nCX 0 1");
    files.put("recovery.magna", "X 0");
    for (task, expected_error, expected_z) in [
        (
            r#"{"outputs":[2],"ideal_state":{"circuit":"projected.magna","outputs":[0],"postselection":{"1":0}},"recovery":"recovery.magna","observables":[[0,1]]}"#,
            r(3, 4),
            r(-1, 2),
        ),
        (
            r#"{"outputs":[2],"ideal_state":{"circuit":"bell.magna","outputs":[1]},"observables":[[0,1]]}"#,
            r(1, 2),
            r(1, 2),
        ),
    ] {
        let e = evaluator(
            &files,
            "X_ERROR(1/4) 2",
            task,
            &registry,
            EvaluationOptions::default(),
            &NoiseSampling::Physical,
        );
        let result = e.enumerate_physical(&opts(1)).unwrap();
        assert_eq!(result.masses.accepted, ExactField::one());
        assert_eq!(result.masses.error, field(expected_error.clone()));
        assert_eq!(
            result.masses.target_success,
            Some(field(BigRational::one() - expected_error))
        );
        assert_eq!(result.masses.moments, [field(expected_z)]);
        assert_eq!(result.omitted_probability.value(), &BigRational::zero());
    }
}

#[test]
fn global_history_query_and_replay_budgets_count_all_patterns_and_incomplete_prefixes() {
    let files = Files::new();
    let mut registry = ControllerRegistry::default();
    registry
        .register("copy", "1", |bits: &[bool]| Ok(bits.to_vec()))
        .unwrap();
    for strategy in [CausalStrategy::FixedTemplate, CausalStrategy::ColdPrefix] {
        let mut evaluation = EvaluationOptions::default();
        evaluation.runtime.strategy = strategy;
        let e = evaluator(
            &files,
            "X_ERROR(1/3) 0\nH 0\nM 0\nDETECTOR rec[-1]\nCONTROL[copy] det[0] -> ctrl[0]\nCX ctrl[0] 1\nM 1\nOBSERVABLE_INCLUDE(0) rec[-1]",
            r#"{"error_observables":[0]}"#,
            &registry,
            evaluation,
            &NoiseSampling::Physical,
        );
        let result = e.enumerate_physical(&opts(1)).unwrap();
        assert_eq!(result.histories, 4);
        assert_eq!(result.patterns, 2);
        assert_eq!(result.masses.error, field(r(1, 2)));
        let mut exact = opts(1);
        exact.histories.max_branches = result.histories;
        exact.histories.max_marginal_queries = result.marginal_queries;
        exact.histories.max_replay_attempts = result.replay_attempts;
        assert_eq!(e.enumerate_physical(&exact).unwrap(), result);
        for option in [
            "max_branches",
            "max_marginal_queries",
            "max_replay_attempts",
        ] {
            let mut limited = exact;
            match option {
                "max_branches" => limited.histories.max_branches -= 1,
                "max_marginal_queries" => limited.histories.max_marginal_queries -= 1,
                _ => limited.histories.max_replay_attempts -= 1,
            }
            let error = e.enumerate_physical(&limited).unwrap_err();
            assert_eq!(error.field, format!("enumeration.{option}"));
            assert!(error.message.contains("configured limit"));
        }
    }
}

#[test]
fn unsupported_sampling_configuration_and_requested_samples_fail_explicitly() {
    let files = Files::new();
    let registry = ControllerRegistry::default();
    for source in ["I 0", "H 0\nM 0\nX_ERROR(1/3) rec[-1] 1"] {
        for scale in ["1", "2"] {
            let e = evaluator(
                &files,
                source,
                "{}",
                &registry,
                EvaluationOptions::default(),
                &NoiseSampling::Importance {
                    proposal_scale: scale.parse().unwrap(),
                },
            );
            assert_eq!(
                e.enumerate_physical(&opts(1)).unwrap_err().field,
                "enumeration.sampling"
            );
        }
    }
    let e = evaluator(
        &files,
        "H 0\nM 0",
        r#"{"sample_records":[0]}"#,
        &registry,
        EvaluationOptions::default(),
        &NoiseSampling::Physical,
    );
    assert_eq!(
        e.enumerate_physical(&opts(1)).unwrap_err().field,
        "enumeration.sample_records"
    );
}

#[test]
fn zero_acceptance_empty_sectors_and_arithmetic_or_contraction_limits_are_explicit() {
    let files = Files::new();
    let registry = ControllerRegistry::default();
    let e = evaluator(
        &files,
        "M 0\nDETECTOR rec[-1]\nPOSTSELECT !det[0]",
        "{}",
        &registry,
        EvaluationOptions::default(),
        &NoiseSampling::Physical,
    );
    let result = e.enumerate_physical(&opts(0)).unwrap();
    assert_eq!((result.patterns, result.histories), (1, 1));
    assert!(result.masses.accepted.is_zero());
    assert!(result.masses.error.is_zero());
    assert!(result.conditional_error.is_none());
    assert!(result.full_acceptance.upper.is_zero());
    let e = evaluator(
        &files,
        "X_ERROR(1) 0",
        "{}",
        &registry,
        EvaluationOptions::default(),
        &NoiseSampling::Physical,
    );
    let result = e.enumerate_physical(&opts(0)).unwrap();
    assert_eq!((result.patterns, result.histories), (0, 0));
    assert_eq!(result.omitted_probability.value(), &BigRational::one());
    let bounds = result.conditional_error.unwrap();
    assert!(bounds.lower.is_zero());
    assert_eq!(bounds.upper, ExactField::one());
    let evaluation = EvaluationOptions {
        fields: FieldArithmetic::new(1),
        ..Default::default()
    };
    let e = evaluator(
        &files,
        "X_ERROR(1/3) 0",
        "{}",
        &registry,
        evaluation,
        &NoiseSampling::Physical,
    );
    let error = e.enumerate_physical(&opts(1)).unwrap_err();
    assert_eq!(error.field, "enumeration.response");
    assert!(error.message.contains("bits"));
    let mut evaluation = EvaluationOptions::default();
    evaluation.runtime.sampling.max_terms = 0;
    let e = evaluator(
        &files,
        "I 0",
        "{}",
        &registry,
        evaluation,
        &NoiseSampling::Physical,
    );
    assert_eq!(
        e.enumerate_physical(&opts(0)).unwrap_err().field,
        "acceptance"
    );
}

#[test]
fn correlated_alternatives_count_as_one_fault_and_report_flips_rescue_selected_errors() {
    let files = Files::new();
    files.put("zero.magna", "I 0 1");
    let registry = ControllerRegistry::default();
    let e = evaluator(
        &files,
        "CORRELATED_ERROR(1/5) X0 X1\nELSE_CORRELATED_ERROR(1/4) Y0 Z1\nM(1/7) 0\nM 1\nDETECTOR rec[-2]\nPOSTSELECT det[0]",
        r#"{"outputs":[0,1],"ideal_state":"zero.magna","observables":[[0,1],[0,2],[0,3]]}"#,
        &registry,
        EvaluationOptions::default(),
        &NoiseSampling::Physical,
    );
    let low = e.enumerate_physical(&opts(1)).unwrap();
    assert_eq!(low.covered_probability.value(), &r(33, 35));
    assert_eq!(low.masses.accepted, field(r(18, 35)));
    assert_eq!(low.masses.error, ExactField::zero());
    assert_eq!(low.patterns, 4);
    assert_eq!(low.by_weight[1].patterns, 3);
    let full = e.enumerate_physical(&opts(2)).unwrap();
    assert_eq!(full.covered_probability.value(), &BigRational::one());
    assert_eq!(full.patterns, 6);
    assert_eq!(full.by_weight[2].probability.value(), &r(2, 35));
    assert_eq!(full.masses.accepted, field(r(4, 7)));
    assert_eq!(full.masses.error, field(r(2, 35)));
    assert_eq!(full.masses.target_success, Some(field(r(18, 35))));
    assert_eq!(
        full.masses.moments,
        [field(r(16, 35)), field(r(18, 35)), field(r(18, 35))]
    );
    assert_eq!(full.conditional_error.unwrap().lower, field(r(1, 10)));
}

#[test]
fn biased_decoder_histories_integrate_born_masses_without_conditional_renormalization() {
    let files = Files::new();
    let mut registry = ControllerRegistry::default();
    registry
        .register("copy", "1", |bits: &[bool]| Ok(bits.to_vec()))
        .unwrap();
    let f = FieldArithmetic::default();
    let p1 = f.value([r(1, 2), r(-1, 4), r(0, 1), r(1, 4)]).unwrap();
    let p0 = f.subtract(&ExactField::one(), &p1).unwrap();
    for strategy in [CausalStrategy::FixedTemplate, CausalStrategy::ColdPrefix] {
        let mut evaluation = EvaluationOptions::default();
        evaluation.runtime.strategy = strategy;
        let e = evaluator(
            &files,
            "H 0\nT 0\nH 0\nM(1/7) 0\nDETECTOR rec[-1]\nCONTROL[copy] det[0] -> ctrl[0]\nCX ctrl[0] 1\nM 1\nOBSERVABLE_INCLUDE(0) rec[-1]",
            r#"{"error_observables":[0]}"#,
            &registry,
            evaluation,
            &NoiseSampling::Physical,
        );
        let low = e.enumerate_physical(&opts(0)).unwrap();
        let no_flip_error = f.scale(&p1, &r(6, 7)).unwrap();
        assert_eq!(low.masses.accepted, field(r(6, 7)));
        assert_eq!(low.masses.error, no_flip_error);
        assert_eq!(low.histories, 2);
        let full = e.enumerate_physical(&opts(1)).unwrap();
        let expected = f
            .add(&no_flip_error, &f.scale(&p0, &r(1, 7)).unwrap())
            .unwrap();
        assert_eq!(full.masses.accepted, ExactField::one());
        assert_eq!(full.masses.error, expected);
        assert_eq!(full.histories, 4);
    }
}
