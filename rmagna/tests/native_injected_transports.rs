use rmagna::{
    catalogue::{PhaseInjectionOptions, builtin_phase_catalogue},
    clifford::CliffordGate as C,
    gf2::BitVector,
    pauli::PauliWord,
    phase::{NativePhaseOptions, NativeTransportMode},
    scalar::{ExactScalar, ScalarArithmetic},
    unitary::{CompilerStrategy, PauliInsertion, UnitaryCircuit, UnitaryGate as G},
};

// A physical seven-wire parity-phase circuit. Only wires 0, 1, 2 receive
// final Hadamards; the other wires are traced by the probability query.
const AXES: [(usize, u8); 13] = [
    (2, 3),
    (3, 3),
    (11, 3),
    (15, 7),
    (18, 7),
    (19, 7),
    (21, 5),
    (33, 5),
    (50, 1),
    (52, 1),
    (56, 7),
    (57, 7),
    (64, 2),
];
type Coefficients = [i64; 4];
fn bits(width: usize, value: usize) -> BitVector {
    BitVector::from_indices(width, (0..width).filter(|j| value >> j & 1 != 0)).unwrap()
}
fn add(mut left: Coefficients, right: Coefficients) -> Coefficients {
    for j in 0..4 {
        left[j] += right[j];
    }
    left
}
fn omega(exponent: usize) -> Coefficients {
    let mut result = [0; 4];
    result[exponent % 4] = if exponent % 8 < 4 { 1 } else { -1 };
    result
}
fn norm(value: Coefficients) -> Coefficients {
    let conjugate = [value[0], -value[3], -value[2], -value[1]];
    let mut result = [0; 4];
    for j in 0..4 {
        for k in 0..4 {
            result[(j + k) % 4] += value[j] * conjugate[k] * if j + k < 4 { 1 } else { -1 };
        }
    }
    result
}
fn literal_mass(output: usize) -> Coefficients {
    // Seven initial and three final H gates give sqrt(2)^-10 in each
    // amplitude. Square the literal eight-path sum, then trace high bits.
    (0..16).fold([0; 4], |total, high| {
        let amplitude = (0..8).fold([0; 4], |sum, low| {
            let input = low | high << 3;
            let phase: usize = AXES
                .iter()
                .map(|&(mask, turns)| {
                    usize::from((input & mask).count_ones() % 2 != 0) * usize::from(turns)
                })
                .sum();
            add(sum, omega(phase + 4 * (low & output).count_ones() as usize))
        });
        add(total, norm(amplitude))
    })
}
fn circuit() -> UnitaryCircuit {
    let mut gates = (0..7).map(|q| G::Clifford(C::H(q))).collect::<Vec<_>>();
    for (mask, turns) in AXES {
        let pivot = mask.trailing_zeros() as usize;
        let controls = (0..7)
            .filter(|&q| q != pivot && mask >> q & 1 != 0)
            .collect::<Vec<_>>();
        for &control in &controls {
            gates.push(G::Clifford(C::Cx {
                control,
                target: pivot,
            }));
        }
        gates.push(G::T {
            qubit: pivot,
            turns,
        });
        for &control in controls.iter().rev() {
            gates.push(G::Clifford(C::Cx {
                control,
                target: pivot,
            }));
        }
    }
    gates.extend((0..3).map(|q| G::Clifford(C::H(q))));
    UnitaryCircuit::new(7, gates).unwrap()
}

#[test]
fn selected_injected_physical_resources_reuse_transport_for_all_three_output_faults() {
    let a = ScalarArithmetic::new(2048).unwrap();
    let circuit = circuit();
    let before_gate = circuit.gates().len();
    let reference = circuit
        .compile_with_strategy(&bits(7, 0), &a, CompilerStrategy::SharedCommutingLayer)
        .unwrap();
    drop(circuit);
    let rows = [bits(7, 1), bits(7, 2), bits(7, 4)];
    let database = Some(builtin_phase_catalogue().unwrap());
    let expected: [ExactScalar; 8] =
        std::array::from_fn(|y| a.scalar(literal_mass(y).map(Into::into), -10).unwrap());
    assert_eq!(
        expected
            .iter()
            .try_fold(ExactScalar::zero(), |sum, v| a.add(&sum, v))
            .unwrap(),
        ExactScalar::one()
    );
    let mut replayed = 0;
    for mode in ["off", "prepared", "term_skip", "geometry_skip", "work_skip"] {
        let mut options = NativePhaseOptions {
            injection: Some(PhaseInjectionOptions::default()),
            ..Default::default()
        };
        options.selection.max_quotient_rank = 12;
        options.selection.preparation.max_terms = 4096;
        options.selection.construction.max_verification_work = 1 << 32;
        options.transport.mode = if mode == "off" {
            NativeTransportMode::Off
        } else {
            NativeTransportMode::Prepared
        };
        match mode {
            "term_skip" => options.transport.limits.max_terms = 0,
            "geometry_skip" => options.transport.limits.max_geometry_words = 0,
            "work_skip" => options.transport.limits.max_construction_work = 0,
            _ => {}
        }
        let probability = reference
            .prepare_probability(&rows, &a)
            .unwrap()
            .with_native_phase_resource(4096, options, database, &a)
            .unwrap();
        let support = reference
            .prepare_probability_support_with_native(&rows, 4096, options, database, &a)
            .unwrap();
        for resource in [
            probability.fixed_phase_resource().unwrap(),
            support.fixed_phase_resource(),
        ] {
            assert!(
                resource
                    .native_resource()
                    .unwrap()
                    .injection_receipt()
                    .unwrap()
                    .selected,
                "fixture must actually select injection"
            );
            assert_eq!(
                resource.num_terms(),
                64,
                "transport admission must preserve the selected decomposition"
            );
            match (mode, resource.native_transport_receipt()) {
                ("off", None)
                | ("prepared", Some(rmagna::phase::NativeTransportReceipt::Prepared(_))) => {}
                (m, Some(rmagna::phase::NativeTransportReceipt::BudgetSkipped { .. }))
                    if m.ends_with("_skip") => {}
                _ => panic!("incorrect preparation receipt for {mode}"),
            }
        }
        for fault in 0..8 {
            let state = reference
                .with_insertions(
                    &[PauliInsertion {
                        before_gate,
                        word: PauliWord::new(bits(7, fault), bits(7, 0), 0).unwrap(),
                    }],
                    &a,
                )
                .unwrap();
            let expansion = support.evaluate_with_statistics(&state, 4096, &a).unwrap();
            assert_eq!(expansion.native_transport.is_some(), mode == "prepared");
            if let Some(stats) = expansion.native_transport {
                replayed += stats.gaussian_reuses;
            }
            for y in 0..8 {
                let actual = probability.evaluate(&state, &bits(3, y), 4096, &a).unwrap();
                assert_eq!(
                    actual.value,
                    expected[y ^ fault],
                    "mode={mode} fault={fault} output={y}"
                );
                assert_eq!(actual.native_transport.is_some(), mode == "prepared");
                if let Some(stats) = actual.native_transport {
                    replayed += stats.gaussian_reuses;
                }
                let mass = expansion
                    .states
                    .iter()
                    .fold(ExactScalar::zero(), |sum, term| {
                        a.add(
                            &sum,
                            &term.prepare().unwrap().amplitude(&bits(3, y), &a).unwrap(),
                        )
                        .unwrap()
                    });
                assert_eq!(mass, expected[y ^ fault], "labelled support mass");
            }
            assert!(
                probability.evaluate(&state, &bits(3, 0), 0, &a).is_err(),
                "query cap remains independent"
            );
            assert!(probability.evaluate(&state, &bits(2, 0), 4096, &a).is_err());
        }
    }
    assert!(
        replayed > 0,
        "the injected fixture must replay Gaussian work"
    );
}
