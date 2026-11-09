// SPDX-License-Identifier: MPL-2.0
// Complete semantic contract from itpyi/magna 56847478c840395735146ef1e1ccdc3d8aedc10f,
// tests/core/test_nonlinear_postselection.py, SHA256
// 76becb6b2146a1c7bcee326b3a7daab1aac5de12390f81887175d417d07351e8,
// test_not_and_xor_constant_use_zero_projection_template.
// Rust uses a direct parity event, with its own wire layout and random stream.
use super::*;
use crate::circuit::{BitAtom, BitExpr, Operation, ParseOptions, RegisterVersion, parse_circuit};
use crate::costs::{ExecutionLimits, ExecutionMeter};
use crate::noise::{NoiseSampling, OriginalNoiseOptions, SplitMix64, original_noise_categories};
use crate::runtime::{CausalProgram, CausalStrategy, ControllerRegistry, RuntimeOptions};
use crate::unitary::CompilerStrategy;
use std::sync::atomic::{AtomicUsize, Ordering};

#[test]
fn original_not_and_xor_constant_use_zero_projection_template() {
    let a = ScalarArithmetic::default();
    let empty = BitVector::zeros(0).unwrap();
    let zero = BitVector::zeros(1).unwrap();
    let max_terms = 1_000_000;
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&calls);
    let mut registry = ControllerRegistry::default();
    registry
        .register("echo", "1", move |bits: &[bool]| {
            assert_eq!(bits, &[false], "actual deterministic echo input");
            observed.fetch_add(1, Ordering::Relaxed);
            Ok(bits.to_vec())
        })
        .unwrap();
    let parse = ParseOptions {
        controllers: registry.names(),
        ..Default::default()
    };
    let complement = BitExpr::atom(BitAtom::Decision { call: 0, bit: 0 }).inverted();
    let target_circuit = parse_circuit("I 0", &ParseOptions::default()).unwrap();
    for suffix in ["CONTROL[NOT]", "CONTROL[XOR](1)"] {
        let text = format!(
            "M 0\nDETECTOR rec[-1]\nCONTROL[echo] det[-1] -> ctrl[0]\n{suffix} ctrl[0] -> ctrl[1]\nPOSTSELECT ctrl[1]"
        );
        let circuit = parse_circuit(&text, &parse).unwrap();
        assert_eq!(circuit.num_qubits(), 1);
        assert_eq!(circuit.num_records(), 1);
        assert_eq!(
            circuit
                .control_analysis()
                .expression(RegisterVersion::new(1, 0)),
            Some(&complement)
        );
        assert!(
            matches!(&circuit.operations().last().unwrap().operation, Operation::Postselect { expression } if expression == &complement)
        );
        assert!(
            original_noise_categories(&circuit, &OriginalNoiseOptions::default())
                .unwrap()
                .is_empty()
        );
        for compiler in [
            CompilerStrategy::ParityCopies,
            CompilerStrategy::SharedCommutingLayer,
        ] {
            let options = CompileOptions {
                compiler_strategy: compiler,
                probability_preparation: QueryPreparation::Trace,
                ..Default::default()
            };
            let baseline = compile_causal_stabilizer(
                &circuit,
                &zero_input(1, &options).unwrap(),
                &options,
                &a,
            )
            .unwrap();
            assert_eq!(
                baseline.noise_sites().len(),
                1,
                "implicit internal readout slot"
            );
            assert!(baseline.noise_sites().iter().all(|site| {
                site.identity_probability().value() == &BigRational::from_integer(1.into())
                    && site.alternatives().iter().all(|alternative| {
                        alternative.probability().value() == &BigRational::from_integer(0.into())
                    })
            }));
            assert_eq!(baseline.decision_wires.len(), 1);
            let row = BitVector::from_indices(
                baseline.coherent_qubits(),
                [baseline.decision_wires[&(0, 0)]],
            )
            .unwrap();
            assert_eq!(baseline.event_rows.as_ref(), std::slice::from_ref(&row));
            assert_eq!(
                baseline.outcomes,
                BitVector::from_indices(1, [0]).unwrap(),
                "complemented selection requires decision one"
            );
            assert_eq!(
                baseline
                    .probability(std::slice::from_ref(&row), &zero, &[], max_terms, &a)
                    .unwrap()
                    .value,
                ExactScalar::one(),
                "unassigned raw decision is zero with unit mass"
            );
            assert_eq!(
                baseline
                    .prepare_query(InstrumentQueryLayout::Acceptance, max_terms, &a)
                    .unwrap(),
                QueryPreparationOutcome::Reused
            );
            for decision in [false, true] {
                let decisions = BTreeMap::from([((0, 0), decision)]);
                let view = baseline.branch(&decisions, &BTreeSet::new()).unwrap();
                assert!(Arc::ptr_eq(&view.unitary, &baseline.unitary));
                assert!(Arc::ptr_eq(
                    &view.probability_queries,
                    &baseline.probability_queries
                ));
                assert_eq!(
                    view.prepare_query(InstrumentQueryLayout::Acceptance, max_terms, &a)
                        .unwrap(),
                    QueryPreparationOutcome::Reused
                );
                assert_eq!(
                    view.probability(&[], &empty, &[], max_terms, &a)
                        .unwrap()
                        .value,
                    ExactScalar::one()
                );
                assert_eq!(
                    view.acceptance(&[], max_terms, &a).unwrap().value,
                    if decision {
                        ExactScalar::one()
                    } else {
                        ExactScalar::zero()
                    },
                    "explicit branch zero-projection mass for decision={decision}"
                );
            }
            assert!(baseline.initial_insertions.is_empty());
            assert_eq!(
                baseline.acceptance(&[], max_terms, &a).unwrap().value,
                ExactScalar::zero()
            );
            let target = compile_unitary(&target_circuit, &options, &a).unwrap();
            let meter = ExecutionMeter::new(ExecutionLimits::default());
            let runtime = RuntimeOptions {
                strategy: CausalStrategy::FixedTemplate,
                compile: options,
                ..Default::default()
            };
            let program = CausalProgram::new_observed(
                &circuit,
                &registry,
                runtime,
                &a,
                &NoiseSampling::Physical,
                Some(&meter),
            )
            .unwrap();
            assert_eq!(program.strategy(), CausalStrategy::FixedTemplate);
            assert_eq!(program.reference_compilations(), 1);
            let before = meter.snapshot().compilation;
            assert_eq!(
                (before.attempted, before.succeeded, before.failed),
                (1, 1, 0)
            );
            let calls_before = calls.load(Ordering::Relaxed);
            let draw = program
                .sample(&[0], &target, &mut SplitMix64::new(0), &a)
                .unwrap();
            assert_eq!(draw.network_compilations, 0);
            assert_eq!(draw.history_mass, ExactScalar::one());
            assert_eq!(draw.masses.accepted.value, ExactScalar::zero());
            assert_eq!(draw.controller_calls.len(), 1);
            let call = &draw.controller_calls[0];
            assert_eq!(call.name, "echo");
            assert_eq!(call.call, 0);
            assert_eq!(call.inputs, vec![false]);
            assert_eq!(call.outputs, vec![false]);
            assert_eq!(calls.load(Ordering::Relaxed) - calls_before, 1);
            assert_eq!(
                meter.snapshot().compilation,
                before,
                "fixed route needs no additional network compilation"
            );
        }
    }
}
