#[path = "support/near_clifford_oracle.rs"]
mod oracle;

use oracle::DenseOracle;
use rand::{RngCore, SeedableRng, rngs::StdRng};
use rstim::ir::{PauliBasis, StimInstr, StimTarget};
use rstim::near_clifford::{CompiledNearCliffordExecutor, CompiledRotationArithmetic};
use std::collections::BTreeMap;

#[derive(Clone)]
struct Branch {
    state: DenseOracle,
    records: Vec<bool>,
    weight: f64,
}

fn pauli(state: &mut DenseOracle, q: usize, basis: usize) {
    match basis {
        0 => {}
        1 => state.x(q),
        2 => state.y(q),
        3 => state.z(q),
        _ => unreachable!(),
    }
}

// A classical readout channel copies the same conditional quantum state.
fn report(branch: Branch, physical: bool, inverted: bool, probability: f64) -> Vec<Branch> {
    assert!(probability.is_finite() && (0.0..=1.0).contains(&probability));
    [false, true]
        .into_iter()
        .filter_map(|flipped| {
            let weight = if flipped {
                probability
            } else {
                1. - probability
            };
            if weight == 0. {
                return None;
            }
            let mut next = branch.clone();
            next.weight *= weight;
            next.records.push(physical ^ inverted ^ flipped);
            Some(next)
        })
        .collect()
}

// Matrix-based physical channels, independent of all tableau/compiled internals.
fn evolve(instructions: &[StimInstr], mut branches: Vec<Branch>) -> Vec<Branch> {
    for instruction in instructions {
        let StimInstr::Op {
            name,
            args,
            targets,
            ..
        } = instruction
        else {
            let StimInstr::Repeat { count, body } = instruction else {
                unreachable!()
            };
            for _ in 0..*count {
                branches = evolve(body, branches);
            }
            continue;
        };
        if matches!(name.as_str(), "CX" | "CY" | "CZ" | "SWAP") {
            for pair in targets.chunks_exact(2) {
                let b = pair[1].qubit_index().unwrap() as usize;
                for branch in &mut branches {
                    match pair[0] {
                        StimTarget::Rec(offset) => {
                            if branch.records[(branch.records.len() as i32 + offset) as usize] {
                                pauli(
                                    &mut branch.state,
                                    b,
                                    match name.as_str() {
                                        "CX" => 1,
                                        "CY" => 2,
                                        _ => 3,
                                    },
                                );
                            }
                        }
                        StimTarget::Qubit(a) => match name.as_str() {
                            "CX" => branch.state.cx(a as usize, b),
                            "CZ" => branch.state.cz(a as usize, b),
                            "CY" => {
                                branch.state.s_dag(b);
                                branch.state.cx(a as usize, b);
                                branch.state.s(b);
                            }
                            "SWAP" => branch.state.swap(a as usize, b),
                            _ => unreachable!(),
                        },
                        _ => unreachable!(),
                    }
                }
            }
            continue;
        }
        if name == "MPP" {
            let readout = args.first().copied().unwrap_or(0.);
            let mut cursor = 0;
            while cursor < targets.len() {
                let mut factors = Vec::new();
                let mut inverted = false;
                loop {
                    let StimTarget::Pauli {
                        qubit,
                        basis,
                        inverted: factor_inverted,
                    } = targets[cursor]
                    else {
                        panic!("oracle expects an MPP factor");
                    };
                    factors.push((
                        qubit as usize,
                        match basis {
                            PauliBasis::X => 'X',
                            PauliBasis::Y => 'Y',
                            PauliBasis::Z => 'Z',
                        },
                    ));
                    inverted ^= factor_inverted;
                    cursor += 1;
                    if targets.get(cursor) != Some(&StimTarget::Combiner) {
                        break;
                    }
                    cursor += 1;
                }
                let mut next = Vec::new();
                for branch in branches {
                    for outcome in [false, true] {
                        let mut conditional = branch.clone();
                        let probability =
                            conditional.state.project_pauli_product(&factors, outcome);
                        if probability < 1e-14 {
                            continue;
                        }
                        conditional.weight *= probability;
                        next.extend(report(conditional, outcome, inverted, readout));
                    }
                }
                branches = next;
            }
            continue;
        }
        if name == "DEPOLARIZE2" {
            for pair in targets.chunks_exact(2) {
                branches = branches
                    .into_iter()
                    .flat_map(|branch| {
                        (0..16).filter_map(move |choice| {
                            let weight = if choice == 0 {
                                1. - args[0]
                            } else {
                                args[0] / 15.
                            };
                            if weight == 0. {
                                return None;
                            }
                            let mut next = branch.clone();
                            next.weight *= weight;
                            pauli(
                                &mut next.state,
                                pair[0].qubit_index().unwrap() as usize,
                                choice / 4,
                            );
                            pauli(
                                &mut next.state,
                                pair[1].qubit_index().unwrap() as usize,
                                choice % 4,
                            );
                            Some(next)
                        })
                    })
                    .collect();
            }
            continue;
        }
        if matches!(
            name.as_str(),
            "DETECTOR" | "OBSERVABLE_INCLUDE" | "TICK" | "QUBIT_COORDS" | "SHIFT_COORDS"
        ) {
            continue;
        }
        for target in targets {
            let q = target.qubit_index().unwrap() as usize;
            if matches!(
                name.as_str(),
                "M" | "MZ" | "MX" | "MY" | "MR" | "MRZ" | "MRX" | "MRY" | "R" | "RZ" | "RX" | "RY"
            ) {
                let basis = if name.ends_with('X') {
                    'X'
                } else if name.ends_with('Y') {
                    'Y'
                } else {
                    'Z'
                };
                branches = branches
                    .into_iter()
                    .flat_map(|branch| {
                        let mut results = Vec::new();
                        for outcome in [false, true] {
                            let probability =
                                branch.state.measurement_probability(q, basis, outcome);
                            if probability < 1e-14 {
                                continue;
                            }
                            let mut next = branch.clone();
                            next.weight *= probability;
                            next.state.collapse(q, basis, outcome);
                            if (name.starts_with('R') || name.starts_with("MR")) && outcome {
                                if basis == 'X' {
                                    next.state.z(q);
                                } else {
                                    next.state.x(q);
                                }
                            }
                            if name.starts_with('M') {
                                results.extend(report(
                                    next,
                                    outcome,
                                    matches!(target, StimTarget::QubitInv(_)),
                                    args.first().copied().unwrap_or(0.),
                                ));
                            } else {
                                results.push(next);
                            }
                        }
                        results
                    })
                    .collect();
            } else if matches!(
                name.as_str(),
                "X_ERROR" | "Y_ERROR" | "Z_ERROR" | "DEPOLARIZE1"
            ) {
                let choices = if name == "DEPOLARIZE1" {
                    vec![1, 2, 3]
                } else {
                    vec![match name.as_str() {
                        "X_ERROR" => 1,
                        "Y_ERROR" => 2,
                        _ => 3,
                    }]
                };
                let num_choices = choices.len();
                branches = branches
                    .into_iter()
                    .flat_map(|branch| {
                        std::iter::once(0)
                            .chain(choices.clone())
                            .filter_map(move |choice| {
                                let weight = if choice == 0 {
                                    1. - args[0]
                                } else {
                                    args[0] / num_choices as f64
                                };
                                if weight == 0. {
                                    return None;
                                }
                                let mut next = branch.clone();
                                next.weight *= weight;
                                pauli(&mut next.state, q, choice);
                                Some(next)
                            })
                    })
                    .collect();
            } else {
                for branch in &mut branches {
                    match name.as_str() {
                        "I" => {}
                        "H" => branch.state.h(q),
                        "S" => branch.state.s(q),
                        "S_DAG" => branch.state.s_dag(q),
                        "X" => branch.state.x(q),
                        "Y" => branch.state.y(q),
                        "Z" => branch.state.z(q),
                        "T" => branch.state.t(q),
                        "T_DAG" => branch.state.t_dag(q),
                        _ => panic!("oracle gate {name}"),
                    }
                }
            }
        }
    }
    branches
}

fn key(bits: &[bool]) -> u64 {
    bits.iter()
        .enumerate()
        .fold(0, |word, (i, &bit)| word | (u64::from(bit) << i))
}
fn check_distribution(text: &str, qubits: usize) {
    check_distribution_with_arithmetic(text, qubits, CompiledRotationArithmetic::Strict);
}
fn check_distribution_with_arithmetic(
    text: &str,
    qubits: usize,
    arithmetic: CompiledRotationArithmetic,
) {
    let branches = evolve(
        &rstim::parser::parse_lines(text).unwrap(),
        vec![Branch {
            state: DenseOracle::new(qubits),
            records: Vec::new(),
            weight: 1.,
        }],
    );
    let mut expected = BTreeMap::<u64, f64>::new();
    for branch in branches {
        *expected.entry(key(&branch.records)).or_default() += branch.weight;
    }
    assert!((expected.values().sum::<f64>() - 1.).abs() < 2e-12);
    let plan =
        CompiledNearCliffordExecutor::compile_text_with_arithmetic(text, arithmetic).unwrap();
    assert_eq!(plan.rotation_arithmetic(), arithmetic);
    let shots = 16384;
    let mut observed = BTreeMap::<u64, usize>::new();
    let mut rng = StdRng::seed_from_u64(20261007);
    let mut flat_rng = rng.clone();
    let structured = plan.sample(shots, &mut rng).unwrap();
    let flat = plan
        .prepare_sampler()
        .unwrap()
        .sample_measurements_u8(shots, &mut flat_rng)
        .unwrap();
    assert_eq!(
        flat.measurements,
        structured
            .iter()
            .flat_map(|shot| shot.measurements.iter().copied().map(u8::from))
            .collect::<Vec<_>>()
    );
    assert_eq!(rng.next_u64(), flat_rng.next_u64());
    for shot in structured {
        *observed.entry(key(&shot.measurements)).or_default() += 1;
    }
    for &word in observed.keys() {
        assert!(
            expected.get(&word).copied().unwrap_or(0.) > 1e-12,
            "forbidden branch {word}: {text}"
        );
    }
    for (word, probability) in expected {
        let actual = observed.get(&word).copied().unwrap_or(0) as f64 / shots as f64;
        let tolerance =
            7. * (probability * (1. - probability) / shots as f64).sqrt() + 4. / shots as f64;
        assert!(
            (actual - probability).abs() <= tolerance,
            "word={word}, actual={actual}, expected={probability}: {text}"
        );
    }
}

#[test]
fn compiled_multibit_y_projection_and_feedback_match_dense_conditional_tomography() {
    let prefix = "H 0 1 2\nT 0 1 2\nCX 0 1\nCX 1 2\nMY 2\nT_DAG 1\nCX rec[-1] 0\nMX 0\n";
    for basis in ['X', 'Y', 'Z'] {
        check_distribution(&format!("{prefix}M{basis} 1\n"), 3);
    }
    let prefix = "H 0 1\nT 0\nCX 0 1\nMY !1\nCX rec[-1] 0\nT_DAG 0\nH 1\nT 1\n";
    for a in ['X', 'Y', 'Z'] {
        for b in ['X', 'Y', 'Z'] {
            check_distribution(&format!("{prefix}M{a} 0\nM{b} 1\n"), 2);
        }
    }
}

#[test]
fn compiled_dormant_frame_reset_noise_and_later_rotation_match_dense_channels() {
    for measurement in ["MY 1", "MY !1", "MRY !1"] {
        check_distribution(
            &format!("H 0\nT 0\nCZ 0 1\nZ_ERROR(1) 1\n{measurement}\nT 0\nMX 0\nM 1\n"),
            2,
        );
    }
    for noise in [
        "X_ERROR(0.37) 1",
        "Y_ERROR(1) 1",
        "Z_ERROR(0) 1",
        "DEPOLARIZE1(0.29) 1",
        "DEPOLARIZE2(0.23) 0 1",
    ] {
        check_distribution(
            &format!("H 0\nT 0\nCX 0 1\n{noise}\nMRY !1\nCY rec[-1] 0\nT_DAG 0\nMX 0\nMY 1\n"),
            2,
        );
    }
    check_distribution("H 0 1\nT 0 1\nCX 0 1\nRX 0\nRY 1\nT 0\nMX 0\nMY 1\n", 2);
}

#[test]
fn compiled_flat_structured_prepared_and_split_calls_preserve_own_rng_stream() {
    let text = "H 0\nT 0\nCX 0 1\nY_ERROR(0.37) 0\nMY !1\nCX rec[-1] 0\nT_DAG 0\nMRX 0\nCX sweep[3] 1\nM 0 1\nDETECTOR rec[-1] rec[-2]\nOBSERVABLE_INCLUDE(2) rec[-3]\nOBSERVABLE_INCLUDE(2) rec[-1]\n";
    let plan = CompiledNearCliffordExecutor::compile_text(text).unwrap();
    let mut a = StdRng::seed_from_u64(17);
    let mut b = a.clone();
    let mut c = a.clone();
    let sweep = [false, false, false, true];
    let structured = plan
        .prepare_sampler()
        .unwrap()
        .sample_with_sweep(37, &sweep, &mut a)
        .unwrap();
    let flat = plan
        .prepare_sampler()
        .unwrap()
        .sample_measurements_u8_with_sweep(37, &sweep, &mut b)
        .unwrap();
    assert_eq!(
        flat.measurements,
        structured
            .iter()
            .flat_map(|s| s.measurements.iter().copied().map(u8::from))
            .collect::<Vec<_>>()
    );
    let continuation = a.next_u64();
    assert_eq!(continuation, b.next_u64());
    let mut prepared = plan.prepare_sampler().unwrap();
    let mut split = prepared.sample_with_sweep(13, &sweep, &mut c).unwrap();
    split.extend(prepared.sample_with_sweep(24, &sweep, &mut c).unwrap());
    assert_eq!(structured, split);
    assert_eq!(continuation, c.next_u64());
    for shot in structured {
        assert_eq!(
            shot.detectors,
            [shot.measurements[3] ^ shot.measurements[2]]
        );
        assert_eq!(
            shot.observables,
            [(2, shot.measurements[1]), (2, shot.measurements[3])]
        );
    }
}

#[test]
fn compiled_wide_frame_word_boundaries_preserve_bell_correlations() {
    for q in [63, 64, 127, 128] {
        let text = format!("H 0\nCX 0 {q}\nX_ERROR(1) {q}\nM 0 {q}\nCX rec[-1] 0\nT 0\nM 0\n");
        let plan = CompiledNearCliffordExecutor::compile_text(&text).unwrap();
        let mut rng = StdRng::seed_from_u64(64);
        let mut flat_rng = rng.clone();
        let structured = plan.sample(256, &mut rng).unwrap();
        let flat = plan
            .prepare_sampler()
            .unwrap()
            .sample_measurements_u8(256, &mut flat_rng)
            .unwrap();
        assert_eq!(
            flat.measurements,
            structured
                .iter()
                .flat_map(|shot| shot.measurements.iter().copied().map(u8::from))
                .collect::<Vec<_>>()
        );
        assert_eq!(rng.next_u64(), flat_rng.next_u64());
        for shot in structured {
            assert_ne!(shot.measurements[0], shot.measurements[1]);
            assert!(shot.measurements[2]);
        }
    }
}

#[test]
fn compiled_limits_fail_before_sampling_and_do_not_truncate_coherence() {
    let too_many = "H 0 1\nT 0 1\nCX 0 1\nMY 0\nMX 1\n";
    assert!(
        CompiledNearCliffordExecutor::compile_with_limit(
            rstim::parser::parse_lines(too_many).unwrap(),
            1
        )
        .is_err()
    );
    assert!(CompiledNearCliffordExecutor::compile_text("REPEAT 1000001 {\nH 0\n}\n").is_err());
    assert!(CompiledNearCliffordExecutor::compile_text("H 4096\n").is_err());
    let mut wide_magic = String::new();
    for q in 0..21 {
        wide_magic.push_str(&format!("H {q}\nT {q}\n"));
    }
    for q in 1..21 {
        wide_magic.push_str(&format!("CX 0 {q}\n"));
    }
    wide_magic.push_str("MY 0\n");
    assert!(
        CompiledNearCliffordExecutor::compile_with_limit(
            rstim::parser::parse_lines(&wide_magic).unwrap(),
            21
        )
        .unwrap_err()
        .contains("coefficient reservation")
    );
    let noisy = StimInstr::new(
        "DEPOLARIZE2",
        vec![0.1],
        (0..20000)
            .flat_map(|_| [StimTarget::Qubit(0), StimTarget::Qubit(128)])
            .collect(),
    );
    assert!(
        CompiledNearCliffordExecutor::compile_with_limit(vec![noisy], 16)
            .unwrap_err()
            .contains("plan reservation")
    );
    let plan =
        CompiledNearCliffordExecutor::compile_text("REPEAT 3 {\nH 0\nMR 0\n}\nM 0\n").unwrap();
    assert_eq!(plan.peak_active_rank(), 0);
    let mut rng = StdRng::seed_from_u64(5);
    let mut unchanged = rng.clone();
    assert!(
        plan.prepare_sampler()
            .unwrap()
            .sample_measurements_u8(usize::MAX, &mut rng)
            .is_err()
    );
    assert_eq!(rng.next_u64(), unchanged.next_u64());
    for shot in plan.sample(32, &mut rng).unwrap() {
        assert!(!shot.measurements[3]);
    }
}

#[test]
fn compiled_coefficient_cache_and_budget_fallback_preserve_records_and_continuation() {
    let text = "H 0 1 2\nT 0 1 2\nREPEAT 3 {\nCX 0 1\nCZ 1 2\nY_ERROR(0.37) 0\nMY !2\nCX rec[-1] 1\nT_DAG 1\nMX 0\nT 2\n}\nM 0 1 2\n";
    let plan = CompiledNearCliffordExecutor::compile_text(text).unwrap();
    let initial = plan
        .prepare_sampler()
        .unwrap()
        .coefficient_cache_reserved_bytes();
    let limited = initial + 512;
    for budget in [1, limited, 64 * 1024 * 1024] {
        let mut a = plan.prepare_sampler_with_cache_budget(0).unwrap();
        let mut b = plan.prepare_sampler_with_cache_budget(budget).unwrap();
        let mut ra = StdRng::seed_from_u64(37);
        let mut rb = ra.clone();
        let start = b.coefficient_cache_reserved_bytes();
        let mut previous = start;
        for round in 0..8 {
            assert_eq!(
                a.sample_measurements_u8(256, &mut ra).unwrap(),
                b.sample_measurements_u8(256, &mut rb).unwrap()
            );
            let reserved = b.coefficient_cache_reserved_bytes();
            assert!(reserved <= budget);
            if budget > 1 {
                assert!(reserved > start, "cache must actually admit transitions");
            }
            if budget == limited && round > 0 {
                assert_eq!(
                    reserved, previous,
                    "exhausted cache must continue via fallback"
                );
            }
            previous = reserved;
        }
        if budget == limited {
            assert!(budget - previous < 288);
        }
        assert_eq!(ra.next_u64(), rb.next_u64());
    }
    assert!(
        plan.prepare_sampler_with_cache_budget(64 * 1024 * 1024 + 1)
            .is_err()
    );
}

#[test]
fn compiled_packet_boundaries_sweep_feedback_and_split_calls_keep_row_rng_order() {
    let text = "H 0 64\nT 0 64\nDEPOLARIZE2(0.43) 0 64\nMY !64\nCX rec[-1] 0\nCX sweep[1] 64\nT_DAG 0\nMRX 0\nM 0 64\n";
    let plan = CompiledNearCliffordExecutor::compile_text(text).unwrap();
    let sweep = [false, true];
    for shots in [0, 1, 31, 32, 33, 63, 64, 65, 127, 129] {
        let mut a = StdRng::seed_from_u64(419);
        let mut b = a.clone();
        let expected = plan
            .prepare_sampler_with_cache_budget(0)
            .unwrap()
            .sample_with_sweep(shots, &sweep, &mut a)
            .unwrap();
        let actual = plan
            .prepare_sampler()
            .unwrap()
            .sample_measurements_u8_with_sweep(shots, &sweep, &mut b)
            .unwrap();
        assert_eq!(
            actual.measurements,
            expected
                .iter()
                .flat_map(|shot| shot.measurements.iter().copied().map(u8::from))
                .collect::<Vec<_>>()
        );
        assert_eq!(a.next_u64(), b.next_u64());
    }
    for parts in [[31, 34], [1, 64], [32, 33], [63, 2]] {
        let mut a = StdRng::seed_from_u64(53);
        let mut b = a.clone();
        let expected = plan
            .prepare_sampler()
            .unwrap()
            .sample_measurements_u8_with_sweep(65, &sweep, &mut a)
            .unwrap();
        let mut sampler = plan.prepare_sampler().unwrap();
        let mut actual = Vec::new();
        for part in parts {
            actual.extend(
                sampler
                    .sample_measurements_u8_with_sweep(part, &sweep, &mut b)
                    .unwrap()
                    .measurements,
            );
        }
        assert_eq!(expected.measurements, actual);
        assert_eq!(a.next_u64(), b.next_u64());
    }
}

#[test]
fn compiled_commuting_measurements_reduce_rank_and_preserve_signed_record_scatter() {
    let plan = CompiledNearCliffordExecutor::compile_with_limit(
        rstim::parser::parse_lines("H 0 1\nT 0 1\nMX 0 1\n").unwrap(),
        1,
    )
    .unwrap();
    assert_eq!(plan.peak_active_rank(), 1);
    check_distribution("H 0 1\nT 0 1\nCX 0 1\nMY !0\nMY 0\nMY !0\nMX 1\n", 2);
    check_distribution("H 0 1\nT 0 1\nCX 0 1\nZ_ERROR(0.37) 1\nMY 0\nMX !1\n", 2);
    check_distribution(
        "H 0 1\nT 0 1\nCX 0 1\nDEPOLARIZE1(0.23) 1\nMY 0\nMX !1\n",
        2,
    );
    check_distribution(
        "H 0 1\nT 0 1\nMY !1\nCX rec[-1] 0\nT_DAG 0\nMRY !0\nT 0\nMX 0\nMY 1\n",
        2,
    );
}

#[test]
fn compiled_commuting_reset_instruments_retire_magic_before_later_rotation() {
    let plan =
        CompiledNearCliffordExecutor::compile_text("H 0\nT 0\nCX 0 1\nMR 1\nT 0\nM 0 1\n").unwrap();
    assert_eq!(plan.peak_active_rank(), 0);
    check_distribution("H 0\nT 0\nCX 0 1\nMR !1\nT_DAG 0\nMX 0\nM 1\n", 2);
    check_distribution(
        "H 0\nT 0\nCX 0 1\nX_ERROR(0.37) 1\nMR !1\nT_DAG 0\nMY 0\nM 1\n",
        2,
    );
    check_distribution("H 0 1\nT 0 1\nCZ 0 1\nMRY !1\nMY 0\nT 1\nMX 1\n", 2);
}

#[test]
fn compiled_sampling_only_rotation_elimination_preserves_noise_and_classical_branches() {
    let text = "H 0\nT 0\nX_ERROR(0.37) 0\nCX 0 1\nMR !1\nCX rec[-1] 0\nT_DAG 0\nM 0 1\n";
    let plan = CompiledNearCliffordExecutor::compile_text(text).unwrap();
    assert_eq!(plan.peak_active_rank(), 0);
    check_distribution(text, 2);
    check_distribution(
        "H 0\nT 0\nDEPOLARIZE1(0.41) 0\nCX 0 1\nMR !1\nT 0\nM 0 1\n",
        2,
    );
    // A future anticommuting rotation affects records and must block elimination.
    check_distribution("H 0\nT 0\nH 0\nT_DAG 0\nDEPOLARIZE1(0.31) 0\nMY !0\n", 1);
    check_distribution(
        "H 0 1\nT 0\nCZ 0 1\nMY 1\nT_DAG 1\nCX rec[-1] 0\nT 0\nMX 0\nMY !1\n",
        2,
    );
}

#[test]
fn compiled_sparse_noise_runs_and_word_pools_preserve_full_record_distribution() {
    check_distribution(
        "RX 0 1\nZ_ERROR(0.003) 0\nMX 0\nMRX 0\nDEPOLARIZE2(0.007) 0 1\nMX 0 1\nZ_ERROR(0.003) 1\nMX !1\n",
        2,
    );
    check_distribution(
        "H 0\nT 0\nX_ERROR(0.001) 0\nMY 0\nY_ERROR(0.006) 0\nT_DAG 0\nZ_ERROR(0.001) 0\nMX 0\n",
        1,
    );
    let text = "REPEAT 65 {\nRX 0\nZ_ERROR(0.001) 0\nMX 0\n}\n";
    let plan = CompiledNearCliffordExecutor::compile_text(text).unwrap();
    let mut a = StdRng::seed_from_u64(970);
    let mut b = a.clone();
    let mut scalar = plan.prepare_sampler_with_cache_budget(0).unwrap();
    let mut packed = plan.prepare_sampler().unwrap();
    let expected = scalar.sample_measurements_u8(129, &mut a).unwrap();
    let mut actual = Vec::new();
    for shots in [31, 33, 65] {
        actual.extend(
            packed
                .sample_measurements_u8(shots, &mut b)
                .unwrap()
                .measurements,
        );
    }
    assert_eq!(expected.measurements, actual);
    assert_eq!(a.next_u64(), b.next_u64());
}

#[test]
fn compiled_independent_pool_refill_and_long_unobserved_prefix_are_bounded() {
    let text = "REPEAT 65 {\nR 0\nH 0\nM 0\n}\n";
    let plan = CompiledNearCliffordExecutor::compile_text(text).unwrap();
    let mut a = StdRng::seed_from_u64(437);
    let mut b = a.clone();
    let expected = plan
        .prepare_sampler_with_cache_budget(0)
        .unwrap()
        .sample_measurements_u8(129, &mut a)
        .unwrap();
    let mut packed = plan.prepare_sampler().unwrap();
    let mut actual = Vec::new();
    for shots in [31, 33, 65] {
        actual.extend(
            packed
                .sample_measurements_u8(shots, &mut b)
                .unwrap()
                .measurements,
        );
    }
    assert_eq!(expected.measurements, actual);
    assert_eq!(a.next_u64(), b.next_u64());
    let long = CompiledNearCliffordExecutor::compile_text("REPEAT 20000 {\nT 0\n}\n").unwrap();
    assert_eq!(long.peak_active_rank(), 0);
    assert_eq!(
        long.prepare_sampler()
            .unwrap()
            .sample_measurements_u8(1, &mut a)
            .unwrap()
            .measurements_per_shot,
        0
    );
}

#[test]
fn compiled_native_mpp_matches_direct_dense_projectors_with_signed_y_and_readout() {
    // Three Y factors exercise the imaginary tensor phase; two inversions cancel.
    // X0 followed by Z0 also requires sequential, noncommuting instruments.
    for probability in [0., 0.37, 1.] {
        check_distribution(
            &format!(
                "H 0 1 2\nT 0 1 2\nS 1\nMPP({probability}) !Y0*!Y1*Y2 X0 Z0\nCX rec[-2] 1\nT_DAG 1\nMPP !X1*Y2\nMY 0\nM 0 1 2\n"
            ),
            3,
        );
    }
    check_distribution(
        "H 0\nCX 0 1\nMPP Y0*Y1 !Y0*!Y1 !Y0*Y1\nT 0\nMPP X0 Z0\nMY 1\n",
        2,
    );
    check_distribution(
        "H 0 1\nT 0 1\nREPEAT 2 {\nMPP(0.37) !Y0*X1\nCZ rec[-1] 0\nT_DAG 1\n}\nMPP X0*Y1\nM 0 1\n",
        2,
    );
}

#[test]
fn compiled_native_noisy_measurement_resets_ideal_state_and_feedback_uses_reported_bit() {
    for basis in ['X', 'Y', 'Z'] {
        for probability in [0., 0.37, 1.] {
            check_distribution(
                &format!(
                    "H 0\nT 0\nCX 0 1\nM{basis}({probability}) !1\nCX rec[-1] 0\nT_DAG 0\nMR{basis}({probability}) !0\nM{basis} 0\nMPP Z0*Z1\nM 0 1\n"
                ),
                2,
            );
        }
    }
    for (reset, measurement, final_measurement) in [
        ("R", "MR", "M"),
        ("RZ", "MRZ", "MZ"),
        ("RX", "MRX", "MX"),
        ("RY", "MRY", "MY"),
    ] {
        for (probability, inversion) in [(1., ""), (0., "!")] {
            let plan = CompiledNearCliffordExecutor::compile_text(&format!(
                "{reset} 0\n{measurement}({probability}) {inversion}0\n{final_measurement} 0\n"
            ))
            .unwrap();
            for shot in plan.sample(65, &mut StdRng::seed_from_u64(910)).unwrap() {
                assert_eq!(shot.measurements, [true, false]);
            }
        }
    }
}

#[test]
fn compiled_native_mpp_and_readout_keep_cache_packet_and_split_rng_streams() {
    let text = "H 0 64\nT 0 64\nMPP(0.003) !Y0*Y64 X0\nCX rec[-2] 64\nMRX(0.37) !0\nCX rec[-1] 64\nT_DAG 64\nMPP(1) X0*!Z64\nM(0.007) !0 64\nDETECTOR rec[-1] rec[-3]\nOBSERVABLE_INCLUDE(2) rec[-5]\n";
    let plan = CompiledNearCliffordExecutor::compile_text(text).unwrap();
    for shots in [0, 1, 31, 32, 63, 64, 65, 129] {
        let mut a = StdRng::seed_from_u64(530);
        let mut b = a.clone();
        let expected = plan
            .prepare_sampler_with_cache_budget(0)
            .unwrap()
            .sample(shots, &mut a)
            .unwrap();
        let actual = plan
            .prepare_sampler()
            .unwrap()
            .sample_measurements_u8(shots, &mut b)
            .unwrap();
        assert_eq!(
            actual.measurements,
            expected
                .iter()
                .flat_map(|shot| shot.measurements.iter().copied().map(u8::from))
                .collect::<Vec<_>>()
        );
        assert_eq!(a.next_u64(), b.next_u64());
        for shot in expected {
            assert_eq!(
                shot.detectors,
                [shot.measurements[5] ^ shot.measurements[3]]
            );
            assert_eq!(shot.observables, [(2, shot.measurements[1])]);
        }
    }
    for parts in [[31, 98], [64, 65], [1, 128]] {
        let mut a = StdRng::seed_from_u64(531);
        let mut b = a.clone();
        let expected = plan.sample(129, &mut a).unwrap();
        let mut sampler = plan.prepare_sampler().unwrap();
        let mut actual = Vec::new();
        for count in parts {
            actual.extend(sampler.sample(count, &mut b).unwrap());
        }
        assert_eq!(expected, actual);
        assert_eq!(a.next_u64(), b.next_u64());
    }
}

#[test]
fn compiled_native_mpp_validates_full_original_width_and_product_readout_contract() {
    let plan = CompiledNearCliffordExecutor::compile_text("MPP !Z0*Z4095 Z4095\nM 4095\n").unwrap();
    assert_eq!(plan.peak_active_rank(), 0);
    let batch = plan
        .prepare_sampler()
        .unwrap()
        .sample_measurements_u8(2, &mut StdRng::seed_from_u64(4095))
        .unwrap();
    assert_eq!(batch.measurements, [1, 0, 0, 1, 0, 0]);
    for text in [
        "MPP Z0*Z4096\n",
        "MPP\n",
        "MPP *X0\n",
        "MPP X0*\n",
        "MPP X0**Y1\n",
        "MPP X0*X0\n",
        "MPP X0*Y0\n",
        "MPP 0\n",
        "MPP(-0.1) X0\n",
        "MPP(1.1) X0\n",
        "MPP(nan) X0\n",
        "MPP(inf) X0\n",
        "MPP(0.1,0.2) X0\n",
        "M(-0.1) 0\n",
        "MX(1.1) 0\n",
        "MY(nan) 0\n",
        "MR(inf) 0\n",
        "MRX(0.1,0.2) 0\n",
        "RY(0.1) 0\n",
        "MPP X0\nCX rec[-2] 1\n",
    ] {
        assert!(
            CompiledNearCliffordExecutor::compile_text(text).is_err(),
            "accepted {text}"
        );
    }
    // The new opt-in contract does not broaden the incumbent's validation API.
    assert!(rstim::near_clifford::NearCliffordExecutor::compile_text("MPP X0\n").is_err());
    assert!(rstim::near_clifford::NearCliffordExecutor::compile_text("MR(0.37) 0\n").is_err());
}

#[test]
fn compiled_shared_coherent_states_preserve_mixed_pauli_signs_and_noisy_feedback() {
    for text in [
        "H 0\nT 0\nX_ERROR(0.5) 0\nT 0\nMX 0\n",
        "H 0 1\nT 0 1\nDEPOLARIZE2(1) 0 1\nT_DAG 0\nMPP(0.37) X0*Y1\nCX rec[-1] 0\nMRX(1) 0\nT 1\nMY 1\nM 0 1\n",
    ] {
        check_distribution(text, 2);
        let plan = CompiledNearCliffordExecutor::compile_text(text).unwrap();
        let initial = plan
            .prepare_sampler()
            .unwrap()
            .coefficient_cache_reserved_bytes();
        for budget in [0, initial + 288, 64 * 1024 * 1024] {
            let mut packed = plan.prepare_sampler_with_cache_budget(budget).unwrap();
            let mut scalar = plan.prepare_sampler_with_cache_budget(0).unwrap();
            let mut a = StdRng::seed_from_u64(852);
            let mut b = a.clone();
            for shots in [31, 32, 63, 64, 65, 64] {
                let expected = scalar.sample_measurements_u8(shots, &mut a).unwrap();
                let actual = packed.sample_measurements_u8(shots, &mut b).unwrap();
                assert_eq!(expected, actual);
                assert_eq!(a.next_u64(), b.next_u64());
            }
        }
    }
}

#[test]
fn compiled_batched_coherence_keeps_full_joint_records_after_projection_noise_and_feedback() {
    let text = "H 0 1 2 3\nT 0 1 2 3\nDEPOLARIZE2(0.23) 0 1\nMPP(0.37) !X0*Y1*X2*Y3\nMRX(0.41) !0\nCX rec[-1] 3\nT_DAG 3\nMRY 1\nMY 2\nMX 3\nM 0 1 2 3\n";
    let plan = CompiledNearCliffordExecutor::compile_text(text).unwrap();
    assert!(plan.peak_active_rank() >= 4);
    check_distribution(text, 4);
    for budget in [0, 16384, 64 * 1024 * 1024] {
        let mut scalar = plan.prepare_sampler_with_cache_budget(0).unwrap();
        let mut flat = plan.prepare_sampler_with_cache_budget(budget).unwrap();
        let mut a = StdRng::seed_from_u64(461);
        let mut b = a.clone();
        for parts in [[1, 31, 32, 65], [63, 1, 64, 1], [32, 33, 32, 32]] {
            for shots in parts {
                let expected = scalar.sample(shots, &mut a).unwrap();
                let actual = flat.sample_measurements_u8(shots, &mut b).unwrap();
                assert_eq!(
                    actual.measurements,
                    expected
                        .iter()
                        .flat_map(|shot| shot.measurements.iter().copied().map(u8::from))
                        .collect::<Vec<_>>()
                );
                assert_eq!(a.next_u64(), b.next_u64());
            }
        }
    }
}

#[test]
fn compiled_large_coherent_packets_use_bounded_scalar_fallback() {
    let targets = (0..16).map(|q| q.to_string()).collect::<Vec<_>>().join(" ");
    let product = (0..16)
        .map(|q| format!("X{q}"))
        .collect::<Vec<_>>()
        .join("*");
    let plan = CompiledNearCliffordExecutor::compile_text(&format!(
        "H {targets}\nT {targets}\nMPP {product}\n"
    ))
    .unwrap();
    assert_eq!(plan.peak_active_rank(), 16);
    let mut a = StdRng::seed_from_u64(751);
    let mut b = a.clone();
    let expected = plan
        .prepare_sampler_with_cache_budget(0)
        .unwrap()
        .sample(33, &mut a)
        .unwrap();
    let actual = plan
        .prepare_sampler()
        .unwrap()
        .sample_measurements_u8(33, &mut b)
        .unwrap();
    assert_eq!(
        actual.measurements,
        expected
            .iter()
            .map(|shot| u8::from(shot.measurements[0]))
            .collect::<Vec<_>>()
    );
    assert_eq!(a.next_u64(), b.next_u64());
}

#[test]
fn compiled_scheduled_noise_keeps_identity_record_corrections_and_uses_effective_pauli() {
    // After the first swap the X channel becomes identity plus record0 XOR.
    // The second measurement must use that identity, not the original X.
    for text in [
        "X_ERROR(1) 0\nMR 0\nM 0\n",
        "X_ERROR(1) 0\nMR 0\nM 0\nDETECTOR rec[-2]\nOBSERVABLE_INCLUDE(3) rec[-2]\n",
    ] {
        let plan = CompiledNearCliffordExecutor::compile_text(text).unwrap();
        for shot in plan.sample(129, &mut StdRng::seed_from_u64(9101)).unwrap() {
            assert_eq!(shot.measurements, [true, false]);
            if text.contains("DETECTOR") {
                assert_eq!(shot.detectors, [true]);
                assert_eq!(shot.observables, [(3, true)]);
            }
        }
        for budget in [0, 64 * 1024 * 1024] {
            let mut sampler = plan.prepare_sampler_with_cache_budget(budget).unwrap();
            for shots in [1, 31, 32, 63, 64, 65, 129] {
                let flat = sampler
                    .sample_measurements_u8(shots, &mut StdRng::seed_from_u64(9102))
                    .unwrap();
                assert_eq!(flat.measurements, [1u8, 0].repeat(shots));
            }
        }
    }
}

#[test]
fn compiled_scheduled_noise_with_reset_and_future_rotations_matches_dense_channels() {
    for (text, qubits) in [
        ("X_ERROR(1) 0\nR 0\nH 0\nT 0\nH 0\nM 0\n", 1),
        ("Y_ERROR(0.37) 0\nR 0\nH 0\nT 0\nH 0\nM 0\n", 1),
        ("H 0\nT 0\nY_ERROR(0.37) 0\nMR 0\nMY 0\nT_DAG 0\nMX 0\n", 1),
        ("H 0\nT 0\nDEPOLARIZE1(1) 0\nMRX 0\nMY 0\nT 0\nM 0\n", 1),
        (
            "H 0 1\nT 0 1\nDEPOLARIZE2(0.37) 0 1\nMPP !Y0*X1 X0*!Y1\nMR(0.37) !0\nMPP(1) !Z0*Y1\nT_DAG 1\nM 0 1\n",
            2,
        ),
    ] {
        check_distribution(text, qubits);
    }
}

#[test]
fn compiled_scheduled_noise_readout_feedback_and_annotations_keep_final_records() {
    for probability in [0., 0.37, 1.] {
        let text = format!(
            "X_ERROR(0.37) 0\nMR({probability}) !0\nCX rec[-1] 1\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(3) rec[-1]\nM 1\nH 0\nT 0\nH 0\nMPP Y0*X1\nM 0 1\n"
        );
        check_distribution(&text, 2);
        let plan = CompiledNearCliffordExecutor::compile_text(&text).unwrap();
        for shot in plan.sample(129, &mut StdRng::seed_from_u64(9103)).unwrap() {
            assert_eq!(shot.detectors, [shot.measurements[0]]);
            assert_eq!(shot.observables, [(3, shot.measurements[0])]);
            // The classical consumer follows the noise/record correction.
            assert_eq!(shot.measurements[1], shot.measurements[0]);
        }
    }
}

#[test]
fn compiled_scheduled_multichoice_noise_preserves_packet_cache_and_split_streams() {
    // Both noise channels can cross multiple products and resets. Y factors and
    // record inversions exercise each choice's independent report-label edits.
    let text = "H 0 1\nT 0 1\nDEPOLARIZE2(0.003) 0 1\nMPP(0.007) !Y0*X1 X0*!Y1\nMRX(0.37) !0\nDEPOLARIZE2(1) 0 1\nMPP(1) Z0*Y1 X0\nCX rec[-2] 1\nDETECTOR rec[-2] rec[-4]\nOBSERVABLE_INCLUDE(2) rec[-1]\nT 1\nM 0 1\n";
    check_distribution(text, 2);
    let plan = CompiledNearCliffordExecutor::compile_text(text).unwrap();
    let initial = plan
        .prepare_sampler()
        .unwrap()
        .coefficient_cache_reserved_bytes();
    for budget in [0, initial + 288, 64 * 1024 * 1024] {
        let mut a = StdRng::seed_from_u64(9104);
        let mut b = a.clone();
        let mut scalar = plan.prepare_sampler_with_cache_budget(0).unwrap();
        let mut packed = plan.prepare_sampler_with_cache_budget(budget).unwrap();
        for count in [0, 1, 31, 32, 33, 63, 64, 65, 129] {
            let expected = scalar.sample(count, &mut a).unwrap();
            let actual = packed.sample_measurements_u8(count, &mut b).unwrap();
            assert_eq!(
                actual.measurements,
                expected
                    .iter()
                    .flat_map(|shot| shot.measurements.iter().copied().map(u8::from))
                    .collect::<Vec<_>>()
            );
            assert_eq!(a.next_u64(), b.next_u64());
            for shot in expected {
                assert_eq!(
                    shot.detectors,
                    [shot.measurements[3] ^ shot.measurements[1]]
                );
                assert_eq!(shot.observables, [(2, shot.measurements[4])]);
            }
        }
    }
    for parts in [[31, 98], [64, 65], [1, 128], [63, 66]] {
        let mut a = StdRng::seed_from_u64(9105);
        let mut b = a.clone();
        let expected = plan.sample(129, &mut a).unwrap();
        let mut sampler = plan.prepare_sampler().unwrap();
        let mut actual = Vec::new();
        for count in parts {
            actual.extend(sampler.sample(count, &mut b).unwrap());
        }
        assert_eq!(actual, expected);
        assert_eq!(a.next_u64(), b.next_u64());
    }
}

#[test]
fn compiled_long_random_runs_keep_records_and_rng_across_packets_and_call_splits() {
    let mut text = String::from("H 0\nT 0\n");
    for _ in 0..129 {
        text.push_str("R 1\nH 1\nM 1\n");
    }
    text.push_str("REPEAT 17 {\nX_ERROR(0.01) 0 2 0 2 0 2\nDEPOLARIZE1(0.01) 2 2 2\nY_ERROR(0.37) 2\nMR(0.001) 2\nCX rec[-1] 0\n}\nMX 0\nM 1 2\n");
    let plan = CompiledNearCliffordExecutor::compile_text(&text).unwrap();
    let mut expected_rng = StdRng::seed_from_u64(2810);
    let mut flat_rng = expected_rng.clone();
    let mut split_rng = expected_rng.clone();
    let expected = plan
        .prepare_sampler_with_cache_budget(0)
        .unwrap()
        .sample(129, &mut expected_rng)
        .unwrap()
        .iter()
        .flat_map(|shot| shot.measurements.iter().copied().map(u8::from))
        .collect::<Vec<_>>();
    let actual = plan
        .prepare_sampler()
        .unwrap()
        .sample_measurements_u8(129, &mut flat_rng)
        .unwrap();
    assert_eq!(expected, actual.measurements);
    let mut split = plan.prepare_sampler().unwrap();
    let mut split_records = split
        .sample_measurements_u8(31, &mut split_rng)
        .unwrap()
        .measurements;
    split_records.extend(
        split
            .sample_measurements_u8(98, &mut split_rng)
            .unwrap()
            .measurements,
    );
    assert_eq!(expected, split_records);
    let continuation = expected_rng.next_u64();
    assert_eq!(continuation, flat_rng.next_u64());
    assert_eq!(continuation, split_rng.next_u64());
}

#[test]
fn compiled_compact_noise_keeps_cache_replay_tails_and_split_typed_rng_streams() {
    let mut text = String::from(
        "H 0 1\nT 0 1\nDEPOLARIZE2(0) 0 1\nDEPOLARIZE2(1) 0 1\nMPP(0.007) !Y0*X1\nMRX(0.37) !0\nCX rec[-1] 1\n",
    );
    text.push_str("REPEAT 33 {\nX_ERROR(0.01) 0 2\nDEPOLARIZE1(0.01) 2\nDEPOLARIZE2(0.01) 0 1\nY_ERROR(0.37) 2\nMR(0.001) 2\nCX rec[-1] 0\nR 3\nH 3\nM 3\n}\nT_DAG 1\nMX 0\nMY 1\n");
    for arithmetic in [
        CompiledRotationArithmetic::Strict,
        CompiledRotationArithmetic::Fused,
    ] {
        let plan =
            CompiledNearCliffordExecutor::compile_text_with_arithmetic(&text, arithmetic).unwrap();
        let initial = plan
            .prepare_sampler()
            .unwrap()
            .coefficient_cache_reserved_bytes();
        for budget in [0, initial + 288, 64 * 1024 * 1024] {
            let mut scalar = plan.prepare_sampler_with_cache_budget(0).unwrap();
            let mut packed = plan.prepare_sampler_with_cache_budget(budget).unwrap();
            let mut a = StdRng::seed_from_u64(1604);
            let mut b = a.clone();
            for shots in [64, 63, 129, 1, 32, 64] {
                let expected = scalar.sample(shots, &mut a).unwrap();
                let actual = packed.sample_measurements_u8(shots, &mut b).unwrap();
                assert_eq!(
                    actual.measurements,
                    expected
                        .iter()
                        .flat_map(|shot| shot.measurements.iter().copied().map(u8::from))
                        .collect::<Vec<_>>()
                );
                assert_eq!(a.next_u64(), b.next_u64());
            }
        }
        let mut a = StdRng::seed_from_u64(1605);
        let mut b = a.clone();
        let expected = plan.sample(129, &mut a).unwrap();
        let mut packed = plan.prepare_sampler().unwrap();
        let mut actual = packed
            .sample_measurements_u8(64, &mut b)
            .unwrap()
            .measurements;
        actual.extend(
            packed
                .sample_measurements_u8(65, &mut b)
                .unwrap()
                .measurements,
        );
        assert_eq!(
            actual,
            expected
                .iter()
                .flat_map(|shot| shot.measurements.iter().copied().map(u8::from))
                .collect::<Vec<_>>()
        );
        assert_eq!(a.next_u64(), b.next_u64());
    }
}

#[test]
fn compiled_both_rotation_policies_match_independent_full_joint_noisy_density() {
    // Evolve every dense physical branch independently, then compare public
    // structured and flat samples with that complete joint distribution.
    // Rank four also exercises the coherent packet path in the flat API.
    let text = "H 0 1 2 3\nT 0 1 2 3\nDEPOLARIZE2(0.23) 0 1\nMPP(0.37) !X0*Y1*X2*Y3\nMRX(0.41) !0\nCX rec[-1] 3\nT_DAG 3\nMRY 1\nMY 2\nMX 3\nM 0 1 2 3\n";
    for arithmetic in [
        CompiledRotationArithmetic::Strict,
        CompiledRotationArithmetic::Fused,
    ] {
        let plan =
            CompiledNearCliffordExecutor::compile_text_with_arithmetic(text, arithmetic).unwrap();
        assert!(plan.peak_active_rank() >= 4);
        assert_eq!(plan.rotation_arithmetic(), arithmetic);
        check_distribution_with_arithmetic(text, 4, arithmetic);
    }
}

#[test]
fn compiled_wide_coherent_packets_keep_raw_records_and_rng_across_tiles_and_tails() {
    let text = "H 0 1 2 3 4 5 6 7\nT 0 1 2 3 4 5 6 7\nDEPOLARIZE2(0.23) 0 1\nMPP(0.37) !X0*Y1*X2*Y3*X4*Y5*Y6*X7\nMRX(0.41) !0\nCX rec[-1] 7\nT_DAG 7\nMRY 1\nMY 2\nMX 3\nM 0 1 2 3 4 5 6 7\n";
    for arithmetic in [
        CompiledRotationArithmetic::Strict,
        CompiledRotationArithmetic::Fused,
    ] {
        let plan =
            CompiledNearCliffordExecutor::compile_text_with_arithmetic(text, arithmetic).unwrap();
        assert!(plan.peak_active_rank() >= 8);
        for budget in [0, 16384, 64 * 1024 * 1024] {
            let mut scalar = plan.prepare_sampler_with_cache_budget(0).unwrap();
            let mut flat = plan.prepare_sampler_with_cache_budget(budget).unwrap();
            let mut a = StdRng::seed_from_u64(464);
            let mut b = a.clone();
            for shots in [0, 1, 31, 32, 63, 64, 65, 129, 64, 1024] {
                let expected = scalar.sample(shots, &mut a).unwrap();
                let actual = flat.sample_measurements_u8(shots, &mut b).unwrap();
                assert_eq!(
                    actual.measurements,
                    expected
                        .iter()
                        .flat_map(|shot| shot.measurements.iter().copied().map(u8::from))
                        .collect::<Vec<_>>()
                );
                assert_eq!(a.next_u64(), b.next_u64());
            }
        }
    }
}

// The late product commutes with feedback on the separate reset ancilla,
// while it requires all four non-Clifford axes at once. This keeps rank four
// available to exercise the public coherent packet strategy on the new plan.
const COMMUTING_CLASSICAL_PACKET_CIRCUIT: &str = "H 0 1 2 3 5\nT 0 1 2 3\nX_ERROR(0.23) 4\nMR(0.37) !4\nCX rec[-1] 4\nDETECTOR rec[-1] rec[-1]\nOBSERVABLE_INCLUDE(2) rec[-1]\nMPP(0.19) !X0*X1*X2*X3\nM 4 5\nDETECTOR rec[-4] rec[-2]\nDETECTOR\nOBSERVABLE_INCLUDE(7) rec[-3]\n";

fn commuting_classical_expected_annotations(
    records: &[bool],
    packet_circuit: bool,
) -> (Vec<bool>, Vec<(u32, bool)>) {
    if packet_circuit {
        assert_eq!(records.len(), 4);
        (
            vec![false, records[0] ^ records[2], false],
            vec![(2, records[0]), (7, records[1])],
        )
    } else {
        assert_eq!(records.len(), 5);
        (
            vec![
                false,
                records[0] ^ records[3],
                records[1] ^ records[2],
                false,
            ],
            vec![(2, records[0]), (7, records[1] ^ records[2])],
        )
    }
}

#[test]
fn compiled_commuting_classical_readers_release_separate_nonclifford_axes() {
    // The blanket reader barriers kept both T axes live simultaneously. The
    // late MX0 can now cross the independent reader and T1, closing axis0
    // before axis1 opens. This small generic witness does not embed a fixture.
    let text = "H 0 1\nT 0 1\nMR 2\nCX rec[-1] 2\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(3) rec[-1]\nMX 0 1\nM 2\n";
    for arithmetic in [
        CompiledRotationArithmetic::Strict,
        CompiledRotationArithmetic::Fused,
    ] {
        let plan =
            CompiledNearCliffordExecutor::compile_text_with_arithmetic(text, arithmetic).unwrap();
        assert_eq!(plan.rotation_arithmetic(), arithmetic);
        assert_eq!(plan.peak_active_rank(), 1);
        check_distribution_with_arithmetic(text, 3, arithmetic);
    }
}

#[test]
fn compiled_commuting_classical_readers_match_dense_joint_noise_reset_and_native_products() {
    let mut circuits = Vec::new();
    for (noise, earlier_readout, later_readout) in [(0., 0., 0.), (0.23, 0.37, 0.41), (1., 1., 1.)]
    {
        // The early MR moves before its X error, making the error an identity
        // Pauli plus a deferred record XOR. The late inverted/noisy MRX may
        // cross the feedback and annotations, but both still consume the
        // final earlier report. The final Z2 result copies that control.
        circuits.push((
            format!(
                "H 0 1\nT 0 1\nX_ERROR({noise}) 2\nMR({earlier_readout}) !2\nCX rec[-1] 2\nDETECTOR rec[-1] rec[-1]\nOBSERVABLE_INCLUDE(2) rec[-1]\nMRX({later_readout}) !0\nMX 1\nM 2\nMPP(0.19) !X0*X1\nDETECTOR rec[-5] rec[-2]\nDETECTOR rec[-4] rec[-3]\nDETECTOR\nOBSERVABLE_INCLUDE(7) rec[-4] rec[-3]\n"
            ),
            3,
            false,
        ));
    }
    circuits.push((COMMUTING_CLASSICAL_PACKET_CIRCUIT.to_owned(), 6, true));
    for (text, qubits, packet_circuit) in circuits {
        // Enumerate full physical branches with the independent matrix oracle.
        // Each complete raw word has an independently enumerated annotation
        // tuple; duplicate/empty detector inputs give exact constant false.
        let expected_annotations = evolve(
            &rstim::parser::parse_lines(&text).unwrap(),
            vec![Branch {
                state: DenseOracle::new(qubits),
                records: Vec::new(),
                weight: 1.,
            }],
        )
        .into_iter()
        .map(|branch| {
            (
                key(&branch.records),
                commuting_classical_expected_annotations(&branch.records, packet_circuit),
            )
        })
        .collect::<BTreeMap<_, _>>();
        for arithmetic in [
            CompiledRotationArithmetic::Strict,
            CompiledRotationArithmetic::Fused,
        ] {
            check_distribution_with_arithmetic(&text, qubits, arithmetic);
            let plan =
                CompiledNearCliffordExecutor::compile_text_with_arithmetic(&text, arithmetic)
                    .unwrap();
            assert_eq!(plan.peak_active_rank(), if packet_circuit { 4 } else { 1 });
            for shot in plan.sample(129, &mut StdRng::seed_from_u64(91017)).unwrap() {
                let expected = &expected_annotations[&key(&shot.measurements)];
                assert_eq!(shot.detectors, expected.0);
                assert_eq!(shot.observables, expected.1);
                // A reset prepares Z=0; only the final earlier reported bit
                // controls X on the ancilla. This is a physical correlation,
                // not merely a comparison with compiler metadata.
                assert_eq!(
                    shot.measurements[0],
                    shot.measurements[if packet_circuit { 2 } else { 3 }]
                );
            }
        }
    }
}

#[test]
fn compiled_new_commuting_plan_keeps_scalar_cache_coherent_and_split_raw_rng_streams() {
    for text in [
        COMMUTING_CLASSICAL_PACKET_CIRCUIT.to_owned(),
        COMMUTING_CLASSICAL_PACKET_CIRCUIT
            .replace("0.23", "0.003")
            .replace("0.37", "0.007")
            .replace("0.19", "0.019"),
    ] {
        for arithmetic in [
            CompiledRotationArithmetic::Strict,
            CompiledRotationArithmetic::Fused,
        ] {
            let plan =
                CompiledNearCliffordExecutor::compile_text_with_arithmetic(&text, arithmetic)
                    .unwrap();
            assert_eq!(plan.peak_active_rank(), 4);
            let initial = plan
                .prepare_sampler()
                .unwrap()
                .coefficient_cache_reserved_bytes();
            for budget in [0, initial + 288, 64 * 1024 * 1024] {
                // Structured sampling is scalar. Budget-zero flat sampling
                // selects coherent packets at >=32 shots; nonzero budgets
                // exercise cache packets and bounded cache fallback/replay.
                let mut scalar = plan.prepare_sampler_with_cache_budget(0).unwrap();
                let mut flat = plan.prepare_sampler_with_cache_budget(budget).unwrap();
                let mut cached_structured = plan.prepare_sampler_with_cache_budget(budget).unwrap();
                let mut a = StdRng::seed_from_u64(91018);
                let mut b = a.clone();
                let mut c = a.clone();
                for shots in [0, 1, 31, 32, 33, 63, 64, 65, 129, 64] {
                    let expected = scalar.sample(shots, &mut a).unwrap();
                    let actual = flat.sample_measurements_u8(shots, &mut b).unwrap();
                    let structured = cached_structured.sample(shots, &mut c).unwrap();
                    assert_eq!(structured, expected);
                    assert_eq!(
                        actual.measurements,
                        expected
                            .iter()
                            .flat_map(|shot| shot.measurements.iter().copied().map(u8::from))
                            .collect::<Vec<_>>()
                    );
                    assert_eq!(a.clone().next_u64(), b.clone().next_u64());
                    assert_eq!(a.clone().next_u64(), c.clone().next_u64());
                    for shot in expected {
                        let annotations =
                            commuting_classical_expected_annotations(&shot.measurements, true);
                        assert_eq!(shot.detectors, annotations.0);
                        assert_eq!(shot.observables, annotations.1);
                    }
                }
                for parts in [[0, 31, 98, 0], [64, 0, 65, 0], [1, 31, 32, 65]] {
                    let mut a = StdRng::seed_from_u64(91019);
                    let mut b = a.clone();
                    let mut c = a.clone();
                    let expected = plan
                        .prepare_sampler_with_cache_budget(0)
                        .unwrap()
                        .sample(129, &mut a)
                        .unwrap();
                    let mut flat = plan.prepare_sampler_with_cache_budget(budget).unwrap();
                    let mut structured = plan.prepare_sampler_with_cache_budget(budget).unwrap();
                    let mut flat_records = Vec::new();
                    let mut structured_shots = Vec::new();
                    for shots in parts {
                        flat_records.extend(
                            flat.sample_measurements_u8(shots, &mut b)
                                .unwrap()
                                .measurements,
                        );
                        structured_shots.extend(structured.sample(shots, &mut c).unwrap());
                    }
                    assert_eq!(structured_shots, expected);
                    assert_eq!(
                        flat_records,
                        expected
                            .iter()
                            .flat_map(|shot| shot.measurements.iter().copied().map(u8::from))
                            .collect::<Vec<_>>()
                    );
                    let continuation = a.next_u64();
                    assert_eq!(continuation, b.next_u64());
                    assert_eq!(continuation, c.next_u64());
                }
            }
        }
    }
}

#[path = "support/near_clifford_structural.rs"]
mod conditional_structural;

#[test]
fn compiled_conditional_noise_schedule_reduces_structural_rank_and_keeps_public_streams() {
    let text = conditional_structural::circuit(16, 8, false);
    for arithmetic in [
        CompiledRotationArithmetic::Strict,
        CompiledRotationArithmetic::Fused,
    ] {
        let plan =
            CompiledNearCliffordExecutor::compile_text_with_arithmetic(&text, arithmetic).unwrap();
        assert_eq!(plan.peak_active_rank(), 5);
        let mut rng = StdRng::seed_from_u64(1081739);
        let mut reference = plan.prepare_sampler_with_cache_budget(0).unwrap();
        let mut expected = Vec::new();
        for _ in 0..129 {
            expected.extend(reference.sample(1, &mut rng).unwrap());
        }
        let raw: Vec<_> = expected
            .iter()
            .flat_map(|s| s.measurements.iter().copied().map(u8::from))
            .collect();
        let final_rng = rng.clone();
        for cache in [0, 2048, 64 * 1024 * 1024] {
            let mut other_rng = StdRng::seed_from_u64(1081739);
            let mut sampler = plan.prepare_sampler_with_cache_budget(cache).unwrap();
            let mut actual = Vec::new();
            for shots in [1, 0, 31, 32, 64, 1] {
                actual.extend(
                    sampler
                        .sample_measurements_u8(shots, &mut other_rng)
                        .unwrap()
                        .measurements,
                );
            }
            assert_eq!(actual, raw, "policy={arithmetic:?} cache={cache}");
            let mut continuation = final_rng.clone();
            for _ in 0..16 {
                assert_eq!(continuation.next_u64(), other_rng.next_u64());
            }
        }
        let accepted = expected
            .iter()
            .filter(|s| s.detectors.iter().all(|b| !*b))
            .count();
        let errors = expected
            .iter()
            .filter(|s| {
                s.detectors.iter().all(|b| !*b)
                    && s.observables
                        .iter()
                        .filter(|(id, _)| *id == 0)
                        .fold(false, |p, (_, v)| p ^ v)
            })
            .count();
        let mut counts_rng = StdRng::seed_from_u64(1081739);
        let counts = plan
            .prepare_sampler()
            .unwrap()
            .sample_postselected_counts(129, 0, &mut counts_rng)
            .unwrap();
        assert_eq!(counts.attempted, 129);
        assert_eq!(counts.accepted, accepted);
        assert_eq!(counts.logical_errors, errors);
        for _ in 0..16 {
            assert_eq!(rng.next_u64(), counts_rng.next_u64());
        }
    }
}

#[test]
fn compiled_conditional_packets_preserve_streams_for_zero_sparse_and_certain_channels() {
    let base = conditional_structural::circuit(8, 3, true);
    for channel in ["X_ERROR", "DEPOLARIZE1", "DEPOLARIZE2"] {
        for probability in ["0", "0.001", "1"] {
            let text = base.replace("DEPOLARIZE1(0.001)", &format!("{channel}({probability})"));
            for arithmetic in [
                CompiledRotationArithmetic::Strict,
                CompiledRotationArithmetic::Fused,
            ] {
                let plan =
                    CompiledNearCliffordExecutor::compile_text_with_arithmetic(&text, arithmetic)
                        .unwrap();
                let mut expected_rng = StdRng::seed_from_u64(391729);
                let mut reference = plan.prepare_sampler_with_cache_budget(0).unwrap();
                let mut expected = Vec::new();
                for _ in 0..129 {
                    expected.extend(
                        reference.sample(1, &mut expected_rng).unwrap()[0]
                            .measurements
                            .iter()
                            .copied()
                            .map(u8::from),
                    );
                }
                for cache in [0, 2048, 8192, 64 * 1024 * 1024] {
                    let mut rng = StdRng::seed_from_u64(391729);
                    let mut sampler = plan.prepare_sampler_with_cache_budget(cache).unwrap();
                    if cache >= 8192 {
                        assert!(sampler.coefficient_cache_reserved_bytes() > 0);
                    }
                    let mut actual = Vec::new();
                    for shots in [64, 0, 1, 63, 1] {
                        actual.extend(
                            sampler
                                .sample_measurements_u8(shots, &mut rng)
                                .unwrap()
                                .measurements,
                        );
                    }
                    assert_eq!(
                        actual, expected,
                        "{channel}({probability}) {arithmetic:?} cache={cache}"
                    );
                    let mut continuation = expected_rng.clone();
                    for _ in 0..16 {
                        assert_eq!(rng.next_u64(), continuation.next_u64());
                    }
                }
            }
        }
    }
}

#[test]
fn compiled_conditional_schedule_matches_independent_signed_noisy_density() {
    let base = conditional_structural::circuit(4, 2, true);
    let mut noisy = String::new();
    let mut channel = false;
    for line in base.lines() {
        if line.starts_with("DEPOLARIZE1") {
            if !channel {
                noisy.push_str("DEPOLARIZE2(0.23) 0 1\n");
                channel = true;
            }
        } else {
            noisy.push_str(line);
            noisy.push('\n');
        }
    }
    noisy = noisy.replacen("M 0\n", "M(0.23) 0\n", 1);
    for arithmetic in [
        CompiledRotationArithmetic::Strict,
        CompiledRotationArithmetic::Fused,
    ] {
        let plan =
            CompiledNearCliffordExecutor::compile_text_with_arithmetic(&noisy, arithmetic).unwrap();
        assert!(
            plan.peak_active_rank() < 4,
            "must validate the new schedule, not fallback"
        );
        check_distribution_with_arithmetic(&noisy, 8, arithmetic);
    }
}
