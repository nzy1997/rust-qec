#[allow(dead_code)]
mod common;

use common::{bits, dense_gate};
use rmagna::circuit::{BitAtom, BitExpr, Circuit, ParseOptions, parse_circuit};
use rmagna::clifford::{CliffordCircuit, CliffordGate as C};
use rmagna::gf2::BitVector;
use rmagna::instrument::{
    CompileOptions, FaultSelection, InsertionPreparation, InstrumentError, InstrumentQueryLayout,
    QueryPreparation, QueryPreparationOutcome, compile_affine, compile_affine_stabilizer,
};
use rmagna::noise::{NoiseSampling, SplitMix64};
use rmagna::pauli::PauliWord;
use rmagna::quadratic::QuadraticForm;
use rmagna::runtime::{
    CausalProgram, CausalStrategy, ControllerCall, ControllerRegistry, HistorySampler,
    RuntimeError, RuntimeOptions,
};
use rmagna::scalar::{ExactScalar, ScalarArithmetic, ScalarError};
use rmagna::stabilizer::AffineKet;
use rmagna::unitary::{
    CompilerStrategy, InsertionPreparationOptions, UnitaryCircuit, UnitaryError,
};
use std::sync::{Arc, Mutex};

const TERMS: usize = 65536;
const LOWERINGS: [CompilerStrategy; 2] = [
    CompilerStrategy::ParityCopies,
    CompilerStrategy::SharedCommutingLayer,
];
const HISTORIES: [CausalStrategy; 2] = [CausalStrategy::FixedTemplate, CausalStrategy::ColdPrefix];

fn parse(text: &str, registry: &ControllerRegistry) -> Circuit {
    parse_circuit(
        text,
        &ParseOptions {
            controllers: registry.names(),
            ..Default::default()
        },
    )
    .unwrap()
}
fn record(j: usize) -> BitExpr {
    BitExpr::atom(BitAtom::Record(j))
}
fn parity(indices: &[usize], constant: bool) -> BitExpr {
    BitExpr {
        constant,
        terms: indices.iter().map(|&j| BitAtom::Record(j)).collect(),
    }
}
fn options(strategy: CompilerStrategy, prepared: bool) -> CompileOptions {
    let mode = if prepared {
        QueryPreparation::FixedFibers(Default::default())
    } else {
        QueryPreparation::Cold
    };
    CompileOptions {
        compiler_strategy: strategy,
        probability_preparation: mode,
        density_preparation: mode,
        moment_preparation: mode,
        insertion_preparation: if prepared {
            InsertionPreparation::Prepared(InsertionPreparationOptions::default())
        } else {
            InsertionPreparation::Direct
        },
        ..Default::default()
    }
}
// Literal states below specify every amplitude independently of affine/quadratic
// evaluation, normalization, contraction, or compiler preparation.
fn complex_input() -> (AffineKet, Vec<ExactScalar>) {
    let a = ScalarArithmetic::default();
    let input = AffineKet::new(
        bits(3, 4),
        vec![bits(3, 3), bits(3, 6)],
        QuadraticForm::new(3, vec![1, 3], vec![bits(2, 2), bits(2, 0)]).unwrap(),
        a.omega(5, -2).unwrap(),
    )
    .unwrap();
    let mut dense = vec![ExactScalar::zero(); 8];
    for (label, phase) in [(4, 3), (7, 5), (2, 1), (1, 7)] {
        dense[label] = a.omega(phase, -2).unwrap();
    }
    (input, dense)
}
fn bell_input() -> (AffineKet, Vec<ExactScalar>) {
    let a = ScalarArithmetic::default();
    let input = AffineKet::new(
        bits(3, 4),
        vec![bits(3, 3)],
        QuadraticForm::new(3, vec![1], vec![bits(1, 0)]).unwrap(),
        a.omega(5, -1).unwrap(),
    )
    .unwrap();
    let mut dense = vec![ExactScalar::zero(); 8];
    dense[4] = a.omega(3, -1).unwrap();
    dense[7] = a.omega(5, -1).unwrap();
    (input, dense)
}
fn norm(v: &[ExactScalar]) -> ExactScalar {
    let a = ScalarArithmetic::default();
    v.iter().fold(ExactScalar::zero(), |s, x| {
        a.add(&s, &a.norm_squared(x).unwrap()).unwrap()
    })
}
fn pauli(v: &[ExactScalar], x: usize, z: usize, phase: i64) -> Vec<ExactScalar> {
    let a = ScalarArithmetic::default();
    let mut out = vec![ExactScalar::zero(); v.len()];
    for (label, value) in v.iter().enumerate() {
        let turns = phase + 4 * i64::from((label & z).count_ones() & 1);
        out[label ^ x] = a.multiply(value, &a.omega(turns, 0).unwrap()).unwrap();
    }
    out
}
fn project(v: &[ExactScalar], x: usize, z: usize, phase: i64, outcome: bool) -> Vec<ExactScalar> {
    let a = ScalarArithmetic::default();
    let p = pauli(v, x, z, phase);
    v.iter()
        .zip(p)
        .map(|(v, p)| {
            a.multiply(
                &a.add(v, &if outcome { p.negated() } else { p }).unwrap(),
                &a.omega(0, -2).unwrap(),
            )
            .unwrap()
        })
        .collect()
}
fn diagonal(v: &mut [ExactScalar], turns: impl Fn(usize) -> i64) {
    let a = ScalarArithmetic::default();
    for (label, value) in v.iter_mut().enumerate() {
        *value = a
            .multiply(value, &a.omega(turns(label), 0).unwrap())
            .unwrap();
    }
}
#[derive(Clone)]
struct Branch {
    state: Vec<ExactScalar>,
    records: Vec<bool>,
}
fn measure(branches: Vec<Branch>, x: usize, z: usize, phase: i64, flip: bool) -> Vec<Branch> {
    branches
        .into_iter()
        .flat_map(|b| {
            [false, true].into_iter().filter_map(move |value| {
                let state = project(&b.state, x, z, phase, value);
                if norm(&state).is_zero() {
                    return None;
                }
                let mut records = b.records.clone();
                records.push(value ^ flip);
                Some(Branch { state, records })
            })
        })
        .collect()
}
fn oracle_instrument(dense: Vec<ExactScalar>, flip: bool, fault: bool) -> Vec<Branch> {
    let a = ScalarArithmetic::default();
    let mut v = dense;
    diagonal(&mut v, |s| (s & 1) as i64);
    dense_gate(C::H(1), &mut v, &a);
    diagonal(&mut v, |s| {
        2 * ((s & 1) * ((s >> 2) & 1)) as i64
            + 4 * ((s & 1) * ((s >> 1) & 1) * ((s >> 2) & 1)) as i64
    });
    let mut branches = measure(
        vec![Branch {
            state: v,
            records: vec![],
        }],
        0,
        1,
        0,
        flip,
    );
    if fault {
        for b in &mut branches {
            b.state = pauli(&b.state, 2, 0, 0);
        }
    }
    branches = measure(branches, 2, 2, 2, false);
    // Trace the old wire and replace it by zero. The unobserved alternatives
    // stay separate density trajectories; no coherent sum repairs a discard.
    let mut reset = vec![];
    for b in branches {
        for value in [false, true] {
            let v = project(&b.state, 0, 1, 0, value);
            let mut state = if value { pauli(&v, 1, 0, 0) } else { v };
            if norm(&state).is_zero() {
                continue;
            }
            dense_gate(C::H(0), &mut state, &a);
            reset.push(Branch {
                state,
                records: b.records.clone(),
            });
        }
    }
    measure(measure(reset, 0, 1, 0, false), 1, 4, 0, false)
}
fn sum_masses(branches: &[Branch], selected: impl Fn(&Branch) -> bool) -> ExactScalar {
    let a = ScalarArithmetic::default();
    branches
        .iter()
        .filter(|b| selected(b))
        .fold(ExactScalar::zero(), |s, b| {
            a.add(&s, &norm(&b.state)).unwrap()
        })
}
fn accepted(b: &Branch) -> bool {
    b.records[2] == b.records[3]
}

#[test]
fn initialized_instruments_match_independent_projectors_discard_and_complex_moments() {
    let a = ScalarArithmetic::default();
    let source = parse(
        "T 0\nH 1\nCS 0 2\nCCZ 0 1 2\nM(1/7) 0\nX_ERROR(1/3) 1\nMY 1\nR 0\nH 0\nM 0\nMPP X0*Z2\nDETECTOR rec[-1] rec[-2]\nPOSTSELECT det[0]",
        &ControllerRegistry::default(),
    );
    let (input, dense) = complex_input();
    let original = input.clone();
    let rows = [
        parity(&[0], true),
        record(1),
        parity(&[0, 2], false),
        record(3),
    ];
    let target = UnitaryCircuit::new(3, [])
        .unwrap()
        .compile(&bits(3, 0), &a)
        .unwrap();
    for strategy in LOWERINGS {
        for prepared in [false, true] {
            let program =
                compile_affine_stabilizer(&source, &input, &options(strategy, prepared), &a)
                    .unwrap();
            assert_eq!(program.num_qubits(), 3);
            assert_eq!(program.num_records(), 4);
            assert_eq!(program.noise_sites().len(), 5);
            for layout in [
                InstrumentQueryLayout::ReportedProbability { parities: &rows },
                InstrumentQueryLayout::AcceptedProbability { parities: &rows },
            ] {
                assert_eq!(
                    program.prepare_query(layout, TERMS, &a).unwrap(),
                    if prepared {
                        QueryPreparationOutcome::Created
                    } else {
                        QueryPreparationOutcome::Cold
                    }
                );
                assert_eq!(
                    program.prepare_query(layout, TERMS, &a).unwrap(),
                    if prepared {
                        QueryPreparationOutcome::Reused
                    } else {
                        QueryPreparationOutcome::Cold
                    }
                );
            }
            for flip in [false, true] {
                for fault in [false, true] {
                    let faults = [(0, flip), (1, fault)]
                        .into_iter()
                        .filter_map(|(site, yes)| {
                            yes.then_some(FaultSelection {
                                site,
                                alternative: 0,
                            })
                        })
                        .collect::<Vec<_>>();
                    let branches = oracle_instrument(dense.clone(), flip, fault);
                    assert_eq!(sum_masses(&branches, |_| true), ExactScalar::one());
                    for outcome in 0..16 {
                        let matches = |b: &Branch| {
                            [
                                !b.records[0],
                                b.records[1],
                                b.records[0] ^ b.records[2],
                                b.records[3],
                            ]
                            .iter()
                            .enumerate()
                            .all(|(j, &v)| v == (outcome >> j & 1 != 0))
                        };
                        let values = bits(4, outcome);
                        assert_eq!(
                            program
                                .reported_probability(&rows, &values, &faults, TERMS, &a)
                                .unwrap()
                                .value,
                            sum_masses(&branches, matches)
                        );
                        assert_eq!(
                            program
                                .accepted_probability(&rows, &values, &faults, TERMS, &a)
                                .unwrap()
                                .value,
                            sum_masses(&branches, |b| accepted(b) && matches(b))
                        );
                    }
                    let masses = program
                        .masses(&[2, 0, 1], &target, &faults, TERMS, &a)
                        .unwrap();
                    assert_eq!(masses.accepted.value, sum_masses(&branches, accepted));
                    let zero = branches
                        .iter()
                        .filter(|b| accepted(b))
                        .fold(ExactScalar::zero(), |s, b| {
                            a.add(&s, &a.norm_squared(&b.state[0]).unwrap()).unwrap()
                        });
                    assert_eq!(masses.target_success.value, zero);
                    let wanted = branches.iter().filter(|b| accepted(b)).fold(
                        ExactScalar::zero(),
                        |sum, b| {
                            let p = pauli(&b.state, 1, 4, 1);
                            b.state.iter().zip(p).fold(sum, |s, (v, p)| {
                                a.add(&s, &a.multiply(&v.conjugate(), &p).unwrap()).unwrap()
                            })
                        },
                    );
                    let word = PauliWord::new(bits(3, 2), bits(3, 1), 1).unwrap();
                    assert_eq!(
                        program
                            .pauli_moment(&[2, 0, 1], &word, &faults, TERMS, &a)
                            .unwrap()
                            .value,
                        wanted
                    );
                    let duplicate = [record(0), record(0)];
                    assert!(
                        program
                            .reported_probability(&duplicate, &bits(2, 1), &faults, TERMS, &a)
                            .unwrap()
                            .value
                            .is_zero()
                    );
                }
            }
            assert_eq!(
                program
                    .prepared_probability_statistics()
                    .unwrap()
                    .cached_queries,
                if prepared { 4 } else { 0 }
            );
            assert_eq!(program.prepared_insertion_statistics().is_some(), prepared);
        }
    }
    assert_eq!(input, original);
}

type Transcript = Arc<Mutex<Vec<Vec<bool>>>>;
fn registry() -> (ControllerRegistry, Transcript) {
    let mut registry = ControllerRegistry::default();
    let seen = Arc::new(Mutex::new(vec![]));
    for (name, xor) in [("copy", false), ("xor", true)] {
        let log = seen.clone();
        registry
            .register(name, "1", move |v: &[bool]| {
                log.lock().unwrap().push(v.to_vec());
                Ok(vec![if xor { v[0] ^ v[1] } else { v[0] }])
            })
            .unwrap();
    }
    (registry, seen)
}
fn causal_source(registry: &ControllerRegistry) -> Circuit {
    parse(
        "M(1/7) 0\nDETECTOR rec[-1]\nCONTROL[copy] det[0] -> ctrl[0]\nCH ctrl[0] 1\nX_ERROR(1/3) ctrl[0] 1\nM 1\nDETECTOR rec[-1]\nCONTROL[xor] det[1] ctrl[0] -> ctrl[1]\nH 2\nCS ctrl[1] 2\nT 2\nH 2\nM 2\nDETECTOR rec[-1]\nPOSTSELECT ctrl[1] det[2]",
        registry,
    )
}
fn oracle_history(
    mut v: Vec<ExactScalar>,
    flip: bool,
    fault: bool,
    m: [bool; 3],
) -> Vec<ExactScalar> {
    let a = ScalarArithmetic::default();
    v = project(&v, 0, 1, 0, m[0] ^ flip);
    if m[0] {
        dense_gate(C::H(1), &mut v, &a);
        if fault {
            v = pauli(&v, 2, 0, 0);
        }
    }
    v = project(&v, 0, 2, 0, m[1]);
    dense_gate(C::H(2), &mut v, &a);
    diagonal(&mut v, |s| {
        ((s >> 2) & 1) as i64 * if m[0] ^ m[1] { 3 } else { 1 }
    });
    dense_gate(C::H(2), &mut v, &a);
    project(&v, 0, 4, 0, m[2])
}

#[test]
fn initialized_fixed_and_cold_histories_match_literal_mass_callbacks_and_compilation_counts() {
    let a = ScalarArithmetic::default();
    let (registry, seen) = registry();
    let source = causal_source(&registry);
    let (input, dense) = bell_input();
    let original = input.clone();
    let request = [record(2), parity(&[2], true), parity(&[0, 1], false)];
    for lowering in LOWERINGS {
        for prepared in [false, true] {
            let programs = HISTORIES.map(|strategy| {
                CausalProgram::new_stabilizer(
                    &source,
                    &input,
                    &registry,
                    RuntimeOptions {
                        strategy,
                        compile: options(lowering, prepared),
                        ..Default::default()
                    },
                    &a,
                )
                .unwrap()
            });
            assert!(seen.lock().unwrap().is_empty());
            for flip in [false, true] {
                for fault in [false, true] {
                    for label in 0..8 {
                        let m = [label & 1 != 0, label & 2 != 0, label & 4 != 0];
                        let wanted = norm(&oracle_history(dense.clone(), flip, fault, m));
                        let groups = [
                            vec![m[0]],
                            vec![m[0]],
                            vec![m[1], m[0]],
                            vec![m[2], !m[2], m[0] ^ m[1]],
                        ];
                        let faults = [(0, flip), (1, fault)]
                            .into_iter()
                            .filter_map(|(site, yes)| {
                                yes.then_some(FaultSelection {
                                    site,
                                    alternative: 0,
                                })
                            })
                            .collect::<Vec<_>>();
                        for (index, program) in programs.iter().enumerate() {
                            seen.lock().unwrap().clear();
                            let result =
                                program.resolve_prescribed_history(&request, &faults, &groups, &a);
                            if wanted.is_zero() {
                                assert!(matches!(result, Err(RuntimeError::InvalidHistory { .. })));
                                continue;
                            }
                            let history = result.unwrap();
                            assert_eq!(history.history_mass(), &wanted);
                            let sign = if m[0] ^ m[1] { 1 } else { -1 };
                            let p0 = a
                                .scalar([2.into(), sign.into(), 0.into(), (-sign).into()], -2)
                                .unwrap();
                            let last = if m[2] {
                                a.subtract(&ExactScalar::one(), &p0).unwrap()
                            } else {
                                p0
                            };
                            assert_eq!(
                                wanted,
                                a.multiply(&last, &a.omega(0, if m[0] { -4 } else { -2 }).unwrap())
                                    .unwrap()
                            );
                            assert_eq!(history.requested_values(), groups[3]);
                            assert_eq!(
                                history.controller_calls(),
                                [
                                    ControllerCall {
                                        name: "copy".into(),
                                        call: 0,
                                        inputs: vec![m[0]],
                                        outputs: vec![m[0]]
                                    },
                                    ControllerCall {
                                        name: "xor".into(),
                                        call: 1,
                                        inputs: vec![m[1], m[0]],
                                        outputs: vec![m[1] ^ m[0]]
                                    },
                                ]
                            );
                            assert_eq!(*seen.lock().unwrap(), [vec![m[0]], vec![m[1], m[0]]]);
                            assert_eq!(history.entropy_words(), (0, 0));
                            assert_eq!(
                                history.network_compilations(),
                                if index == 0 { 0 } else { 5 }
                            );
                            assert_eq!(history.history_queries(), 4);
                            assert_eq!(program.reference_compilations(), 1);
                            let accepted = history
                                .instrument()
                                .acceptance(history.faults(), TERMS, &a)
                                .unwrap()
                                .value;
                            assert_eq!(
                                accepted,
                                if !m[2] && m[0] == m[1] {
                                    wanted.clone()
                                } else {
                                    ExactScalar::zero()
                                }
                            );
                            let report_weight = num_rational::BigRational::new(
                                if flip { 1.into() } else { 6.into() },
                                7.into(),
                            );
                            let active_weight = if m[0] {
                                num_rational::BigRational::new(
                                    if fault { 1.into() } else { 2.into() },
                                    3.into(),
                                )
                            } else {
                                num_rational::BigRational::from_integer(1.into())
                            };
                            assert_eq!(*history.noise_probability(), report_weight * active_weight);
                        }
                    }
                }
            }
            seen.lock().unwrap().clear();
        }
    }
    assert_eq!(input, original);
}

#[test]
fn initialized_noise_proposals_preserve_seeded_entropy_and_own_the_input() {
    let a = ScalarArithmetic::default();
    let (registry, seen) = registry();
    let source = causal_source(&registry);
    let (input, _) = bell_input();
    let sampling = NoiseSampling::Importance {
        proposal_scale: num_rational::BigRational::from_integer(2.into()),
    };
    let request = [record(2), record(0)];
    for lowering in LOWERINGS {
        for prepared in [false, true] {
            for history_sampler in [HistorySampler::ExactMarginal, HistorySampler::AffineSupport] {
                let programs = HISTORIES.map(|strategy| {
                    CausalProgram::new_stabilizer_with_sampling(
                        &source,
                        &input,
                        &registry,
                        RuntimeOptions {
                            strategy,
                            history_sampler,
                            compile: options(lowering, prepared),
                            ..Default::default()
                        },
                        &a,
                        &sampling,
                    )
                    .unwrap()
                });
                seen.lock().unwrap().clear();
                for seed in 0..16 {
                    let mut f = SplitMix64::new(seed);
                    let mut c = f;
                    let fixed = programs[0].resolve(&request, &mut f, &a).unwrap();
                    let fixed_log = seen.lock().unwrap().clone();
                    seen.lock().unwrap().clear();
                    let cold = programs[1].resolve(&request, &mut c, &a).unwrap();
                    assert_eq!(fixed_log, *seen.lock().unwrap());
                    seen.lock().unwrap().clear();
                    assert_eq!(f.state(), c.state());
                    assert_eq!(fixed.entropy_words(), cold.entropy_words());
                    assert_eq!(fixed.history_mass(), cold.history_mass());
                    assert_eq!(fixed.history_proposal_mass(), cold.history_proposal_mass());
                    assert_eq!(fixed.noise_probability(), cold.noise_probability());
                    assert_eq!(
                        fixed.noise_likelihood_ratio(),
                        cold.noise_likelihood_ratio()
                    );
                    assert_eq!(fixed.controller_calls(), cold.controller_calls());
                    assert_eq!(fixed.requested_values(), cold.requested_values());
                    assert_eq!(fixed.faults(), cold.faults());
                    assert_eq!(fixed.network_compilations(), 0);
                    assert_eq!(cold.network_compilations(), 5);
                    assert_eq!(
                        fixed
                            .instrument()
                            .acceptance(fixed.faults(), TERMS, &a)
                            .unwrap()
                            .value,
                        cold.instrument()
                            .acceptance(cold.faults(), TERMS, &a)
                            .unwrap()
                            .value
                    );
                }
            }
        }
    }
    let owned = HISTORIES.map(|strategy| {
        CausalProgram::new_stabilizer(
            &source,
            &input,
            &registry,
            RuntimeOptions {
                strategy,
                ..Default::default()
            },
            &a,
        )
        .unwrap()
    });
    drop(source);
    drop(input);
    for program in owned {
        assert!(
            !program
                .resolve(&request, &mut SplitMix64::new(19), &a)
                .unwrap()
                .history_mass()
                .is_zero()
        );
    }
}

#[test]
fn initialized_wide_records_resets_and_raw_limits_preserve_physical_wire_order() {
    let a = ScalarArithmetic::default();
    let source = parse(
        "I 128\nM 0 64 128\nR 64\nM 64",
        &ControllerRegistry::default(),
    );
    let input = AffineKet::new(
        BitVector::from_indices(129, [64]).unwrap(),
        vec![BitVector::from_indices(129, [0, 64, 128]).unwrap()],
        QuadraticForm::new(1, vec![3], vec![bits(1, 0)]).unwrap(),
        a.omega(5, -1).unwrap(),
    )
    .unwrap();
    let original = input.clone();
    let rows = [record(0), record(1), record(2), record(3)];
    for strategy in LOWERINGS {
        for prepared in [false, true] {
            let mut config = options(strategy, prepared);
            config.max_path_bits = 1;
            let program = compile_affine_stabilizer(&source, &input, &config, &a).unwrap();
            assert_eq!(program.compilation().raw_path_variables, 1);
            assert_eq!(program.num_qubits(), 129);
            assert_eq!(program.coherent_qubits(), 134);
            for outcome in 0..16 {
                let wanted = if outcome == 2 || outcome == 5 {
                    a.omega(0, -2).unwrap()
                } else {
                    ExactScalar::zero()
                };
                assert_eq!(
                    program
                        .reported_probability(&rows, &bits(4, outcome), &[], TERMS, &a)
                        .unwrap()
                        .value,
                    wanted
                );
            }
            config.max_path_bits = 0;
            assert!(matches!(
                compile_affine_stabilizer(&source, &input, &config, &a),
                Err(InstrumentError::Unitary(UnitaryError::RawPathLimit {
                    limit: 0,
                    required: 1
                }))
            ));
            config.max_path_bits = 1;
            config.max_coherent_qubits = 129;
            assert!(matches!(
                compile_affine_stabilizer(&source, &input, &config, &a),
                Err(InstrumentError::ResourceLimit {
                    option: "max_coherent_qubits",
                    limit: 129,
                    required: 130
                })
            ));
            assert_eq!(
                program.acceptance(&[], TERMS, &a).unwrap().value,
                ExactScalar::one()
            );
        }
    }
    assert_eq!(input, original);
}

fn raw(columns: Vec<BitVector>, coefficient: ExactScalar) -> AffineKet {
    let n = columns.len();
    AffineKet::new(
        bits(3, 4),
        columns,
        QuadraticForm::new(0, vec![0; n], vec![bits(n, 0); n]).unwrap(),
        coefficient,
    )
    .unwrap()
}

#[test]
fn invalid_initial_states_and_resource_errors_leave_callbacks_inputs_and_references_untouched() {
    let a = ScalarArithmetic::default();
    let (registry, seen) = registry();
    let source = parse(
        "M 0\nDETECTOR rec[-1]\nCONTROL[copy] det[0] -> ctrl[0]\nCX ctrl[0] 1\nI 2",
        &registry,
    );
    let affine = parse("I 2\nM 0", &ControllerRegistry::default());
    let empty = parse("I 2", &ControllerRegistry::default());
    let (valid, _) = bell_input();
    let dependent = raw(vec![bits(3, 0)], a.omega(0, -2).unwrap());
    assert_eq!(dependent.norm_squared(&a).unwrap(), ExactScalar::one());
    let invalid = [
        (
            CliffordCircuit::computational(bits(2, 0)).unwrap(),
            UnitaryError::DimensionMismatch,
        ),
        (
            raw(vec![], ExactScalar::zero()),
            UnitaryError::ZeroStabilizerInput,
        ),
        (dependent, UnitaryError::DependentStabilizerInput),
        (
            raw(vec![bits(3, 1)], ExactScalar::one()),
            UnitaryError::UnnormalizedStabilizerInput,
        ),
        (
            raw(vec![], a.omega(0, -2).unwrap()),
            UnitaryError::UnnormalizedStabilizerInput,
        ),
        (
            raw(
                vec![],
                a.scalar([1.into(), 0.into(), 0.into(), 0.into()], i64::MAX)
                    .unwrap(),
            ),
            UnitaryError::Scalar(ScalarError::ExponentOverflow),
        ),
    ];
    for lowering in LOWERINGS {
        for strategy in HISTORIES {
            let config = RuntimeOptions {
                strategy,
                compile: options(lowering, true),
                ..Default::default()
            };
            let prior =
                CausalProgram::new_stabilizer(&source, &valid, &registry, config, &a).unwrap();
            assert!(seen.lock().unwrap().is_empty());
            for (input, error) in &invalid {
                let before = input.clone();
                assert_eq!(
                    compile_affine_stabilizer(&affine, input, &config.compile, &a).err(),
                    Some(InstrumentError::Unitary(error.clone()))
                );
                for result in [
                    CausalProgram::new_stabilizer(&source, input, &registry, config, &a),
                    CausalProgram::new_stabilizer_with_sampling(
                        &source,
                        input,
                        &registry,
                        config,
                        &a,
                        &NoiseSampling::Physical,
                    ),
                ] {
                    assert!(
                        matches!(result,Err(RuntimeError::Instrument(InstrumentError::Unitary(ref actual))) if actual==error)
                    );
                }
                assert_eq!(*input, before);
                assert!(seen.lock().unwrap().is_empty());
            }
            let mut bounded = config;
            bounded.compile.max_path_bits = 0;
            assert!(matches!(
                compile_affine_stabilizer(&empty, &valid, &bounded.compile, &a),
                Err(InstrumentError::Unitary(UnitaryError::RawPathLimit {
                    limit: 0,
                    required: 1
                }))
            ));
            assert!(matches!(
                CausalProgram::new_stabilizer(&source, &valid, &registry, bounded, &a),
                Err(RuntimeError::Instrument(InstrumentError::Unitary(
                    UnitaryError::RawPathLimit { limit: 0, .. }
                )))
            ));
            bounded = config;
            bounded.compile.max_coherent_qubits = 3;
            assert!(matches!(
                CausalProgram::new_stabilizer(&source, &valid, &registry, bounded, &a),
                Err(RuntimeError::Instrument(InstrumentError::ResourceLimit {
                    option: "max_coherent_qubits",
                    ..
                }))
            ));
            let oversized = raw(vec![], a.integer(3).unwrap());
            let tiny = ScalarArithmetic::new(1).unwrap();
            assert!(matches!(
                CausalProgram::new_stabilizer(&source, &oversized, &registry, config, &tiny),
                Err(RuntimeError::Instrument(InstrumentError::Unitary(
                    UnitaryError::Scalar(ScalarError::CoefficientLimit { .. })
                )))
            ));
            assert!(seen.lock().unwrap().is_empty());
            let history = prior
                .resolve_prescribed_history(&[record(0)], &[], &[vec![true], vec![true]], &a)
                .unwrap();
            assert_eq!(history.history_mass(), &a.omega(0, -2).unwrap());
            assert_eq!(*seen.lock().unwrap(), [vec![true]]);
            seen.lock().unwrap().clear();
        }
    }
}

#[test]
fn legacy_zero_constructors_agree_with_explicit_zero_inputs_and_proposals() {
    let a = ScalarArithmetic::default();
    let (registry, seen) = registry();
    let source = causal_source(&registry);
    let zero = CliffordCircuit::computational(bits(3, 0)).unwrap();
    let affine = parse("H 0\nT 0\nH 0\nM(1/7) 0\nR 1\nM 1\nI 2", &registry);
    let request = [record(0), record(1)];
    for lowering in LOWERINGS {
        for prepared in [false, true] {
            let config = options(lowering, prepared);
            let old = compile_affine(&affine, &config, &a).unwrap();
            let new = compile_affine_stabilizer(&affine, &zero, &config, &a).unwrap();
            assert_eq!(old.compilation(), new.compilation());
            for label in 0..4 {
                assert_eq!(
                    old.reported_probability(&request, &bits(2, label), &[], TERMS, &a)
                        .unwrap()
                        .value,
                    new.reported_probability(&request, &bits(2, label), &[], TERMS, &a)
                        .unwrap()
                        .value
                );
            }
            for strategy in HISTORIES {
                let runtime = RuntimeOptions {
                    strategy,
                    compile: config,
                    ..Default::default()
                };
                for sampling in [
                    NoiseSampling::Physical,
                    NoiseSampling::Importance {
                        proposal_scale: num_rational::BigRational::from_integer(2.into()),
                    },
                ] {
                    let old = CausalProgram::new_with_sampling(
                        &source, &registry, runtime, &a, &sampling,
                    )
                    .unwrap();
                    let new = CausalProgram::new_stabilizer_with_sampling(
                        &source, &zero, &registry, runtime, &a, &sampling,
                    )
                    .unwrap();
                    assert_eq!(old.compilation(), new.compilation());
                    for seed in 0..4 {
                        let mut f = SplitMix64::new(seed);
                        let mut c = f;
                        let old = old.resolve(&request, &mut f, &a).unwrap();
                        let new = new.resolve(&request, &mut c, &a).unwrap();
                        assert_eq!(old.history_mass(), new.history_mass());
                        assert_eq!(old.controller_calls(), new.controller_calls());
                        assert_eq!(old.requested_values(), new.requested_values());
                        assert_eq!(old.entropy_words(), new.entropy_words());
                        assert_eq!(f.state(), c.state());
                        assert_eq!(old.network_compilations(), new.network_compilations());
                        assert_eq!(old.noise_probability(), new.noise_probability());
                        assert_eq!(old.noise_likelihood_ratio(), new.noise_likelihood_ratio());
                    }
                }
            }
            seen.lock().unwrap().clear();
        }
    }
}

#[test]
fn causal_quadratic_input_phases_agree_with_complete_state_projectors() {
    let a = ScalarArithmetic::default();
    let (registry, seen) = registry();
    let source = causal_source(&registry);
    let (input, dense) = complex_input();
    let request = [record(2), parity(&[2], true), parity(&[0, 1], false)];
    for lowering in LOWERINGS {
        for strategy in HISTORIES {
            let program = CausalProgram::new_stabilizer(
                &source,
                &input,
                &registry,
                RuntimeOptions {
                    strategy,
                    compile: options(lowering, true),
                    ..Default::default()
                },
                &a,
            )
            .unwrap();
            for flip in [false, true] {
                for fault in [false, true] {
                    let faults = [(0, flip), (1, fault)]
                        .into_iter()
                        .filter_map(|(site, yes)| {
                            yes.then_some(FaultSelection {
                                site,
                                alternative: 0,
                            })
                        })
                        .collect::<Vec<_>>();
                    let mut total = ExactScalar::zero();
                    for label in 0..8 {
                        let m = [label & 1 != 0, label & 2 != 0, label & 4 != 0];
                        let state = oracle_history(dense.clone(), flip, fault, m);
                        let wanted = norm(&state);
                        let groups = [
                            vec![m[0]],
                            vec![m[0]],
                            vec![m[1], m[0]],
                            vec![m[2], !m[2], m[0] ^ m[1]],
                        ];
                        let result =
                            program.resolve_prescribed_history(&request, &faults, &groups, &a);
                        if wanted.is_zero() {
                            assert!(matches!(result, Err(RuntimeError::InvalidHistory { .. })));
                        } else {
                            let history = result.unwrap();
                            assert_eq!(*history.history_mass(), wanted);
                            assert_eq!(history.controller_calls()[0].inputs, [m[0]]);
                            assert_eq!(history.controller_calls()[1].inputs, [m[1], m[0]]);
                            assert_eq!(history.requested_values(), groups[3]);
                            assert_eq!(
                                history
                                    .instrument()
                                    .acceptance(history.faults(), TERMS, &a)
                                    .unwrap()
                                    .value,
                                if !m[2] && m[0] == m[1] {
                                    wanted.clone()
                                } else {
                                    ExactScalar::zero()
                                }
                            );
                            total = a.add(&total, &wanted).unwrap();
                        }
                    }
                    assert_eq!(total, ExactScalar::one());
                }
            }
            seen.lock().unwrap().clear();
        }
    }
}

#[test]
fn zero_width_global_phase_inputs_are_admitted_by_instruments_and_both_history_strategies() {
    let a = ScalarArithmetic::default();
    let registry = ControllerRegistry::default();
    let source = parse("", &registry);
    let input = AffineKet::new(
        bits(0, 0),
        vec![],
        QuadraticForm::new(3, vec![], vec![]).unwrap(),
        a.omega(5, 0).unwrap(),
    )
    .unwrap();
    for lowering in LOWERINGS {
        let config = CompileOptions {
            compiler_strategy: lowering,
            max_path_bits: 0,
            max_coherent_qubits: 0,
            ..Default::default()
        };
        let instrument = compile_affine_stabilizer(&source, &input, &config, &a).unwrap();
        assert_eq!(
            instrument.acceptance(&[], 1, &a).unwrap().value,
            ExactScalar::one()
        );
        for strategy in HISTORIES {
            let program = CausalProgram::new_stabilizer(
                &source,
                &input,
                &registry,
                RuntimeOptions {
                    strategy,
                    compile: config,
                    ..Default::default()
                },
                &a,
            )
            .unwrap();
            let history = program
                .resolve_prescribed_history(&[], &[], &[], &a)
                .unwrap();
            assert_eq!(history.history_mass(), &ExactScalar::one());
            assert_eq!(history.entropy_words(), (0, 0));
            assert_eq!(
                history.network_compilations(),
                if strategy == CausalStrategy::FixedTemplate {
                    0
                } else {
                    1
                }
            );
        }
    }
}
