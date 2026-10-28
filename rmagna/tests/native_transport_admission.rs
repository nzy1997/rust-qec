use rmagna::{
    circuit::{ParseOptions, parse_circuit},
    clifford::CliffordGate as C,
    gf2::BitVector,
    instrument::{
        CompileOptions, InstrumentQueryLayout as Q, QueryPreparation, compile_affine,
        compile_unitary,
    },
    pauli::PauliWord,
    phase::{NativePhaseOptions, NativeTransportMode},
    scalar::{ExactScalar, ScalarArithmetic},
    unitary::{DensityLayout, PauliInsertion, UnitaryCircuit, UnitaryGate as G},
};
fn bits(width: usize, mask: usize) -> BitVector {
    BitVector::from_indices(width, (0..width).filter(|j| mask >> j & 1 != 0)).unwrap()
}
fn options() -> NativePhaseOptions {
    let mut options = NativePhaseOptions::default();
    options.transport.mode = NativeTransportMode::Prepared;
    options
}

#[test]
fn owned_transport_keeps_zero_outcomes_and_compatible_fault_updates_exact() {
    let arithmetic = ScalarArithmetic::default();
    let circuit = UnitaryCircuit::new(
        3,
        [
            G::Clifford(C::X(0)),
            G::Clifford(C::H(1)),
            G::T { qubit: 1, turns: 1 },
            G::Clifford(C::H(1)),
        ],
    )
    .unwrap();
    let before_gate = circuit.gates().len();
    let reference = circuit.compile(&bits(3, 0), &arithmetic).unwrap();
    let rows = [bits(3, 1)];
    let plan = reference
        .prepare_probability(&rows, &arithmetic)
        .unwrap()
        .with_native_phase_resource(256, options(), None, &arithmetic)
        .unwrap();
    let unchanged = reference.with_insertions(&[], &arithmetic).unwrap();
    let flipped = reference
        .with_insertions(
            &[PauliInsertion {
                before_gate,
                word: PauliWord::new(bits(3, 1), bits(3, 0), 0).unwrap(),
            }],
            &arithmetic,
        )
        .unwrap();
    let foreign = circuit.compile(&bits(3, 0), &arithmetic).unwrap();
    drop(reference);
    drop(circuit);
    for (state, expected_zero) in [(&unchanged, true), (&flipped, false)] {
        let zero = plan.evaluate(state, &bits(1, 0), 256, &arithmetic).unwrap();
        let one = plan.evaluate(state, &bits(1, 1), 256, &arithmetic).unwrap();
        assert_eq!(
            zero.value,
            if expected_zero {
                ExactScalar::zero()
            } else {
                ExactScalar::one()
            }
        );
        assert_eq!(
            one.value,
            if expected_zero {
                ExactScalar::one()
            } else {
                ExactScalar::zero()
            }
        );
        assert!(plan.evaluate(state, &bits(2, 0), 256, &arithmetic).is_err());
    }
    assert!(
        plan.evaluate(&foreign, &bits(1, 0), 256, &arithmetic)
            .is_err(),
        "program identity remains a hard error"
    );
    assert!(
        plan.evaluate(&unchanged, &bits(1, 1), 0, &arithmetic)
            .is_err(),
        "query term budget is not an optional acceleration budget"
    );
}

#[test]
fn query_cache_quotas_remain_hard_and_failed_preparations_publish_no_resource() {
    let arithmetic = ScalarArithmetic::default();
    let source = parse_circuit("H 0\nT 0\nH 0\nM 0\nI 1 2", &ParseOptions::default()).unwrap();
    let mode = QueryPreparation::Native(options());
    let compile_options = CompileOptions {
        probability_preparation: mode,
        density_preparation: mode,
        moment_preparation: mode,
        max_prepared_probabilities: 1,
        max_prepared_densities: 1,
        max_prepared_moments: 1,
        ..Default::default()
    };
    assert!(
        compile_affine(
            &source,
            &CompileOptions {
                max_prepared_probabilities: 0,
                ..compile_options
            },
            &arithmetic
        )
        .is_err(),
        "native probability cache construction requires a positive cap"
    );
    for zero in [
        CompileOptions {
            max_prepared_densities: 0,
            ..compile_options
        },
        CompileOptions {
            max_prepared_moments: 0,
            ..compile_options
        },
    ] {
        assert!(compile_affine(&source, &zero, &arithmetic).is_err());
    }
    let instrument = compile_affine(&source, &compile_options, &arithmetic).unwrap();
    let target = compile_unitary(
        &parse_circuit("I 0", &ParseOptions::default()).unwrap(),
        &CompileOptions::default(),
        &arithmetic,
    )
    .unwrap();
    let first_word = PauliWord::new(bits(1, 0), bits(1, 1), 0).unwrap();
    let word = PauliWord::new(bits(1, 1), bits(1, 0), 0).unwrap();
    let target_layout = DensityLayout {
        outputs: &[0],
        event_rows: &[],
    };
    for query in [
        Q::Acceptance,
        Q::PauliMoment {
            outputs: &[1],
            word: &first_word,
        },
        Q::TargetDensity {
            outputs: &[1],
            target: &target,
            target_layout,
        },
    ] {
        instrument.prepare_query(query, 256, &arithmetic).unwrap();
    }
    let before = [
        instrument.prepared_probability_statistics().unwrap(),
        instrument.prepared_density_statistics().unwrap(),
        instrument.prepared_moment_statistics().unwrap(),
    ];
    for statistics in &before {
        assert_eq!(statistics.cached_queries, 1);
        assert_eq!(statistics.native_transports.prepared, 1);
    }
    let records = [rmagna::circuit::BitExpr::atom(
        rmagna::circuit::BitAtom::Record(0),
    )];
    for query in [
        Q::ReportedProbability { parities: &records },
        Q::PauliMoment {
            outputs: &[1],
            word: &word,
        },
        Q::TargetDensity {
            outputs: &[2],
            target: &target,
            target_layout: DensityLayout {
                outputs: &[0],
                event_rows: &[],
            },
        },
    ] {
        let error = instrument
            .prepare_query(query, 256, &arithmetic)
            .unwrap_err()
            .to_string();
        assert!(error.contains("max_prepared_"), "{error}");
    }
    let after = [
        instrument.prepared_probability_statistics().unwrap(),
        instrument.prepared_density_statistics().unwrap(),
        instrument.prepared_moment_statistics().unwrap(),
    ];
    assert_eq!(
        after, before,
        "failed preparations must publish neither resources nor transport receipts"
    );
}
