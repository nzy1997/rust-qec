use rmagna::circuit::{BitAtom, BitExpr, ParseOptions, parse_circuit};
use rmagna::instrument::{CompileOptions, CompiledInstrument, FaultSelection, compile_affine};
use rmagna::records::{
    RecordEnumerationOptions, RecordQuery, SamplingError, enumerate_reported_conditioned,
};
use rmagna::scalar::{ExactScalar, ScalarArithmetic};
use std::collections::BTreeMap;
fn compile(text: &str) -> CompiledInstrument {
    compile_options(text, CompileOptions::default())
}
fn compile_options(text: &str, options: CompileOptions) -> CompiledInstrument {
    compile_affine(
        &parse_circuit(text, &ParseOptions::default()).unwrap(),
        &options,
        &ScalarArithmetic::default(),
    )
    .unwrap()
}
fn rec(j: usize) -> BitExpr {
    BitExpr::atom(BitAtom::Record(j))
}
fn xor(mut left: BitExpr, right: &BitExpr) -> BitExpr {
    left.xor_assign(right);
    left
}
fn value(p: &BitExpr, bits: &[bool]) -> bool {
    p.terms.iter().fold(p.constant, |sum, atom| {
        sum ^ match atom {
            BitAtom::Record(j) => bits[*j],
            _ => panic!("oracle accepts reported records only"),
        }
    })
}
#[test]
fn biased_joint_groups_and_report_flips_match_an_independent_four_state_distribution() {
    let a = ScalarArithmetic::default();
    let p0 = a
        .scalar([2.into(), 1.into(), 0.into(), (-1).into()], -2)
        .unwrap();
    let p1 = a
        .scalar([2.into(), (-1).into(), 0.into(), 1.into()], -2)
        .unwrap();
    let groups = [
        vec![
            rec(0),
            xor(rec(1), &rec(2)),
            rec(0).inverted(),
            xor(rec(0), &rec(2)),
            BitExpr {
                constant: true,
                terms: Default::default(),
            },
        ],
        vec![rec(1)],
        vec![],
    ];
    let givens = [
        vec![],
        vec![(rec(1), false)],
        vec![(xor(rec(0), &rec(1)), true)],
    ];
    for strategy in [
        rmagna::unitary::CompilerStrategy::ParityCopies,
        rmagna::unitary::CompilerStrategy::SharedCommutingLayer,
    ] {
        for preparation in [
            rmagna::instrument::QueryPreparation::Cold,
            rmagna::instrument::QueryPreparation::Trace,
            rmagna::instrument::QueryPreparation::FixedFibers(Default::default()),
        ] {
            let instrument = compile_options(
                "H 0\nT 0\nH 0\nCX 0 1\nH 2\nM(1/7) 0 1 2\nDETECTOR rec[-2]\nPOSTSELECT det[0]",
                CompileOptions {
                    compiler_strategy: strategy,
                    probability_preparation: preparation,
                    ..Default::default()
                },
            );
            for mask in 0..8 {
                let faults = (0..3)
                    .filter_map(|site| {
                        (mask >> site & 1 != 0).then_some(FaultSelection {
                            site,
                            alternative: 0,
                        })
                    })
                    .collect::<Vec<_>>();
                for parities in &groups {
                    for given in &givens {
                        let mut expected = BTreeMap::<Vec<bool>, ExactScalar>::new();
                        for b in [false, true] {
                            for c in [false, true] {
                                let reported = [
                                    b ^ (mask & 1 != 0),
                                    b ^ (mask & 2 != 0),
                                    c ^ (mask & 4 != 0),
                                ];
                                if given.iter().any(|(p, v)| value(p, &reported) != *v) {
                                    continue;
                                }
                                let key = parities
                                    .iter()
                                    .map(|p| value(p, &reported))
                                    .collect::<Vec<_>>();
                                let probability = a
                                    .multiply(if b { &p1 } else { &p0 }, &a.omega(0, -2).unwrap())
                                    .unwrap();
                                let sum = expected.entry(key).or_insert_with(ExactScalar::zero);
                                *sum = a.add(sum, &probability).unwrap();
                            }
                        }
                        let query = RecordQuery { given, parities };
                        let actual = enumerate_reported_conditioned(
                            &instrument,
                            &query,
                            &faults,
                            &RecordEnumerationOptions::default(),
                            &a,
                        );
                        if expected.is_empty() {
                            assert!(matches!(
                                actual,
                                Err(SamplingError::ImpossibleSupport)
                                    | Err(SamplingError::ImpossibleCondition)
                            ));
                            continue;
                        }
                        let actual = actual.unwrap();
                        let observed = actual
                            .branches
                            .iter()
                            .map(|b| (b.values.clone(), b.mass.clone()))
                            .collect::<BTreeMap<_, _>>();
                        assert_eq!(observed.len(), actual.branches.len());
                        assert_eq!(observed, expected);
                        let sum = expected
                            .values()
                            .fold(ExactScalar::zero(), |sum, p| a.add(&sum, p).unwrap());
                        assert_eq!(actual.condition_mass, sum);
                        assert!(
                    actual
                        .branches
                        .iter()
                        .all(|b| b.mass.real_sign().unwrap() == std::cmp::Ordering::Greater)
                );
                        assert_eq!(
                            actual,
                            enumerate_reported_conditioned(
                                &instrument,
                                &query,
                                &faults,
                                &RecordEnumerationOptions::default(),
                                &a
                            )
                            .unwrap()
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn physical_interference_pruning_allows_a_single_branch_inside_rank_one_support() {
    let a = ScalarArithmetic::default();
    let instrument = compile("H 0\nT 0\nT_DAG 0\nH 0\nM 0\nDETECTOR rec[-1]\nPOSTSELECT det[0]");
    let parities = [rec(0)];
    let query = RecordQuery {
        given: &[],
        parities: &parities,
    };
    let options = RecordEnumerationOptions {
        max_branches: 1,
        max_record_bits: 1,
        max_marginal_queries: 2,
        ..Default::default()
    };
    let result = enumerate_reported_conditioned(&instrument, &query, &[], &options, &a).unwrap();
    assert_eq!(result.structural_rank, 1);
    assert_eq!(result.marginal_queries, 2);
    assert_eq!(result.branches.len(), 1);
    assert_eq!(result.branches[0].values, [false]);
    assert_eq!(result.branches[0].mass, ExactScalar::one());
    assert_eq!(result.condition_mass, ExactScalar::one());
    let given = [(rec(0), true)];
    assert!(matches!(
        enumerate_reported_conditioned(
            &instrument,
            &RecordQuery {
                given: &given,
                parities: &parities
            },
            &[],
            &options,
            &a
        ),
        Err(SamplingError::ImpossibleCondition)
    ));
    assert!(matches!(
        enumerate_reported_conditioned(
            &instrument,
            &query,
            &[],
            &RecordEnumerationOptions {
                max_marginal_queries: 1,
                ..options
            },
            &a
        ),
        Err(SamplingError::EnumerationLimit {
            option: "max_marginal_queries",
            required: 2,
            ..
        })
    ));
}
#[test]
fn correlated_records_and_complemented_labels_across_words_use_one_unresolved_bit() {
    let a = ScalarArithmetic::default();
    let targets = (0..65).map(|j| j.to_string()).collect::<Vec<_>>().join(" ");
    let mut source = String::from("H 0\n");
    for j in 1..65 {
        source.push_str(&format!("CX 0 {j}\n"));
    }
    source.push_str(&format!("M {targets}\nDETECTOR rec[-1]\nPOSTSELECT det[0]"));
    let instrument = compile(&source);
    let parities = (0..129)
        .map(|j| {
            if j % 2 == 0 {
                rec(j % 65)
            } else {
                rec(j % 65).inverted()
            }
        })
        .collect::<Vec<_>>();
    let result = enumerate_reported_conditioned(
        &instrument,
        &RecordQuery {
            given: &[],
            parities: &parities,
        },
        &[],
        &RecordEnumerationOptions {
            max_record_bits: 1,
            max_branches: 2,
            ..Default::default()
        },
        &a,
    )
    .unwrap();
    assert_eq!(result.structural_rank, 1);
    assert_eq!(result.branches.len(), 2);
    assert_eq!(result.marginal_queries, 2);
    for branch in &result.branches {
        let bit = branch.values[0];
        assert_eq!(
            branch.values,
            (0..129).map(|j| bit ^ (j % 2 != 0)).collect::<Vec<_>>()
        );
        assert_eq!(branch.mass, a.omega(0, -2).unwrap());
    }
}
#[test]
fn independent_rank_positive_branches_and_actual_query_counts_have_explicit_limits() {
    let a = ScalarArithmetic::default();
    let instrument = compile("H 0 1\nM 0 1");
    let parities = [rec(0), rec(1)];
    let query = RecordQuery {
        given: &[],
        parities: &parities,
    };
    let options = RecordEnumerationOptions {
        max_branches: 4,
        max_record_bits: 2,
        max_marginal_queries: 4,
        ..Default::default()
    };
    let result = enumerate_reported_conditioned(&instrument, &query, &[], &options, &a).unwrap();
    assert_eq!(result.marginal_queries, 4);
    assert_eq!(result.structural_rank, 2);
    assert_eq!(
        result
            .branches
            .iter()
            .map(|b| b.values.clone())
            .collect::<Vec<_>>(),
        [
            vec![false, false],
            vec![false, true],
            vec![true, false],
            vec![true, true]
        ]
    );
    for branch in result.branches {
        assert_eq!(branch.mass, a.omega(0, -4).unwrap());
    }
    for (option, limited) in [
        (
            "max_branches",
            RecordEnumerationOptions {
                max_branches: 3,
                ..options
            },
        ),
        (
            "max_record_bits",
            RecordEnumerationOptions {
                max_record_bits: 1,
                ..options
            },
        ),
        (
            "max_marginal_queries",
            RecordEnumerationOptions {
                max_marginal_queries: 3,
                ..options
            },
        ),
    ] {
        assert!(
            matches!(enumerate_reported_conditioned(&instrument,&query,&[],&limited,&a),Err(SamplingError::EnumerationLimit {option:actual,..}) if actual==option)
        );
    }
    let targets = (0..65).map(|j| j.to_string()).collect::<Vec<_>>().join(" ");
    let wide = compile(&format!("H {targets}\nM {targets}"));
    let parities = (0..65).map(rec).collect::<Vec<_>>();
    assert!(matches!(
        enumerate_reported_conditioned(
            &wide,
            &RecordQuery {
                given: &[],
                parities: &parities
            },
            &[],
            &RecordEnumerationOptions::default(),
            &a
        ),
        Err(SamplingError::EnumerationLimit {
            option: "max_record_bits",
            required: 65,
            ..
        })
    ));
}
#[test]
fn deterministic_empty_and_conditioned_groups_keep_joint_mass_and_reject_impossible_conditions() {
    let a = ScalarArithmetic::default();
    let instrument = compile("H 0\nM 0");
    let given = [(rec(0), true)];
    let parities = [
        rec(0),
        rec(0).inverted(),
        BitExpr {
            constant: true,
            terms: Default::default(),
        },
    ];
    let options = RecordEnumerationOptions {
        max_record_bits: 0,
        max_branches: 1,
        max_marginal_queries: 1,
        ..Default::default()
    };
    let result = enumerate_reported_conditioned(
        &instrument,
        &RecordQuery {
            given: &given,
            parities: &parities,
        },
        &[],
        &options,
        &a,
    )
    .unwrap();
    assert_eq!(result.structural_rank, 0);
    assert_eq!(result.marginal_queries, 1);
    assert_eq!(result.branches[0].values, [true, false, true]);
    assert_eq!(result.branches[0].mass, a.omega(0, -2).unwrap());
    assert_eq!(result.condition_mass, result.branches[0].mass);
    let empty = enumerate_reported_conditioned(
        &instrument,
        &RecordQuery {
            given: &given,
            parities: &[],
        },
        &[],
        &options,
        &a,
    )
    .unwrap();
    assert_eq!(empty.branches.len(), 1);
    assert!(empty.branches[0].values.is_empty());
    assert_eq!(empty.condition_mass, a.omega(0, -2).unwrap());
    let contradictory = [(rec(0), false), (rec(0), true)];
    assert!(matches!(
        enumerate_reported_conditioned(
            &instrument,
            &RecordQuery {
                given: &contradictory,
                parities: &[]
            },
            &[],
            &options,
            &a
        ),
        Err(SamplingError::ImpossibleSupport)
    ));
    for (option, limited) in [
        (
            "max_branches",
            RecordEnumerationOptions {
                max_branches: 0,
                ..options
            },
        ),
        (
            "max_marginal_queries",
            RecordEnumerationOptions {
                max_marginal_queries: 0,
                ..options
            },
        ),
    ] {
        assert!(
            matches!(enumerate_reported_conditioned(&instrument,&RecordQuery {given:&given,parities:&[]},&[],&limited,&a),Err(SamplingError::EnumerationLimit {option:actual,required:1,..}) if actual==option)
        );
    }
}
