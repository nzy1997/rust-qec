// SPDX-License-Identifier: MPL-2.0
// Complete semantic contract from itpyi/magna 56847478c840395735146ef1e1ccdc3d8aedc10f,
// tests/core/test_nonlinear_postselection.py, SHA256
// 76becb6b2146a1c7bcee326b3a7daab1aac5de12390f81887175d417d07351e8,
// test_fixed_template_assignments_and_private_boundaries.
// Rust owns its wire layout, cache identities, and random stream.
use super::*;
use crate::circuit::{ParseOptions, parse_circuit};
use crate::costs::{ExecutionLimits, ExecutionMeter};
use crate::noise::{NoiseSampling, OriginalNoiseOptions, SplitMix64, original_noise_categories};
use crate::runtime::{CausalStrategy, ControllerRegistry};
use crate::task::{EvaluationOptions, TaskEvaluator, TaskOptions, parse_task};
use crate::unitary::{CompilerStrategy, DensityLayout};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

const CIRCUIT: &str = "H 0 1 2\nM 0 1\nDETECTOR rec[-2]\nDETECTOR rec[-1]\nCONTROL[and] det[-2] det[-1] -> ctrl[0]\nCH ctrl[0] 2\nCONTROL[XOR] ctrl[0] rec[-1] -> ctrl[1]\nPOSTSELECT ctrl[1]";

struct Files(PathBuf);
impl Files {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "rmagna-original-fixed-selection-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        let files = Self(path);
        std::fs::write(files.0.join("plus.magna"), "H 0").unwrap();
        files
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn dyadic(numerator: i32, exponent: i64, a: &ScalarArithmetic) -> ExactScalar {
    a.scalar([numerator.into(), 0.into(), 0.into(), 0.into()], exponent)
        .unwrap()
}

#[test]
fn original_fixed_template_assignments_and_private_boundaries() {
    let a = ScalarArithmetic::default();
    let half = dyadic(1, -1, &a);
    let quarter = dyadic(1, -2, &a);
    let empty = BitVector::zeros(0).unwrap();
    let max_terms = 1_000_000;
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&calls);
    let mut registry = ControllerRegistry::default();
    registry
        .register("and", "1", move |bits: &[bool]| {
            assert_eq!(bits.len(), 2);
            observed.fetch_add(1, Ordering::Relaxed);
            Ok(vec![bits[0] & bits[1]])
        })
        .unwrap();
    let parse = ParseOptions {
        controllers: registry.names(),
        ..Default::default()
    };
    let circuit = parse_circuit(CIRCUIT, &parse).unwrap();
    let target_circuit = parse_circuit("H 0", &ParseOptions::default()).unwrap();
    assert_eq!(circuit.num_records(), 2);
    assert_eq!(circuit.num_qubits(), 3);
    // Physical declarations and implicit coherent readout slots are separate APIs.
    assert!(
        original_noise_categories(&circuit, &OriginalNoiseOptions::default())
            .unwrap()
            .is_empty()
    );
    let files = Files::new();
    let task = parse_task(
        r#"{"outputs":[2],"ideal_state":"plus.magna","observables":[[1,0],[0,1]]}"#,
        &files.0,
        &TaskOptions::default(),
    )
    .unwrap();
    let modules = task.load_modules(&circuit, &parse).unwrap();

    for compiler in [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ] {
        let options = CompileOptions {
            compiler_strategy: compiler,
            probability_preparation: QueryPreparation::Trace,
            density_preparation: QueryPreparation::Trace,
            moment_preparation: QueryPreparation::Trace,
            ..Default::default()
        };
        let baseline =
            compile_causal_stabilizer(&circuit, &zero_input(3, &options).unwrap(), &options, &a)
                .unwrap();
        let target = compile_unitary(&target_circuit, &options, &a).unwrap();
        assert_eq!(baseline.num_records(), 2);
        assert_eq!(baseline.num_qubits(), 3);
        assert_eq!(
            baseline.noise_sites().len(),
            2,
            "implicit deterministic readout slots"
        );
        assert!(baseline.noise_sites().iter().all(|site| {
            site.identity_probability().value() == &BigRational::from_integer(1.into())
                && site
                    .alternatives()
                    .iter()
                    .all(|alt| alt.probability().value() == &BigRational::from_integer(0.into()))
        }));
        assert_eq!(
            baseline.decision_wires.len(),
            1,
            "one external call, one decision bit"
        );
        let feedback = baseline.decision_wires[&(0, 0)];
        let selection = BitVector::from_indices(
            baseline.coherent_qubits(),
            [feedback, baseline.record_wires[1]],
        )
        .unwrap();
        assert_eq!(
            baseline.event_rows.as_ref(),
            std::slice::from_ref(&selection)
        );
        assert_ne!(
            selection,
            BitVector::from_indices(baseline.coherent_qubits(), [feedback]).unwrap(),
            "final selection includes the second physical record"
        );
        let rows = [
            BitVector::from_indices(baseline.coherent_qubits(), [baseline.record_wires[0]])
                .unwrap(),
            BitVector::from_indices(baseline.coherent_qubits(), [baseline.record_wires[1]])
                .unwrap(),
            selection,
        ];
        let target_layout = DensityLayout {
            outputs: &[0],
            event_rows: &[],
        };
        assert_eq!(
            baseline
                .prepare_query(InstrumentQueryLayout::Acceptance, max_terms, &a)
                .unwrap(),
            QueryPreparationOutcome::Created
        );
        assert_eq!(
            baseline
                .prepare_query(
                    InstrumentQueryLayout::TargetDensity {
                        outputs: &[2],
                        target: &target,
                        target_layout
                    },
                    max_terms,
                    &a
                )
                .unwrap(),
            QueryPreparationOutcome::Created
        );
        let x = PauliWord::new(
            BitVector::from_indices(1, [0]).unwrap(),
            BitVector::zeros(1).unwrap(),
            0,
        )
        .unwrap();
        let z = PauliWord::new(
            BitVector::zeros(1).unwrap(),
            BitVector::from_indices(1, [0]).unwrap(),
            0,
        )
        .unwrap();
        for assignment in [None, Some(false), Some(true)] {
            let decisions = assignment
                .map(|value| ((0, 0), value))
                .into_iter()
                .collect();
            let view = baseline.branch(&decisions, &BTreeSet::new()).unwrap();
            assert!(
                Arc::ptr_eq(&view.unitary, &baseline.unitary),
                "immutable compiled program reused"
            );
            assert!(Arc::ptr_eq(
                &view.probability_queries,
                &baseline.probability_queries
            ));
            assert!(Arc::ptr_eq(
                &view.density_queries,
                &baseline.density_queries
            ));
            assert_eq!(
                view.prepare_query(InstrumentQueryLayout::Acceptance, max_terms, &a)
                    .unwrap(),
                QueryPreparationOutcome::Reused
            );
            assert_eq!(
                view.prepare_query(
                    InstrumentQueryLayout::TargetDensity {
                        outputs: &[2],
                        target: &target,
                        target_layout
                    },
                    max_terms,
                    &a
                )
                .unwrap(),
                QueryPreparationOutcome::Reused
            );
            assert_eq!(
                view.probability(&[], &empty, &[], max_terms, &a)
                    .unwrap()
                    .value,
                ExactScalar::one(),
                "unconditional physical mass"
            );
            for first in [false, true] {
                for second in [false, true] {
                    for selected in [false, true] {
                        let outcomes = BitVector::from_indices(
                            3,
                            [first, second, selected]
                                .into_iter()
                                .enumerate()
                                .filter_map(|(j, v)| v.then_some(j)),
                        )
                        .unwrap();
                        let expected = if selected == (assignment.unwrap_or(false) ^ second) {
                            quarter.clone()
                        } else {
                            ExactScalar::zero()
                        };
                        assert_eq!(
                            view.probability(&rows, &outcomes, &[], max_terms, &a)
                                .unwrap()
                                .value,
                            expected,
                            "joint event mass for assignment={assignment:?}, first={first}, second={second}, selected={selected}"
                        );
                    }
                }
            }
            let one = assignment.unwrap_or(false);
            assert_eq!(view.acceptance(&[], max_terms, &a).unwrap().value, half);
            assert_eq!(
                view.pauli_moment(&[2], &x, &[], max_terms, &a)
                    .unwrap()
                    .value,
                if one {
                    ExactScalar::zero()
                } else {
                    half.clone()
                },
                "accepted X moment distinguishes feedback assignment"
            );
            assert_eq!(
                view.pauli_moment(&[2], &z, &[], max_terms, &a)
                    .unwrap()
                    .value,
                if one {
                    half.clone()
                } else {
                    ExactScalar::zero()
                },
                "accepted Z moment distinguishes feedback assignment"
            );
            assert_eq!(
                view.masses(&[2], &target, &[], max_terms, &a)
                    .unwrap()
                    .target_success
                    .value,
                if one { quarter.clone() } else { half.clone() }
            );
            assert_eq!(
                baseline
                    .prepared_probability_statistics()
                    .unwrap()
                    .cached_queries,
                3
            );
            assert_eq!(
                baseline
                    .prepared_density_statistics()
                    .unwrap()
                    .cached_queries,
                1
            );
        }
        assert_eq!(
            baseline.initial_insertions.len(),
            0,
            "assignment views do not mutate baseline"
        );
        assert_eq!(baseline.acceptance(&[], max_terms, &a).unwrap().value, half);

        let meter = ExecutionMeter::new(ExecutionLimits::default());
        let mut evaluation = EvaluationOptions::default();
        evaluation.runtime.strategy = CausalStrategy::FixedTemplate;
        evaluation.runtime.compile = options;
        let evaluator = TaskEvaluator::new_with_costs(
            &circuit,
            &modules,
            &registry,
            evaluation,
            &NoiseSampling::default(),
            &meter,
        )
        .unwrap();
        assert_eq!(evaluator.execution_route(), "causal_fixed_template");
        let before = meter.snapshot().compilation;
        assert_eq!(
            (before.attempted, before.succeeded, before.failed),
            (2, 2, 0),
            "one fixed template plus one ideal-state compilation"
        );
        let calls_before = calls.load(Ordering::Relaxed);
        for seed in 0..8 {
            let draw = evaluator
                .sample_with_costs(&mut SplitMix64::new(seed), &meter)
                .unwrap();
            assert_eq!(draw.network_compilations, 0);
            assert_eq!(draw.history_mass, quarter);
            assert_eq!(draw.controller_calls.len(), 1);
            let call = &draw.controller_calls[0];
            assert_eq!(call.name, "and");
            assert_eq!(call.inputs.len(), 2);
            let d = call.inputs[0] & call.inputs[1];
            assert_eq!(call.outputs, vec![d]);
            let accepted = !(d ^ call.inputs[1]);
            assert_eq!(
                draw.accepted,
                if accepted {
                    crate::field::ExactField::one()
                } else {
                    crate::field::ExactField::zero()
                }
            );
            assert_eq!(
                draw.raw_moments,
                vec![
                    if accepted && !d {
                        quarter.clone()
                    } else {
                        ExactScalar::zero()
                    },
                    if accepted && d {
                        quarter.clone()
                    } else {
                        ExactScalar::zero()
                    }
                ]
            );
            assert_eq!(draw.target_normalization, Some(ExactScalar::one()));
            assert_eq!(
                meter.snapshot().compilation,
                before,
                "eight draws reuse both compiled programs"
            );
        }
        assert_eq!(calls.load(Ordering::Relaxed) - calls_before, 8);
    }
}
