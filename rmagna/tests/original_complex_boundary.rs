// SPDX-License-Identifier: MPL-2.0
// Complete original boundary scenarios; full license in
// fixtures/original_many_record_input.LICENSE. Independent pinned Native literal
// tables, not the Rust service's prefix queries, define the branch-sum oracle.
use rmagna::{
    clifford::{CliffordCircuit, CliffordGate},
    gf2::BitVector,
    instrument::*,
    noise::{RandomSource, SplitMix64},
    pauli::PauliWord,
    records::bernoulli_ratio,
    scalar::{ExactScalar, ScalarArithmetic},
    unitary::CompilerStrategy,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};
type Calls = Arc<Mutex<Vec<(String, Vec<(String, bool)>)>>>;

fn initial(n: usize) -> rmagna::stabilizer::AffineKet {
    CliffordCircuit::new(n, (0..n).map(CliffordGate::H))
        .unwrap()
        .evolve(
            &CliffordCircuit::computational(BitVector::zeros(n).unwrap()).unwrap(),
            &ScalarArithmetic::default(),
        )
        .unwrap()
        .unwrap()
}
fn op(id: &str, gate: OriginalGate) -> OriginalOperation {
    OriginalOperation {
        id: id.into(),
        gate,
    }
}
fn measure(id: &str, wire: i64, pauli: OriginalPauli) -> OriginalOperation {
    op(
        id,
        OriginalGate::Measure {
            factors: vec![(wire, pauli)],
            inverted: false,
            record: id.into(),
        },
    )
}
fn task(
    n: usize,
    rows: &[&[&str]],
    outcomes: &[bool],
    words: &[(usize, usize)],
) -> OriginalBoundaryTask {
    OriginalBoundaryTask {
        postselection: rows
            .iter()
            .zip(outcomes)
            .map(|(names, &value)| OriginalRecordEvent {
                parity: OriginalRecordParity {
                    records: names.iter().map(|s| s.to_string()).collect(),
                    inverted: false,
                },
                value,
            })
            .collect(),
        observables: words
            .iter()
            .map(|&(x, z)| {
                PauliWord::new(
                    BitVector::from_indices(n, (0..n).filter(|j| x & (1 << j) != 0)).unwrap(),
                    BitVector::from_indices(n, (0..n).filter(|j| z & (1 << j) != 0)).unwrap(),
                    2 * ((x & z).count_ones() % 2) as u8,
                )
                .unwrap()
            })
            .collect(),
    }
}
fn fixture() -> Value {
    let raw = include_bytes!("data/original-complex-boundary.json");
    assert_eq!(
        format!("{:x}", Sha256::digest(raw)),
        "eb3aefe215c88c2b4bbdd40286bde3e812956d6e74a3360f4d1e0ca73dff67c1"
    );
    let data: Value = serde_json::from_slice(raw).unwrap();
    let manifest: Value = serde_json::from_str(include_str!(
        "../../benchmarks/magna_reproduction/reference.json"
    ))
    .unwrap();
    assert_eq!(data["upstream_commit"], manifest["upstream"]["commit"]);
    assert_eq!(
        data["reference_sha256"],
        format!(
            "{:x}",
            Sha256::digest(include_bytes!(
                "../../benchmarks/magna_reproduction/reference.json"
            ))
        )
    );
    assert_eq!(
        data["producer_sha256"],
        format!(
            "{:x}",
            Sha256::digest(include_bytes!(
                "../../benchmarks/magna_reproduction/original_complex_boundary.py"
            ))
        )
    );
    for (name, hash) in data["source_sha256"].as_object().unwrap() {
        assert_eq!(*hash, manifest["upstream"]["files"][name]);
    }
    assert_eq!(data["original_methods_passed"], 5);
    assert_eq!(data["tables"].as_array().unwrap().len(), 7);
    assert_eq!(data["observations"].as_array().unwrap().len(), 50);
    data
}
fn scalar(value: &Value) -> ExactScalar {
    ScalarArithmetic::default()
        .scalar(
            std::array::from_fn(|j| value["coefficients"][j].to_string().parse().unwrap()),
            value["power"].as_i64().unwrap(),
        )
        .unwrap()
}
fn reports(row: &Value) -> BTreeMap<String, bool> {
    row["reports"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(r, v)| (r.clone(), v.as_u64().unwrap() != 0))
        .collect()
}
struct CountRng {
    random: SplitMix64,
    words: usize,
}
impl CountRng {
    fn new(seed: u64) -> Self {
        Self {
            random: SplitMix64::new(seed),
            words: 0,
        }
    }
}
impl RandomSource for CountRng {
    fn next_u64(&mut self) -> u64 {
        self.words += 1;
        self.random.next_u64()
    }
}
fn oracle_reports(
    table: &Value,
    required: &[String],
    random: &mut CountRng,
) -> BTreeMap<String, bool> {
    let a = ScalarArithmetic::default();
    let mut sampled = BTreeMap::new();
    for name in required {
        let mut yes = ExactScalar::zero();
        let mut total = ExactScalar::zero();
        for row in table["rows"].as_array().unwrap() {
            let values = reports(row);
            if sampled.iter().any(|(r, v)| values.get(r) != Some(v)) {
                continue;
            }
            let p = scalar(&row["mass"]);
            total = a.add(&total, &p).unwrap();
            if values[name] {
                yes = a.add(&yes, &p).unwrap();
            }
        }
        sampled.insert(
            name.clone(),
            bernoulli_ratio(&yes, &total, random, 1024, &a)
                .unwrap()
                .selected,
        );
    }
    sampled
}
fn assert_draw(
    draw: &OriginalFeedbackBoundaryDraw,
    table: &Value,
    task: &OriginalBoundaryTask,
    indices: &[usize],
) {
    let a = ScalarArithmetic::default();
    let mut mass = ExactScalar::zero();
    let mut acceptance = ExactScalar::zero();
    let mut moments = vec![ExactScalar::zero(); indices.len()];
    for row in table["rows"].as_array().unwrap() {
        let values = reports(row);
        if draw.reports.iter().any(|(r, v)| values.get(r) != Some(v)) {
            continue;
        }
        mass = a.add(&mass, &scalar(&row["mass"])).unwrap();
        if task.postselection.iter().any(|e| {
            e.parity
                .records
                .iter()
                .fold(e.parity.inverted, |b, r| b ^ values[r])
                != e.value
        }) {
            continue;
        }
        acceptance = a.add(&acceptance, &scalar(&row["mass"])).unwrap();
        for (m, &j) in moments.iter_mut().zip(indices) {
            *m = a.add(m, &scalar(&row["moments"][j])).unwrap();
        }
    }
    assert_eq!(draw.mass, mass);
    assert_eq!(draw.acceptance, acceptance);
    assert_eq!(draw.moments, moments);
}
fn assert_true(draw: &OriginalFeedbackBoundaryDraw, input: &OriginalTraceAssignment) {
    assert_eq!(draw.true_values.len(), draw.reports.len());
    for (name, &value) in &draw.reports {
        assert_eq!(
            draw.true_values[name],
            value ^ (input.report_flips.iter().filter(|r| *r == name).count() % 2 != 0)
        );
    }
}
fn callback(
    id: &str,
    parents: &[&str],
    calls: &Calls,
    f: impl Fn(&OriginalControlInputs) -> Vec<OriginalGate> + Send + Sync + 'static,
) -> OriginalControl {
    let name = id.to_owned();
    let observed = Arc::clone(calls);
    OriginalControl {
        operation: id.into(),
        parents: parents.iter().map(|s| s.to_string()).collect(),
        version: "source-v1".into(),
        callback: Arc::new(move |inputs| {
            observed
                .lock()
                .unwrap()
                .push((name.clone(), inputs.entries().to_vec()));
            Ok(f(inputs))
        }),
    }
}
fn program(
    source: OriginalInstrument,
    families: BTreeMap<String, OriginalPhaseFeedback>,
    compiler: CompilerStrategy,
) -> OriginalFeedbackBoundaryProgram {
    OriginalFeedbackBoundaryProgram::new(
        source,
        &families,
        OriginalFeedbackBoundaryOptions {
            compiler,
            ..Default::default()
        },
        ScalarArithmetic::default(),
    )
    .unwrap()
}
fn phase_source(coefficient: i32) -> OriginalInstrument {
    let limits = ControlRuleLimits::default();
    let record = ControlExpression::record("a".into(), limits).unwrap();
    let multiple = ControlExpression::compose(
        ControlOperator::Multiply,
        vec![ControlExpression::constant(coefficient), record],
        limits,
    )
    .unwrap();
    let exponent = ControlExpression::compose(
        ControlOperator::Add,
        vec![ControlExpression::constant(2), multiple],
        limits,
    )
    .unwrap();
    let gate = ControlGate::new(
        ControlGateKind::Phase,
        vec![1],
        Some(exponent),
        ControlExpression::constant(1),
        limits,
    )
    .unwrap();
    let rule = ControlRule::new(vec![gate], limits).unwrap();
    OriginalInstrument::new_with_declared_controls(
        vec![0, 1],
        vec![1, 0],
        initial(2),
        vec![
            measure("a", 0, OriginalPauli::Z),
            op("c", OriginalGate::Control),
            measure("b", 1, OriginalPauli::X),
        ],
        vec![
            OriginalDeclaredControl::new("c".into(), vec!["a".into()], "arithmetic".into(), rule)
                .unwrap(),
        ],
        Default::default(),
        &ScalarArithmetic::default(),
    )
    .unwrap()
}

#[test]
fn all_original_even_and_odd_phase_cases_keep_routes_masses_moments_and_rng() {
    let data = fixture();
    let task = task(2, &[&["b"]], &[false], &[(1, 0), (1, 1), (0, 1)]);
    for compiler in [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ] {
        for (index, coefficient) in [2, 4, 1].into_iter().enumerate() {
            let program = program(phase_source(coefficient), BTreeMap::new(), compiler);
            assert_eq!(
                program.route(),
                if coefficient == 1 {
                    OriginalFeedbackBoundaryRoute::ComplexFeedback
                } else {
                    OriginalFeedbackBoundaryRoute::Direct
                }
            );
            let sampler = program.prepare_task(&task).unwrap();
            for seed in 0..3 {
                let mut random = CountRng::new(seed);
                let mut reference = CountRng::new(seed);
                let expected = oracle_reports(
                    &data["tables"][index],
                    program.required_reports(),
                    &mut reference,
                );
                let draw = sampler
                    .sample(
                        &OriginalTraceAssignment {
                            report_flips: vec!["a".into()],
                            ..Default::default()
                        },
                        &mut random,
                    )
                    .unwrap();
                assert_eq!(draw.reports, expected);
                assert_draw(&draw, &data["tables"][index], &task, &[0, 1, 2]);
                assert_eq!(
                    (random.words, random.random.state()),
                    (reference.words, reference.random.state())
                );
                assert_eq!(draw.statistics.entropy_words, random.words);
            }
        }
    }
    eprintln!(
        "all3 original coefficients x3 draws x2 compilers; direct-even/complex-odd routing and independent Native branch sums"
    );
}

fn complex_source(calls: &Calls) -> OriginalInstrument {
    let controls = vec![
        callback("c1", &["a", "b"], calls, |r| {
            vec![OriginalGate::Phase {
                wires: vec![2],
                turns: 2 * i64::from(r.get("a").unwrap() & r.get("b").unwrap()),
            }]
        }),
        callback("c2", &["a", "s"], calls, |r| {
            vec![if r.get("s").unwrap() ^ r.get("a").unwrap() {
                OriginalGate::Pauli {
                    wire: 0,
                    pauli: OriginalPauli::X,
                }
            } else {
                OriginalGate::Identity(vec![0])
            }]
        }),
    ];
    OriginalInstrument::new(
        vec![0, 1, 2],
        vec![0, 2],
        initial(3),
        vec![
            op(
                "t",
                OriginalGate::Phase {
                    wires: vec![0],
                    turns: 1,
                },
            ),
            measure("a", 0, OriginalPauli::X),
            measure("b", 1, OriginalPauli::Z),
            op("c1", OriginalGate::Control),
            measure("s", 2, OriginalPauli::X),
            op("c2", OriginalGate::Control),
            op(
                "j",
                OriginalGate::Destructive {
                    wire: 1,
                    basis: ReadoutBasis::X,
                    record: "junk".into(),
                },
            ),
        ],
        controls,
        Default::default(),
        &ScalarArithmetic::default(),
    )
    .unwrap()
}
#[test]
fn two_original_callbacks_sample_causally_and_integrate_future_postselection_and_junk() {
    let data = fixture();
    let task = task(2, &[&["s", "junk"]], &[false], &[(1, 0), (1, 1), (0, 1)]);
    for compiler in [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ] {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let program = program(
            complex_source(&calls),
            BTreeMap::from([
                (
                    "c1".into(),
                    OriginalPhaseFeedback::new(&[2], &[], &[]).unwrap(),
                ),
                (
                    "c2".into(),
                    OriginalPhaseFeedback::new(&[0], &[], &[]).unwrap(),
                ),
            ]),
            compiler,
        );
        assert_eq!(program.required_reports(), ["a", "b", "s"]);
        let sampler = program.prepare_task(&task).unwrap();
        for (case, input) in [
            OriginalTraceAssignment::default(),
            OriginalTraceAssignment {
                faults: vec![OriginalFault {
                    after: "c1".into(),
                    wire: 2,
                    pauli: OriginalPauli::Y,
                }],
                report_flips: vec!["a".into(), "s".into()],
            },
        ]
        .into_iter()
        .enumerate()
        {
            for seed in 0..12 {
                calls.lock().unwrap().clear();
                let mut random = CountRng::new(seed);
                let mut reference = CountRng::new(seed);
                let expected = oracle_reports(
                    &data["tables"][3 + case],
                    program.required_reports(),
                    &mut reference,
                );
                let draw = sampler.sample(&input, &mut random).unwrap();
                assert_true(&draw, &input);
                assert_draw(&draw, &data["tables"][3 + case], &task, &[0, 1, 2]);
                assert_eq!(draw.reports, expected);
                assert!(!draw.reports.contains_key("junk"));
                assert_eq!(draw.statistics.controls_executed, 2);
                let seen = calls.lock().unwrap();
                assert_eq!(seen.len(), 2);
                assert_eq!(
                    seen[0],
                    (
                        "c1".into(),
                        vec![("a".into(), expected["a"]), ("b".into(), expected["b"])]
                    )
                );
                assert_eq!(
                    seen[1],
                    (
                        "c2".into(),
                        vec![("a".into(), expected["a"]), ("s".into(), expected["s"])]
                    )
                );
                assert_eq!(
                    (random.words, random.random.state()),
                    (reference.words, reference.random.state())
                );
                assert_eq!(draw.statistics.entropy_words, random.words);
            }
        }
    }
    eprintln!(
        "original2 callbacks x2 fault/flip cases x12 draws x2 compilers; all Native complete branch sums and causal RNG protocols"
    );
}

fn single_source(calls: &Calls, causal: bool) -> OriginalInstrument {
    let mut ops = vec![op(
        "t1",
        OriginalGate::Phase {
            wires: vec![0],
            turns: 1,
        },
    )];
    if !causal {
        ops.extend([
            op("h", OriginalGate::H(0)),
            op(
                "t2",
                OriginalGate::Phase {
                    wires: vec![0],
                    turns: 1,
                },
            ),
        ]);
    }
    ops.extend([
        measure("a", 0, OriginalPauli::X),
        op("c", OriginalGate::Control),
    ]);
    if causal {
        ops.extend([
            measure("b", 1, OriginalPauli::X),
            op("drop", OriginalGate::Discard(0)),
        ]);
    }
    let control = callback("c", &["a"], calls, move |r| {
        if causal {
            vec![OriginalGate::Phase {
                wires: vec![1],
                turns: 2 * i64::from(r.get("a").unwrap()),
            }]
        } else {
            vec![if r.get("a").unwrap() {
                OriginalGate::Pauli {
                    wire: 1,
                    pauli: OriginalPauli::Z,
                }
            } else {
                OriginalGate::Identity(vec![1])
            }]
        }
    });
    OriginalInstrument::new(
        vec![0, 1],
        if causal { vec![1] } else { vec![1, 0] },
        initial(2),
        ops,
        vec![control],
        Default::default(),
        &ScalarArithmetic::default(),
    )
    .unwrap()
}
#[test]
fn original_noncommuting_and_reference_schedule_cases_use_unconditioned_global_prefixes() {
    let data = fixture();
    for compiler in [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ] {
        for causal in [false, true] {
            let calls = Arc::new(Mutex::new(Vec::new()));
            let program = program(
                single_source(&calls, causal),
                BTreeMap::from([(
                    "c".into(),
                    OriginalPhaseFeedback::new(&[1], &[], &[]).unwrap(),
                )]),
                compiler,
            );
            let task = if causal {
                task(1, &[&["b"]], &[true], &[(0, 1), (1, 0)])
            } else {
                task(2, &[], &[], &[(1, 0), (1, 1), (0, 1)])
            };
            let table = &data["tables"][if causal { 6 } else { 5 }];
            let sampler = program.prepare_task(&task).unwrap();
            let input = if causal {
                OriginalTraceAssignment {
                    faults: vec![OriginalFault {
                        after: "c".into(),
                        wire: 1,
                        pauli: OriginalPauli::Y,
                    }],
                    report_flips: vec!["a".into()],
                }
            } else {
                Default::default()
            };
            for seed in 0..if causal { 12 } else { 5 } {
                calls.lock().unwrap().clear();
                let mut random = CountRng::new(seed);
                let mut reference = CountRng::new(seed);
                let expected = oracle_reports(table, program.required_reports(), &mut reference);
                let draw = sampler.sample(&input, &mut random).unwrap();
                assert_true(&draw, &input);
                assert_eq!(draw.reports, expected);
                assert_draw(
                    &draw,
                    table,
                    &task,
                    if causal { &[2, 0] } else { &[0, 1, 2] },
                );
                assert_eq!(
                    *calls.lock().unwrap(),
                    vec![("c".into(), vec![("a".into(), expected["a"])])]
                );
                assert_eq!(
                    (random.words, random.random.state()),
                    (reference.words, reference.random.state())
                );
                assert_eq!(draw.statistics.controls_executed, 1);
            }
        }
    }
    eprintln!(
        "noncommuting T/H/T5 draws and complete causal-reference12 draws, both compilers; terminal masses/moments and exact caller RNG state"
    );
}

fn fixed_source(gates: Vec<OriginalGate>, calls: &Calls) -> OriginalInstrument {
    OriginalInstrument::new(
        vec![0],
        vec![0],
        initial(1),
        vec![op("c", OriginalGate::Control)],
        vec![callback("c", &[], calls, move |_| gates.clone())],
        Default::default(),
        &ScalarArithmetic::default(),
    )
    .unwrap()
}

#[test]
fn original_missing_family_constructor_and_out_of_family_runtime_fail_separately() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let source = fixed_source(vec![OriginalGate::H(0)], &calls);
    assert!(matches!(
        OriginalFeedbackBoundaryProgram::new(
            source.clone(),
            &BTreeMap::new(),
            Default::default(),
            Default::default()
        ),
        Err(OriginalComplexBoundaryError::UndeclaredFamily { .. })
    ));
    assert!(calls.lock().unwrap().is_empty());
    let program = program(
        source,
        BTreeMap::from([(
            "c".into(),
            OriginalPhaseFeedback::new(&[0], &[], &[]).unwrap(),
        )]),
        CompilerStrategy::ParityCopies,
    );
    let sampler = program
        .prepare_task(&OriginalBoundaryTask::default())
        .unwrap();
    let mut random = CountRng::new(1);
    assert!(matches!(
        sampler.sample(&OriginalTraceAssignment::default(), &mut random),
        Err(OriginalComplexBoundaryError::FeedbackMismatch { .. })
    ));
    assert_eq!(calls.lock().unwrap().len(), 1);
    assert_eq!(random.words, 0);
}

#[test]
fn phase_families_check_normalized_parity_and_nonlocal_axes_without_replaying_callbacks() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let even = OriginalPhaseFeedback::new(&[0], &[], &[]).unwrap();
    let task = task(1, &[], &[], &[(1, 0)]);
    let gates = vec![
        OriginalGate::Phase {
            wires: vec![0],
            turns: 1,
        },
        OriginalGate::Pauli {
            wire: 0,
            pauli: OriginalPauli::X,
        },
        OriginalGate::Phase {
            wires: vec![0],
            turns: 1,
        },
    ];
    let p = program(
        fixed_source(gates, &calls),
        BTreeMap::from([("c".into(), even.clone())]),
        CompilerStrategy::ParityCopies,
    );
    let draw = p
        .prepare_task(&task)
        .unwrap()
        .sample(&Default::default(), &mut CountRng::new(9))
        .unwrap();
    assert_eq!(draw.moments, [ExactScalar::one()]); // T X T = omega X.
    assert_eq!(draw.statistics.controls_executed, 1);
    assert_eq!(draw.statistics.emitted_operations, 3);
    let odd = OriginalPhaseFeedback::new(&[0], &[vec![0]], &[]).unwrap();
    let p = program(
        fixed_source(
            vec![OriginalGate::Phase {
                wires: vec![0],
                turns: 1,
            }],
            &calls,
        ),
        BTreeMap::from([("c".into(), odd)]),
        CompilerStrategy::ParityCopies,
    );
    let draw = p
        .prepare_task(&task)
        .unwrap()
        .sample(&Default::default(), &mut CountRng::new(9))
        .unwrap();
    assert_eq!(
        draw.moments,
        [ScalarArithmetic::default()
            .scalar([0.into(), 1.into(), 0.into(), (-1).into()], -1)
            .unwrap()]
    );
    for gates in [
        vec![OriginalGate::Phase {
            wires: vec![0],
            turns: 1,
        }],
        vec![OriginalGate::Identity(vec![1])],
    ] {
        let p = program(
            fixed_source(gates, &calls),
            BTreeMap::from([("c".into(), even.clone())]),
            CompilerStrategy::ParityCopies,
        );
        assert!(matches!(
            p.prepare_task(&task)
                .unwrap()
                .sample(&Default::default(), &mut CountRng::new(9)),
            Err(OriginalComplexBoundaryError::FeedbackMismatch { .. })
        ));
    }
    for (odd, even) in [
        (vec![vec![0, 0]], vec![]),
        (vec![vec![0]], vec![vec![0]]),
        (vec![vec![1]], vec![]),
    ] {
        assert!(OriginalPhaseFeedback::new(&[0], &odd, &even).is_err());
    }
    let source = OriginalInstrument::new(
        vec![0, 1],
        vec![0, 1],
        initial(2),
        vec![op("c", OriginalGate::Control)],
        vec![callback("c", &[], &calls, |_| {
            vec![OriginalGate::Phase {
                wires: vec![0, 1],
                turns: 1,
            }]
        })],
        Default::default(),
        &ScalarArithmetic::default(),
    )
    .unwrap();
    let p = program(
        source,
        BTreeMap::from([(
            "c".into(),
            OriginalPhaseFeedback::new(&[0, 1], &[vec![1, 0]], &[]).unwrap(),
        )]),
        CompilerStrategy::ParityCopies,
    );
    let two = crate::task(2, &[], &[], &[(1, 0)]);
    let draw = p
        .prepare_task(&two)
        .unwrap()
        .sample(&Default::default(), &mut CountRng::new(9))
        .unwrap();
    assert_eq!(
        draw.moments,
        [ScalarArithmetic::default()
            .scalar([0.into(), 1.into(), 0.into(), (-1).into()], -1)
            .unwrap()]
    );
}

struct FixedWord {
    words: usize,
}
impl RandomSource for FixedWord {
    fn next_u64(&mut self) -> u64 {
        self.words += 1;
        assert_eq!(self.words, 1);
        0xcccc_cccc_cccc_cccc
    }
}
#[test]
fn future_postselection_cannot_reweight_the_first_controller_input() {
    let data = fixture();
    let table = &data["tables"][6];
    let a = ScalarArithmetic::default();
    let mut yes = ExactScalar::zero();
    let mut total = ExactScalar::zero();
    let mut accepted_yes = ExactScalar::zero();
    let mut accepted_total = ExactScalar::zero();
    for row in table["rows"].as_array().unwrap() {
        let values = reports(row);
        let p = scalar(&row["mass"]);
        total = a.add(&total, &p).unwrap();
        if values["a"] {
            yes = a.add(&yes, &p).unwrap();
        }
        if values["b"] {
            accepted_total = a.add(&accepted_total, &p).unwrap();
            if values["a"] {
                accepted_yes = a.add(&accepted_yes, &p).unwrap();
            }
        }
    }
    let mut physical = FixedWord { words: 0 };
    let mut conditioned = FixedWord { words: 0 };
    assert!(
        bernoulli_ratio(&yes, &total, &mut physical, 1, &a)
            .unwrap()
            .selected
    );
    assert!(
        !bernoulli_ratio(&accepted_yes, &accepted_total, &mut conditioned, 1, &a)
            .unwrap()
            .selected,
        "independent full table must distinguish the postselected law"
    );
    let calls = Arc::new(Mutex::new(Vec::new()));
    let p = program(
        single_source(&calls, true),
        BTreeMap::from([(
            "c".into(),
            OriginalPhaseFeedback::new(&[1], &[], &[]).unwrap(),
        )]),
        CompilerStrategy::ParityCopies,
    );
    let selected = task(1, &[&["b"]], &[true], &[(0, 1), (1, 0)]);
    let input = OriginalTraceAssignment {
        faults: vec![OriginalFault {
            after: "c".into(),
            wire: 1,
            pauli: OriginalPauli::Y,
        }],
        report_flips: vec!["a".into()],
    };
    let draw = p
        .prepare_task(&selected)
        .unwrap()
        .sample(&input, &mut FixedWord { words: 0 })
        .unwrap();
    assert!(
        draw.reports["a"],
        "future postselection changed the earlier controller input"
    );
    assert_eq!(
        *calls.lock().unwrap(),
        vec![("c".into(), vec![("a".into(), true)])]
    );
    assert_draw(&draw, table, &selected, &[2, 0]);
}

#[test]
fn owned_draws_survive_program_drop_without_retaining_its_callback_owner() {
    let marker = Arc::new(());
    let weak = Arc::downgrade(&marker);
    let selected = task(1, &[&["b"]], &[true], &[(0, 1), (1, 0)]);
    let input = OriginalTraceAssignment {
        faults: vec![OriginalFault {
            after: "c".into(),
            wire: 1,
            pauli: OriginalPauli::Y,
        }],
        report_flips: vec!["a".into()],
    };
    let draw = {
        let control = OriginalControl {
            operation: "c".into(),
            parents: vec!["a".into()],
            version: "owned".into(),
            callback: Arc::new(move |r| {
                let _owner = &marker;
                Ok(vec![OriginalGate::Phase {
                    wires: vec![1],
                    turns: 2 * i64::from(r.get("a")?),
                }])
            }),
        };
        let source = OriginalInstrument::new(
            vec![0, 1],
            vec![1],
            initial(2),
            vec![
                op(
                    "t1",
                    OriginalGate::Phase {
                        wires: vec![0],
                        turns: 1,
                    },
                ),
                measure("a", 0, OriginalPauli::X),
                op("c", OriginalGate::Control),
                measure("b", 1, OriginalPauli::X),
                op("drop", OriginalGate::Discard(0)),
            ],
            vec![control],
            Default::default(),
            &ScalarArithmetic::default(),
        )
        .unwrap();
        let p = program(
            source,
            BTreeMap::from([(
                "c".into(),
                OriginalPhaseFeedback::new(&[1], &[], &[]).unwrap(),
            )]),
            CompilerStrategy::ParityCopies,
        );
        p.prepare_task(&selected)
            .unwrap()
            .sample(&input, &mut CountRng::new(19))
            .unwrap()
    };
    assert!(weak.upgrade().is_none());
    assert_draw(&draw, &fixture()["tables"][6], &selected, &[2, 0]);
    assert_true(&draw, &input);
}

#[test]
fn limits_and_invalid_shared_inputs_refuse_before_entropy_or_external_callbacks() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let families = BTreeMap::from([(
        "c".into(),
        OriginalPhaseFeedback::new(&[1], &[], &[]).unwrap(),
    )]);
    for options in [
        OriginalFeedbackBoundaryOptions {
            max_literal_records: 0,
            ..Default::default()
        },
        OriginalFeedbackBoundaryOptions {
            max_branch_compilations: 0,
            ..Default::default()
        },
        OriginalFeedbackBoundaryOptions {
            max_work: 0,
            ..Default::default()
        },
    ] {
        assert!(
            OriginalFeedbackBoundaryProgram::new(
                single_source(&calls, false),
                &families,
                options,
                Default::default()
            )
            .is_err()
        );
        assert!(calls.lock().unwrap().is_empty());
    }
    let p = OriginalFeedbackBoundaryProgram::new(
        single_source(&calls, false),
        &families,
        OriginalFeedbackBoundaryOptions {
            max_entropy_words: 0,
            ..Default::default()
        },
        Default::default(),
    )
    .unwrap();
    let sampler = p.prepare_task(&OriginalBoundaryTask::default()).unwrap();
    let mut random = CountRng::new(11);
    assert!(matches!(
        sampler.sample(&Default::default(), &mut random),
        Err(OriginalComplexBoundaryError::Sampling(_))
    ));
    assert_eq!(random.words, 0);
    assert!(calls.lock().unwrap().is_empty());
    for input in [
        OriginalTraceAssignment {
            report_flips: vec!["unknown".into()],
            ..Default::default()
        },
        OriginalTraceAssignment {
            faults: vec![OriginalFault {
                after: "c".into(),
                wire: 99,
                pauli: OriginalPauli::X,
            }],
            ..Default::default()
        },
    ] {
        assert!(sampler.sample(&input, &mut random).is_err());
        assert_eq!(random.words, 0);
        assert!(calls.lock().unwrap().is_empty());
    }
    let direct = program(
        phase_source(2),
        BTreeMap::new(),
        CompilerStrategy::ParityCopies,
    );
    let draw = direct
        .prepare_task(&OriginalBoundaryTask::default())
        .unwrap()
        .sample(&Default::default(), &mut random)
        .unwrap();
    assert_eq!(draw.mass, ExactScalar::one());
    assert_eq!(random.words, 0);
    assert_eq!(draw.statistics.scalar_queries, 1);
}

#[test]
fn future_selection_sectors_preserve_the_same_earlier_reports_and_rng() {
    let data = fixture();
    let calls = Arc::new(Mutex::new(Vec::new()));
    let program = program(
        complex_source(&calls),
        BTreeMap::from([
            (
                "c1".into(),
                OriginalPhaseFeedback::new(&[2], &[], &[]).unwrap(),
            ),
            (
                "c2".into(),
                OriginalPhaseFeedback::new(&[0], &[], &[]).unwrap(),
            ),
        ]),
        CompilerStrategy::ParityCopies,
    );
    let zero = task(2, &[&["s", "junk"]], &[false], &[(1, 0), (1, 1), (0, 1)]);
    let one = task(2, &[&["s", "junk"]], &[true], &[(1, 0), (1, 1), (0, 1)]);
    let left = program.prepare_task(&zero).unwrap();
    let right = program.prepare_task(&one).unwrap();
    for seed in 0..12 {
        let mut l = CountRng::new(seed);
        let mut r = CountRng::new(seed);
        let a = left.sample(&Default::default(), &mut l).unwrap();
        let b = right.sample(&Default::default(), &mut r).unwrap();
        assert_eq!(a.reports, b.reports);
        assert_eq!(a.true_values, b.true_values);
        assert_eq!((l.words, l.random.state()), (r.words, r.random.state()));
        assert_eq!(
            ScalarArithmetic::default()
                .add(&a.acceptance, &b.acceptance)
                .unwrap(),
            a.mass
        );
        assert_draw(&a, &data["tables"][3], &zero, &[0, 1, 2]);
        assert_draw(&b, &data["tables"][3], &one, &[0, 1, 2]);
    }
}
