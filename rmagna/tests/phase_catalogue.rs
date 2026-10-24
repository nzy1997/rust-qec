use rmagna::catalogue::{
    CatalogueLimits, CliffordIndicator, PhaseCatalogue, PhaseDecompositionInput,
};
use rmagna::gf2::BitVector;
use rmagna::phase::ParityPhase;
use rmagna::scalar::{ExactScalar, ScalarArithmetic};
use serde_json::{Value, json};
fn zero(n: usize) -> ParityPhase {
    ParityPhase::new(n, 0, []).unwrap()
}
fn row(n: usize, mask: usize) -> BitVector {
    BitVector::from_indices(n, (0..n).filter(|j| mask >> j & 1 != 0)).unwrap()
}
fn unit(n: usize) -> PhaseDecompositionInput {
    PhaseDecompositionInput {
        dimension: n,
        phase_table: vec![0; 1 << n],
        terms: vec![CliffordIndicator {
            coefficient: ExactScalar::one(),
            constraints: vec![],
            phase: zero(n),
        }],
    }
}
fn ccz() -> PhaseDecompositionInput {
    let a = ScalarArithmetic::default();
    let mut input = unit(3);
    input.phase_table[7] = 4;
    input.terms.push(CliffordIndicator {
        coefficient: a.integer(-2).unwrap(),
        constraints: (0..3).map(|j| (row(3, 1 << j), true)).collect(),
        phase: zero(3),
    });
    input
}
fn ccz_json() -> Value {
    json!({"schema_version":1,"entries":[{"dimension":3,"core_phase_mod8":[0,0,0,0,0,0,0,4],"terms":[{"coefficient":[1,0,0,0],"constraints":[],"clifford_parities":[]},{"coefficient":[-2,0,0,0],"constraints":[[1,1],[2,1],[4,1]],"clifford_parities":[]}]}]})
}
fn certify(source: &str) -> Result<PhaseCatalogue, rmagna::catalogue::CatalogueError> {
    PhaseCatalogue::from_json(
        source,
        &CatalogueLimits::default(),
        &ScalarArithmetic::default(),
    )
}
#[test]
fn exact_complex_overlapping_and_zero_support_identities_are_certified_and_owned() {
    let a = ScalarArithmetic::default();
    let limits = CatalogueLimits::default();
    let mut db = PhaseCatalogue::new();
    db.insert(ccz(), &limits, &a).unwrap();
    let mut input = unit(0);
    input.phase_table[0] = 3;
    input.terms[0].coefficient = a.omega(3, 0).unwrap();
    db.insert(input, &limits, &a).unwrap();
    let mut input = unit(4);
    input.terms[0].phase = ParityPhase::new(4, 5, [(row(4, 9), 2), (row(4, 6), 6)]).unwrap();
    input.phase_table = (0..16)
        .map(|x| input.terms[0].phase.evaluate(&row(4, x)).unwrap())
        .collect();
    // An inconsistent support and a complex cancelling pair do not alter it.
    input.terms.push(CliffordIndicator {
        coefficient: a.omega(1, 0).unwrap(),
        constraints: vec![(row(4, 1), false), (row(4, 1), true)],
        phase: zero(4),
    });
    input.terms.push(CliffordIndicator {
        coefficient: a.omega(7, 0).unwrap(),
        constraints: vec![],
        phase: zero(4),
    });
    input.terms.push(CliffordIndicator {
        coefficient: a.omega(7, 0).unwrap().negated(),
        constraints: vec![],
        phase: zero(4),
    });
    let table = input.phase_table.clone();
    db.insert(input, &limits, &a).unwrap();
    let copy = db.clone();
    drop(db);
    assert_eq!(copy.usage().entries, 3);
    assert_eq!(copy.get(3, &ccz().phase_table).unwrap().terms().len(), 2);
    assert_eq!(
        copy.get(0, &[3]).unwrap().terms()[0].coefficient,
        a.omega(3, 0).unwrap()
    );
    assert_eq!(copy.get(4, &table).unwrap().terms().len(), 4);
    assert!(copy.get(3, &table).is_none());
    let mut source = ccz_json().to_string();
    let loaded = certify(&source).unwrap();
    source.clear();
    assert_eq!(loaded.get(3, &ccz().phase_table).unwrap().dimension(), 3);
    assert_eq!(loaded.usage().verification_work, 48);
}
#[test]
fn all_limits_have_exact_boundaries_and_failures_leave_existing_entries_unchanged() {
    let a = ScalarArithmetic::default();
    let input = ccz();
    let limits = CatalogueLimits {
        max_entries: 1,
        max_dimension: 3,
        max_table_values: 8,
        max_terms: 2,
        max_constraint_rows: 3,
        max_parity_rows: 0,
        max_verification_work: 48,
        ..Default::default()
    };
    let mut db = PhaseCatalogue::new();
    db.insert(input.clone(), &limits, &a).unwrap();
    let usage = db.usage();
    let snapshot = db.clone();
    for (name, bad) in [
        (
            "max_entries",
            CatalogueLimits {
                max_entries: 0,
                ..limits
            },
        ),
        (
            "max_dimension",
            CatalogueLimits {
                max_dimension: 2,
                ..limits
            },
        ),
        (
            "max_table_values",
            CatalogueLimits {
                max_table_values: 7,
                ..limits
            },
        ),
        (
            "max_terms",
            CatalogueLimits {
                max_terms: 1,
                ..limits
            },
        ),
        (
            "max_constraint_rows",
            CatalogueLimits {
                max_constraint_rows: 2,
                ..limits
            },
        ),
        (
            "max_verification_work",
            CatalogueLimits {
                max_verification_work: 47,
                ..limits
            },
        ),
    ] {
        let mut empty = PhaseCatalogue::new();
        let error = empty.insert(input.clone(), &bad, &a).unwrap_err();
        assert!(error.field.contains(name), "{error}");
        assert_eq!(empty.usage().entries, 0);
    }
    let mut clifford = unit(2);
    clifford.terms[0].phase = ParityPhase::new(2, 0, [(row(2, 3), 2)]).unwrap();
    clifford.phase_table = vec![0, 2, 2, 0];
    let error = PhaseCatalogue::new()
        .insert(
            clifford,
            &CatalogueLimits {
                max_parity_rows: 0,
                ..Default::default()
            },
            &a,
        )
        .unwrap_err();
    assert!(error.field.contains("max_parity_rows"));
    let error = db
        .insert(input.clone(), &CatalogueLimits::default(), &a)
        .unwrap_err();
    assert!(error.message.contains("duplicate"));
    assert_eq!(db.usage(), usage);
    assert!(std::ptr::eq(
        db.get(3, &input.phase_table).unwrap(),
        snapshot.get(3, &input.phase_table).unwrap()
    ));
    let mut wrong = unit(2);
    wrong.phase_table[3] = 1;
    let error = db
        .insert(wrong, &CatalogueLimits::default(), &a)
        .unwrap_err();
    assert!(error.field.ends_with("assignment.3"));
    assert_eq!(db.usage(), usage);
    let mut document = ccz_json();
    document["entries"].as_array_mut().unwrap().insert(0,json!({"dimension":0,"core_phase_mod8":[0],"terms":[{"coefficient":[1,0,0,0],"constraints":[],"clifford_parities":[]}]}));
    assert!(
        db.extend_json(&document.to_string(), &CatalogueLimits::default(), &a)
            .is_err()
    );
    assert_eq!(db.usage(), usage);
    assert!(db.get(0, &[0]).is_none());
    let mut zero = unit(3);
    zero.terms[0].coefficient = ExactScalar::zero();
    zero.terms[0].constraints.push((row(2, 1), false));
    let error = PhaseCatalogue::new()
        .insert(zero, &CatalogueLimits::default(), &a)
        .unwrap_err();
    assert!(error.message.contains("dimension"));
    let mut odd = unit(3);
    odd.terms[0].phase = ParityPhase::new(3, 0, [(row(3, 1), 1)]).unwrap();
    assert!(
        PhaseCatalogue::new()
            .insert(odd, &CatalogueLimits::default(), &a)
            .unwrap_err()
            .field
            .contains("clifford_parities")
    );
}
#[test]
fn strict_raw_json_rejects_numeric_tags_shapes_duplicates_and_bad_masks() {
    let mut valid = ccz_json();
    valid["entries"][0]["terms"]
        .as_array_mut()
        .unwrap()
        .push(json!({"coefficient":[0,0,0,0],"constraints":[],"clifford_parities":[[1,1]]}));
    let source = valid.to_string();
    assert_eq!(certify(&source).unwrap().usage().entries, 1);
    for pointer in [
        "/schema_version",
        "/entries/0/dimension",
        "/entries/0/core_phase_mod8/0",
        "/entries/0/terms/0/coefficient/0",
        "/entries/0/terms/1/constraints/0/0",
        "/entries/0/terms/1/constraints/0/1",
        "/entries/0/terms/2/clifford_parities/0/0",
        "/entries/0/terms/2/clifford_parities/0/1",
    ] {
        for bad in [
            json!(1.0),
            json!("1"),
            json!(true),
            Value::Null,
            json!([]),
            json!({"$serde_json::private::Number":"1"}),
            json!({"$serde_json::private::RawValue":"1"}),
        ] {
            let mut input = valid.clone();
            *input.pointer_mut(pointer).unwrap() = bad;
            assert!(certify(&input.to_string()).is_err(), "{pointer}: {input}");
        }
    }
    for pointer in [
        "/entries",
        "/entries/0/terms",
        "/entries/0/terms/0/constraints",
        "/entries/0/terms/0/clifford_parities",
    ] {
        let mut input = valid.clone();
        *input.pointer_mut(pointer).unwrap() = json!({"$serde_json::private::RawValue":"[]"});
        assert!(certify(&input.to_string()).is_err());
    }
    for bad in [
        r#"{"schema_version":1,"schema_version":1,"entries":[]}"#,
        r#"{"schema_version":1,"schema_versi\u006fn":1,"entries":[]}"#,
        r#"{"schema_version":1,"entries":[{"dimension":0,"dimension":0,"core_phase_mod8":[0],"terms":[]}]}"#,
    ] {
        assert!(certify(bad).unwrap_err().message.contains("duplicate"));
    }
    for (pointer, value) in [
        ("/entries/0/terms/1/constraints/0/0", json!(8)),
        ("/entries/0/terms/1/constraints/0/1", json!(2)),
        ("/entries/0/core_phase_mod8/0", json!(8)),
        ("/schema_version", json!(2)),
        ("/entries/0/dimension", json!(64)),
    ] {
        let mut input = valid.clone();
        *input.pointer_mut(pointer).unwrap() = value;
        assert!(certify(&input.to_string()).is_err());
    }
    let mut input = valid.clone();
    input["entries"][0]["terms"][0]["clifford_parities"] = json!([[1, 4.5]]);
    assert!(certify(&input.to_string()).is_err());
    input["entries"][0]["terms"][0]["clifford_parities"] = json!([[8, 1]]);
    assert!(certify(&input.to_string()).is_err());
    let mut input = valid.clone();
    input["entries"][0]["terms"][0]["coefficient"] = json!([1, 0, 0]);
    assert!(certify(&input.to_string()).is_err());
    input["unknown"] = json!(1);
    assert!(certify(&input.to_string()).is_err());
    let mut input = valid;
    input["provenance"] = json!("é");
    let source = input.to_string();
    let exact = CatalogueLimits {
        max_json_bytes: source.len(),
        ..Default::default()
    };
    assert!(PhaseCatalogue::from_json(&source, &exact, &ScalarArithmetic::default()).is_ok());
    assert_eq!(
        PhaseCatalogue::from_json(
            &source,
            &CatalogueLimits {
                max_json_bytes: source.len() - 1,
                ..exact
            },
            &ScalarArithmetic::default()
        )
        .unwrap_err()
        .field,
        "catalogue.max_json_bytes"
    );
    assert!(certify("{é").is_err());
}
#[test]
fn integer_coefficients_are_lossless_and_scalar_budgets_apply_before_certification() {
    let huge = (num_bigint::BigInt::from(1) << 512_usize) + 1;
    let source = format!(
        r#"{{"schema_version":1,"entries":[{{"dimension":0,"core_phase_mod8":[0],"terms":[{{"coefficient":[{huge},0,0,0],"constraints":[],"clifford_parities":[]}},{{"coefficient":[-{huge},0,0,0],"constraints":[],"clifford_parities":[]}},{{"coefficient":[1,0,0,0],"constraints":[],"clifford_parities":[]}}]}}]}}"#
    );
    let db = certify(&source).unwrap();
    assert_eq!(
        db.get(0, &[0]).unwrap().terms()[0]
            .coefficient
            .coefficients()[0],
        huge
    );
    assert!(
        PhaseCatalogue::from_json(
            &source,
            &CatalogueLimits::default(),
            &ScalarArithmetic::new(16).unwrap()
        )
        .unwrap_err()
        .field
        .contains("coefficient_bits")
    );
}

#[test]
fn signed_clifford_turns_are_reduced_modulo_four_and_existing_limits_are_rechecked() {
    for turns in [-1_i64, 3, 7, i64::MAX] {
        let source=json!({"schema_version":1,"entries":[{"dimension":1,"core_phase_mod8":[0,6],"terms":[{"coefficient":[1,0,0,0],"constraints":[],"clifford_parities":[[1,turns]]}]}]}).to_string();
        assert_eq!(certify(&source).unwrap().usage().entries, 1);
    }
    let source=json!({"schema_version":1,"entries":[{"dimension":1,"core_phase_mod8":[0,0],"terms":[{"coefficient":[1,0,0,0],"constraints":[],"clifford_parities":[[1,i64::MIN]]}]}]}).to_string();
    assert_eq!(certify(&source).unwrap().usage().entries, 1);
    let mut db = certify(&ccz_json().to_string()).unwrap();
    let usage = db.usage();
    let error = db
        .extend_json(
            r#"{"schema_version":1,"entries":[]}"#,
            &CatalogueLimits {
                max_terms: 1,
                ..Default::default()
            },
            &ScalarArithmetic::default(),
        )
        .unwrap_err();
    assert_eq!(error.field, "catalogue.max_terms");
    assert_eq!(db.usage(), usage);
}

#[test]
fn coordinate_provenance_is_typed_and_cannot_bypass_entry_reconstruction() {
    let mut source = ccz_json();
    source["entries"][0]["coordinate_derivation"] =
        json!({"source_core":[0,0,0,0,0,0,0,4],"rows":[1,2,4],"convention":"test-only provenance"});
    assert_eq!(certify(&source.to_string()).unwrap().usage().entries, 1);
    for pointer in [
        "/entries/0/coordinate_derivation/source_core/0",
        "/entries/0/coordinate_derivation/rows/0",
        "/entries/0/coordinate_derivation/convention",
    ] {
        let mut bad = source.clone();
        *bad.pointer_mut(pointer).unwrap() = json!({"$serde_json::private::RawValue":"0"});
        assert!(certify(&bad.to_string()).is_err());
    }
    let mut bad = source.clone();
    bad["entries"][0]["core_phase_mod8"][7] = json!(0);
    assert!(
        certify(&bad.to_string())
            .unwrap_err()
            .field
            .ends_with("assignment.7")
    );
    for (key, value) in [
        ("source_core", json!([0])),
        ("rows", json!([1, 2, 8])),
        ("unknown", json!(true)),
    ] {
        let mut bad = source.clone();
        bad["entries"][0]["coordinate_derivation"][key] = value;
        assert!(certify(&bad.to_string()).is_err());
    }
}

#[test]
fn independently_derived_dyadic_complex_overlap_has_exact_work_and_owned_terms() {
    // A+B*(-1)^parity-2*A*delta111-2*B*delta111*(-1)^parity,
    // A=(1+i)/2, B=(1-i)/2. Direct complex arithmetic gives this literal table.
    let a = ScalarArithmetic::default();
    let coefficients = [
        ([1, 0, 1, 0], -1),
        ([1, 0, -1, 0], -1),
        ([-1, 0, -1, 0], 0),
        ([-1, 0, 1, 0], 0),
    ];
    let input = PhaseDecompositionInput {
        dimension: 3,
        phase_table: vec![0, 2, 2, 0, 2, 0, 0, 6],
        terms: coefficients
            .into_iter()
            .enumerate()
            .map(|(j, (values, power))| CliffordIndicator {
                coefficient: a.scalar(values.map(Into::into), power).unwrap(),
                constraints: if j < 2 {
                    vec![]
                } else {
                    vec![(row(3, 1), true), (row(3, 2), true), (row(3, 4), true)]
                },
                phase: ParityPhase::new(
                    3,
                    0,
                    if j % 2 == 1 {
                        vec![(row(3, 7), 4)]
                    } else {
                        vec![]
                    },
                )
                .unwrap(),
            })
            .collect(),
    };
    let limits = CatalogueLimits {
        max_entries: 1,
        max_dimension: 3,
        max_table_values: 8,
        max_terms: 4,
        max_constraint_rows: 6,
        max_parity_rows: 2,
        max_verification_work: 104,
        ..Default::default()
    };
    let mut db = PhaseCatalogue::new();
    db.insert(input.clone(), &limits, &a).unwrap();
    assert_eq!(db.usage().verification_work, 104);
    let copy = db.clone();
    assert!(std::ptr::eq(
        db.get(3, &input.phase_table).unwrap(),
        copy.get(3, &input.phase_table).unwrap()
    ));
    drop(db);
    assert_eq!(
        copy.get(3, &input.phase_table).unwrap().terms()[0].coefficient,
        a.scalar([1.into(), 0.into(), 1.into(), 0.into()], -1)
            .unwrap()
    );
    let mut empty = PhaseCatalogue::new();
    assert_eq!(
        empty
            .insert(
                input,
                &CatalogueLimits {
                    max_verification_work: 103,
                    ..limits
                },
                &a
            )
            .unwrap_err()
            .field,
        "catalogue.max_verification_work"
    );
    assert_eq!(empty.usage().entries, 0);
}

#[test]
fn independent_integer_cancellation_checks_empty_support_coefficients_and_utf8_caps() {
    // Preregistered via integer complex arithmetic, including an inconsistent
    // support with coefficient 2^257+1. It must be bounded even though absent.
    const SOURCE: &str = r#"{"schema_version":1,"provenance":"Independent finite identity with non-Han UTF-8: é","entries":[{"dimension":3,"core_phase_mod8":[2,6,6,2,6,2,2,2],"terms":[{"coefficient":[0,0,1,0],"constraints":[],"clifford_parities":[[7,2]]},{"coefficient":[0,0,-2,0],"constraints":[[1,1],[2,1],[4,1]],"clifford_parities":[[7,2]]},{"coefficient":[1,0,1,0],"constraints":[[5,1]],"clifford_parities":[[3,3]]},{"coefficient":[-1,0,-1,0],"constraints":[[5,1]],"clifford_parities":[[3,3]]},{"coefficient":[231584178474632390847141970017375815706539969331281128078915168015826259279873,0,0,0],"constraints":[[6,0],[6,1]],"clifford_parities":[]}]}]}"#;
    let limits = CatalogueLimits {
        max_json_bytes: SOURCE.len(),
        max_entries: 1,
        max_dimension: 3,
        max_table_values: 8,
        max_terms: 5,
        max_constraint_rows: 7,
        max_parity_rows: 4,
        max_verification_work: 136,
    };
    let a = ScalarArithmetic::new(258).unwrap();
    let db = PhaseCatalogue::from_json(SOURCE, &limits, &a).unwrap();
    assert_eq!(db.usage().verification_work, 136);
    assert_eq!(
        db.get(3, &[2, 6, 6, 2, 6, 2, 2, 2]).unwrap().terms()[4]
            .coefficient
            .coefficients()[0],
        (num_bigint::BigInt::from(1) << 257_usize) + 1
    );
    for (field, bad) in [
        (
            "max_json_bytes",
            CatalogueLimits {
                max_json_bytes: SOURCE.len() - 1,
                ..limits
            },
        ),
        (
            "max_verification_work",
            CatalogueLimits {
                max_verification_work: 135,
                ..limits
            },
        ),
    ] {
        assert_eq!(
            PhaseCatalogue::from_json(SOURCE, &bad, &a)
                .unwrap_err()
                .field,
            format!("catalogue.{field}")
        );
    }
    assert!(
        PhaseCatalogue::from_json(SOURCE, &limits, &ScalarArithmetic::new(257).unwrap())
            .unwrap_err()
            .field
            .contains("coefficient")
    );
    let broken = SOURCE.replace("\"coefficient\":[1,0,1,0]", "\"coefficient\":[1,0,2,0]");
    assert_ne!(SOURCE, broken);
    assert_eq!(
        PhaseCatalogue::from_json(&broken, &limits, &a)
            .unwrap_err()
            .field,
        "catalogue.entries.0.assignment.1"
    );
}

#[test]
fn extension_errors_use_document_indices_and_do_not_commit_valid_prefixes() {
    let limits = CatalogueLimits::default();
    let a = ScalarArithmetic::default();
    let mut db = PhaseCatalogue::new();
    db.insert(unit(0), &limits, &a).unwrap();
    let snapshot = db.clone();
    let mut document = ccz_json();
    document["entries"][0]["core_phase_mod8"][7] = json!(0);
    document["entries"].as_array_mut().unwrap().insert(0,json!({"dimension":1,"core_phase_mod8":[0,0],"terms":[{"coefficient":[1,0,0,0],"constraints":[],"clifford_parities":[]}]}));
    let error = db
        .extend_json(&document.to_string(), &limits, &a)
        .unwrap_err();
    assert_eq!(error.field, "catalogue.entries.1.assignment.7");
    document["entries"][1]["dimension"] = json!(3.0);
    let error = db
        .extend_json(&document.to_string(), &limits, &a)
        .unwrap_err();
    assert_eq!(error.field, "catalogue.entries.1.dimension");
    assert_eq!(db.usage(), snapshot.usage());
    assert!(db.get(1, &[0, 0]).is_none());
    assert!(std::ptr::eq(
        db.get(0, &[0]).unwrap(),
        snapshot.get(0, &[0]).unwrap()
    ));
}

#[test]
fn redundant_json_rows_are_bounded_before_normalization_and_usage_counts_stored_rows() {
    // The two phases cancel exactly. Parsing still needs room for both rows;
    // the immutable certificate stores neither and checks only four work units.
    let source = r#"{"schema_version":1,"entries":[{"dimension":1,"core_phase_mod8":[0,0],"terms":[{"coefficient":[1,0,0,0],"constraints":[],"clifford_parities":[[1,1],[1,-1]]}]}]}"#;
    let limits = CatalogueLimits {
        max_parity_rows: 2,
        max_verification_work: 4,
        ..Default::default()
    };
    let a = ScalarArithmetic::default();
    let db = PhaseCatalogue::from_json(source, &limits, &a).unwrap();
    assert_eq!(db.usage().parity_rows, 0);
    assert_eq!(db.usage().verification_work, 4);
    assert!(
        db.get(1, &[0, 0]).unwrap().terms()[0]
            .phase
            .terms()
            .is_empty()
    );
    assert_eq!(
        PhaseCatalogue::from_json(
            source,
            &CatalogueLimits {
                max_parity_rows: 1,
                ..limits
            },
            &a
        )
        .unwrap_err()
        .field,
        "catalogue.max_parity_rows"
    );
}
