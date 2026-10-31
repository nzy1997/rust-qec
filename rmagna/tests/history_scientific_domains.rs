//! Declared binary64 scientific domain, independent of host long-double ABI.
use num_bigint::BigInt;
use num_rational::BigRational;
use rmagna::field::{ExactField, FieldArithmetic};
use rmagna::numeric::{NumericOptions, approximate_real};
use rmagna::statistics::{
    CheckpointOptions, DiagnosticDomainError, EfficiencyStatus, HistoryEfficiency,
    ScientificDiagnosticDomain, ScientificEfficiency, ScientificEfficiencyStatus as Status,
};
use serde_json::{Value, json};

fn field(n: i64, d: i64) -> ExactField {
    FieldArithmetic::default()
        .from_rational(BigRational::new(n.into(), d.into()))
        .unwrap()
}
fn power(exponent: i32) -> ExactField {
    let (n, d) = if exponent >= 0 {
        (BigInt::from(1) << exponent as usize, BigInt::from(1))
    } else {
        (
            BigInt::from(1),
            BigInt::from(1) << exponent.unsigned_abs() as usize,
        )
    };
    FieldArithmetic::default()
        .from_rational(BigRational::new(n, d))
        .unwrap()
}
fn domain() -> ScientificDiagnosticDomain {
    ScientificDiagnosticDomain::new(-510).unwrap()
}
fn state() -> ScientificEfficiency {
    ScientificEfficiency::new(
        FieldArithmetic::default(),
        domain(),
        NumericOptions::default(),
    )
    .unwrap()
}
fn push(s: &mut ScientificEfficiency, error: Option<&ExactField>) {
    s.push(
        &ExactField::one(),
        error,
        &ExactField::one(),
        &ExactField::one(),
    )
    .unwrap();
}
fn checkpoint(s: &ScientificEfficiency) -> String {
    s.checkpoint_json(CheckpointOptions::default()).unwrap()
}
fn restore(text: &str) -> Result<ScientificEfficiency, rmagna::statistics::CheckpointError> {
    ScientificEfficiency::from_checkpoint_json(
        text,
        FieldArithmetic::default(),
        domain(),
        NumericOptions::default(),
        CheckpointOptions::default(),
    )
}

#[test]
fn original_tiny_error_reports_declared_range_without_clamping_exact_moments() {
    // Original (.5,-100000) scientific tuple is exactly 2^-100001.
    let tiny = power(-100001);
    let mut s = state();
    for _ in 0..2 {
        push(&mut s, Some(&tiny));
    }
    let summary = s.summarize().unwrap();
    assert_eq!(summary.status, Status::DiagnosticRangeExceeded);
    assert_eq!(summary.status.as_str(), "diagnostic_range_exceeded");
    assert_eq!(summary.exact.status, EfficiencyStatus::ZeroResidualVariance);
    assert_eq!(summary.evidence.out_of_domain, 2);
    assert_eq!(summary.evidence.minimum_ratio_frexp_exponent, Some(-100000));
    assert_eq!(summary.kappa_scientific, None);
    assert_eq!(summary.exact.effective_sample_size, field(2, 1));
    assert_eq!(summary.exact.rho, Some(ExactField::one()));
    assert_eq!(summary.exact.mean_weight, Some(ExactField::one()));
    let mut distinct = state();
    push(&mut distinct, Some(&ExactField::zero()));
    push(&mut distinct, Some(&tiny));
    let summary = distinct.summarize().unwrap();
    assert_eq!(summary.status, Status::DiagnosticRangeExceeded);
    assert_eq!(summary.exact.status, EfficiencyStatus::Available);
    assert_eq!(summary.exact.kappa, Some(ExactField::one()));
    assert_eq!(summary.exact.variance_coefficient, Some(power(-200004)));
    assert_eq!(
        summary.exact.exact_marginal_variance_coefficient,
        Some(power(-200004))
    );
}

#[test]
fn native_floor_neighbors_and_priority_are_explicit() {
    for (exponent, range) in [(-510, false), (-511, false), (-512, true)] {
        let mut s = state();
        push(&mut s, Some(&power(exponent)));
        assert_eq!(
            s.evidence().minimum_ratio_frexp_exponent,
            Some(i64::from(exponent) + 1)
        );
        assert_eq!(
            s.summarize().unwrap().status,
            if range {
                Status::DiagnosticRangeExceeded
            } else {
                Status::InsufficientSamples
            }
        );
        push(&mut s, None);
        assert_eq!(s.summarize().unwrap().status, Status::NoErrorMetric);
        assert_eq!(s.evidence().missing_errors, 1);
    }
    assert!(ScientificDiagnosticDomain::new(2).is_err());
    assert_eq!(
        domain().ratio_semantics(),
        "binary64_scientific_mantissa_ratio"
    );
    assert_eq!(
        ScientificDiagnosticDomain::new(i64::MIN)
            .unwrap()
            .min_ratio_frexp_exponent(),
        i64::MIN
    );
}

#[test]
fn zero_acceptance_and_zero_error_do_not_fabricate_ratio_evidence() {
    let mut s = state();
    for _ in 0..2 {
        s.push(
            &ExactField::zero(),
            Some(&ExactField::zero()),
            &ExactField::zero(),
            &ExactField::zero(),
        )
        .unwrap();
    }
    let v = s.summarize().unwrap();
    assert_eq!(v.status, Status::ZeroAcceptance);
    assert_eq!(v.exact.zero_weights, 2);
    assert_eq!(v.exact.effective_sample_size, ExactField::zero());
    assert_eq!(v.exact.rho, None);
    assert_eq!(v.evidence.checked_ratios, 0);
    assert_eq!(v.evidence.minimum_ratio_frexp_exponent, None);
    let mut s = state();
    push(&mut s, Some(&ExactField::zero()));
    push(&mut s, Some(&ExactField::zero()));
    assert_eq!(s.summarize().unwrap().status, Status::ZeroResidualVariance);
    assert_eq!(s.evidence().checked_ratios, 0);
    assert_eq!(restore(&checkpoint(&s)).unwrap().evidence(), s.evidence());
}

#[test]
fn separately_rounded_rational_mantissas_decide_the_threshold() {
    let f = FieldArithmetic::default();
    let a = f.add(&ExactField::one(), &power(-53)).unwrap();
    let e = power(-511);
    // 1+2^-53 ties to 1; exact E/A lies strictly below 2^-511.
    assert_eq!(
        approximate_real(&a, NumericOptions::default())
            .unwrap()
            .scientific
            .mantissa
            .to_bits(),
        1_f64.to_bits()
    );
    assert_eq!(
        f.subtract(&f.divide(&e, &a).unwrap(), &e)
            .unwrap()
            .real_sign()
            .unwrap(),
        std::cmp::Ordering::Less
    );
    let mut s = state();
    s.push(&a, Some(&e), &a, &a).unwrap();
    assert_eq!(s.evidence().minimum_ratio_frexp_exponent, Some(-510));
    assert_eq!(s.evidence().out_of_domain, 0);
    // A normalization crossing is observable even with equal mantissas.
    let mut s = state();
    s.push(
        &power(9),
        Some(&power(-502)),
        &ExactField::one(),
        &ExactField::one(),
    )
    .unwrap();
    assert_eq!(s.evidence().minimum_ratio_frexp_exponent, Some(-510));
}

#[test]
fn separately_rounded_irrational_mantissas_decide_the_threshold() {
    let f = FieldArithmetic::default();
    let sqrt_two = f
        .value([0.into(), 1.into(), 0.into(), (-1).into()].map(BigRational::from_integer))
        .unwrap();
    let e = f
        .multiply(&f.subtract(&sqrt_two, &power(-80)).unwrap(), &power(-511))
        .unwrap();
    for v in [&sqrt_two, &e] {
        assert_eq!(
            approximate_real(v, NumericOptions::default())
                .unwrap()
                .scientific
                .mantissa
                .to_bits(),
            0x3ff6_a09e_667f_3bcd
        );
    }
    let mut s = state();
    s.push(&sqrt_two, Some(&e), &ExactField::one(), &ExactField::one())
        .unwrap();
    assert_eq!(s.evidence().minimum_ratio_frexp_exponent, Some(-510));
    assert_eq!(s.evidence().out_of_domain, 0);
}

#[test]
fn final_kappa_checks_the_scientific_pair_at_binary64_boundaries() {
    let f = FieldArithmetic::default();
    let max = f
        .multiply(
            &f.subtract(&field(2, 1), &power(-52)).unwrap(),
            &power(1023),
        )
        .unwrap();
    for (weight, available) in [
        (power(1023), true),
        (max, true),
        (power(1024), false),
        (power(-1074), true),
        (power(-1075), false),
        (power(-1076), false),
        (f.multiply(&field(3, 2), &power(-1075)).unwrap(), true),
    ] {
        let mut s = state();
        for e in [ExactField::zero(), ExactField::one()] {
            s.push(&ExactField::one(), Some(&e), &weight, &ExactField::one())
                .unwrap();
        }
        let v = s.summarize().unwrap();
        assert_eq!(v.exact.kappa, Some(weight));
        assert_eq!(
            v.status,
            if available {
                Status::Available
            } else {
                Status::DiagnosticRangeExceeded
            }
        );
        assert!(v.kappa_scientific.is_some());
        assert_eq!(v.exact.effective_sample_size, field(2, 1));
    }
}

#[test]
fn invalid_contributions_and_work_budgets_leave_no_partial_evidence() {
    let mut s = state();
    push(&mut s, Some(&field(1, 2)));
    let before = checkpoint(&s);
    let one = ExactField::one();
    let zero = ExactField::zero();
    for (a, e, w, l) in [
        (one.clone(), field(2, 1), one.clone(), one.clone()),
        (one.clone(), zero.clone(), field(-1, 1), one.clone()),
        (one.clone(), zero.clone(), one.clone(), field(-1, 1)),
        (one.clone(), zero.clone(), zero.clone(), one.clone()),
        (one.clone(), zero.clone(), one.clone(), zero.clone()),
        (field(-1, 1), zero.clone(), one.clone(), one.clone()),
    ] {
        assert!(s.push(&a, Some(&e), &w, &l).is_err());
        assert_eq!(checkpoint(&s), before);
    }
    let mut numeric = ScientificEfficiency::new(
        FieldArithmetic::default(),
        domain(),
        NumericOptions { max_work_bits: 0 },
    )
    .unwrap();
    let before = checkpoint(&numeric);
    assert!(matches!(
        numeric.push(&one, Some(&one), &one, &one),
        Err(DiagnosticDomainError::Numeric(_))
    ));
    assert_eq!(checkpoint(&numeric), before);
    let mut arithmetic =
        ScientificEfficiency::new(FieldArithmetic::new(8), domain(), NumericOptions::default())
            .unwrap();
    let before = checkpoint(&arithmetic);
    assert!(
        arithmetic
            .push(&one, Some(&power(-100)), &one, &one)
            .is_err()
    );
    assert_eq!(checkpoint(&arithmetic), before);
    // Successful recovery after either rejection must still start at draw one.
    push(&mut numeric, Some(&zero));
    push(&mut arithmetic, Some(&zero));
    assert_eq!(numeric.evidence().draws, 1);
    assert_eq!(arithmetic.evidence().draws, 1);
}

#[test]
fn merges_are_order_independent_policy_checked_and_resume_exactly() {
    let errors = [
        Some(field(1, 2)),
        Some(power(-512)),
        None,
        Some(ExactField::zero()),
    ];
    let mut all = state();
    let mut parts = Vec::new();
    for e in &errors {
        push(&mut all, e.as_ref());
        let mut part = state();
        push(&mut part, e.as_ref());
        parts.push(part);
    }
    for order in [[0, 1, 2, 3], [3, 1, 0, 2]] {
        let mut joined = state();
        for index in order {
            joined.merge(&parts[index]).unwrap();
            joined = restore(&checkpoint(&joined)).unwrap();
        }
        assert_eq!(checkpoint(&joined), checkpoint(&all));
        assert_eq!(joined.summarize().unwrap(), all.summarize().unwrap());
    }
    let mut resumed = restore(&checkpoint(&parts[0])).unwrap();
    for e in &errors[1..] {
        push(&mut resumed, e.as_ref());
    }
    assert_eq!(checkpoint(&resumed), checkpoint(&all));
    let incompatible = ScientificEfficiency::new(
        FieldArithmetic::default(),
        ScientificDiagnosticDomain::new(-500).unwrap(),
        NumericOptions::default(),
    )
    .unwrap();
    let before = checkpoint(&all);
    assert_eq!(
        all.merge(&incompatible),
        Err(DiagnosticDomainError::IncompatibleDomain)
    );
    assert_eq!(checkpoint(&all), before);
    let mut limited =
        ScientificEfficiency::new(FieldArithmetic::new(8), domain(), NumericOptions::default())
            .unwrap();
    let before = checkpoint(&limited);
    assert!(limited.merge(&parts[1]).is_err());
    assert_eq!(checkpoint(&limited), before);
}

#[test]
fn existing_totals_require_draw_provenance_and_legacy_checkpoints_are_rejected() {
    let mut exact = HistoryEfficiency::new(FieldArithmetic::default()).unwrap();
    assert!(
        ScientificEfficiency::from_empty_exact(exact.clone(), domain(), NumericOptions::default())
            .is_ok()
    );
    exact
        .push(
            &ExactField::one(),
            Some(&field(1, 2)),
            &ExactField::one(),
            &ExactField::one(),
        )
        .unwrap();
    assert!(matches!(
        ScientificEfficiency::from_empty_exact(exact.clone(), domain(), NumericOptions::default()),
        Err(DiagnosticDomainError::MissingDrawProvenance)
    ));
    assert!(restore(&exact.checkpoint_json(CheckpointOptions::default()).unwrap()).is_err());
}

#[test]
fn checkpoint_schema_policy_evidence_empty_and_byte_limits_are_checked() {
    let mut s = state();
    push(&mut s, Some(&power(-512)));
    let value: Value = serde_json::from_str(&checkpoint(&s)).unwrap();
    for (path, replacement) in [
        (vec!["schema_version"], json!(1)),
        (vec!["kind"], json!("history_efficiency")),
        (vec!["policy", "ratio_semantics"], json!("exact_ratio")),
        (vec!["policy", "min_ratio_frexp_exponent"], json!("-511")),
        (vec!["policy", "min_ratio_frexp_exponent"], json!("-0510")),
        (vec!["evidence", "draws"], json!("0")),
        (vec!["evidence", "checked_ratios"], json!("2")),
        (vec!["evidence", "checked_ratios"], json!("0")),
        (vec!["evidence", "out_of_domain"], json!("0")),
        (vec!["evidence", "out_of_domain"], json!("2")),
        (vec!["evidence", "missing_errors"], json!("1")),
        (
            vec!["evidence", "minimum_ratio_frexp_exponent"],
            json!("none"),
        ),
        (vec!["evidence", "minimum_ratio_frexp_exponent"], json!("2")),
        (
            vec!["evidence", "minimum_ratio_frexp_exponent"],
            json!("-0"),
        ),
        (vec!["evidence", "draws"], json!(1)),
        (vec!["evidence", "draws"], json!("01")),
        (vec!["evidence", "draws"], json!("18446744073709551616")),
        (vec!["evidence", "extra"], json!(0)),
        (vec!["policy", "extra"], json!(0)),
        (vec!["extra"], json!(0)),
    ] {
        let mut v = value.clone();
        let mut slot = &mut v;
        for key in path {
            slot = &mut slot[key];
        }
        *slot = replacement;
        assert!(restore(&v.to_string()).is_err(), "{v}");
    }
    for key in ["policy", "evidence"] {
        let mut v = value.clone();
        v[key] = Value::Array(v[key].as_object().unwrap().values().cloned().collect());
        assert!(restore(&v.to_string()).is_err());
    }
    for key in value.as_object().unwrap().keys() {
        let mut v = value.clone();
        v.as_object_mut().unwrap().remove(key);
        assert!(restore(&v.to_string()).is_err());
    }
    let mut v = value.clone();
    v["evidence"]
        .as_object_mut()
        .unwrap()
        .remove("minimum_ratio_frexp_exponent");
    assert!(restore(&v.to_string()).is_err());
    assert!(restore("[]").is_err());
    assert!(restore(&(value.to_string() + " {} ")).is_err());
    let empty = checkpoint(&state());
    assert_eq!(checkpoint(&restore(&empty).unwrap()), empty);
    let mut bad: Value = serde_json::from_str(&empty).unwrap();
    bad["evidence"]["minimum_ratio_frexp_exponent"] = json!("0");
    assert!(restore(&bad.to_string()).is_err());
    let text = checkpoint(&s);
    let options = CheckpointOptions {
        max_json_bytes: text.len(),
    };
    assert_eq!(s.checkpoint_json(options).unwrap(), text);
    assert!(
        ScientificEfficiency::from_checkpoint_json(
            &text,
            FieldArithmetic::default(),
            domain(),
            NumericOptions::default(),
            options
        )
        .is_ok()
    );
    let short = CheckpointOptions {
        max_json_bytes: text.len() - 1,
    };
    assert!(s.checkpoint_json(short).is_err());
    assert!(
        ScientificEfficiency::from_checkpoint_json(
            &text,
            FieldArithmetic::default(),
            domain(),
            NumericOptions::default(),
            short
        )
        .is_err()
    );
}

#[test]
fn count_overflow_on_push_and_merge_is_atomic() {
    // Checkpoints validate basic consistency, not original observation identity.
    let mut v: Value = serde_json::from_str(&checkpoint(&state())).unwrap();
    let mut exact: Value = serde_json::from_str(v["exact_checkpoint"].as_str().unwrap()).unwrap();
    exact["statistics"]["count"] = json!(u64::MAX.to_string());
    exact["zero_weights"] = json!(u64::MAX.to_string());
    v["exact_checkpoint"] = json!(exact.to_string());
    v["evidence"]["draws"] = json!(u64::MAX.to_string());
    let mut s = restore(&v.to_string()).unwrap();
    let before = checkpoint(&s);
    let zero = ExactField::zero();
    assert!(s.push(&zero, Some(&zero), &zero, &zero).is_err());
    assert_eq!(checkpoint(&s), before);
    let mut other = state();
    other.push(&zero, Some(&zero), &zero, &zero).unwrap();
    assert!(s.merge(&other).is_err());
    assert_eq!(checkpoint(&s), before);
}

#[test]
fn range_flags_preserve_actual_unequal_weight_diagnostics() {
    let mut s = state();
    for (w, e) in [(1, field(1, 2)), (2, power(-512)), (4, field(1, 4))] {
        s.push(
            &ExactField::one(),
            Some(&e),
            &field(w, 1),
            &ExactField::one(),
        )
        .unwrap();
    }
    let v = s.summarize().unwrap();
    assert_eq!(v.status, Status::DiagnosticRangeExceeded);
    assert_eq!(v.exact.status, EfficiencyStatus::Available);
    assert_eq!(v.exact.effective_sample_size, field(7, 3));
    assert_eq!(v.exact.rho, Some(field(9, 7)));
    assert_eq!(v.exact.mean_weight, Some(field(7, 3)));
    assert_eq!(v.exact, s.exact().summarize().unwrap());
    assert_eq!(restore(&checkpoint(&s)).unwrap().summarize().unwrap(), v);
}

#[test]
fn final_pair_range_is_distinct_from_independent_exact_rounding() {
    use rmagna::numeric::Binary64;
    let f = FieldArithmetic::default();
    let weight = f
        .multiply(
            &power(-1075),
            &f.add(&ExactField::one(), &power(-54)).unwrap(),
        )
        .unwrap();
    let rounded = approximate_real(&weight, NumericOptions::default()).unwrap();
    assert!(matches!(rounded.binary64, Binary64::Finite(x) if x.to_bits() == 1));
    assert_eq!(rounded.scientific.mantissa, 1.0);
    assert_eq!(rounded.scientific.exponent, -1075);
    let mut s = state();
    for e in [ExactField::zero(), ExactField::one()] {
        s.push(&ExactField::one(), Some(&e), &weight, &ExactField::one())
            .unwrap();
    }
    let v = s.summarize().unwrap();
    assert_eq!(v.exact.kappa, Some(weight));
    assert_eq!(v.status, Status::DiagnosticRangeExceeded);
    assert_eq!(v.kappa_scientific, Some(rounded.scientific));
}
