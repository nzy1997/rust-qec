// SPDX-License-Identifier: MPL-2.0
// Original FileApiTests.test_supplied_noise_is_validated_against_the_circuit
// from itpyi/magna@56847478c840395735146ef1e1ccdc3d8aedc10f,
// tests/core/test_file_api.py, exercised through the Rust public task API.

use num_rational::BigRational;
use rmagna::circuit::{Basis, Operation, ParseOptions, parse_circuit};
use rmagna::field::ExactField;
use rmagna::noise::RandomSource;
use rmagna::runtime::ControllerRegistry;
use rmagna::scalar::ExactScalar;
use rmagna::task::{
    EvaluationOptions, SuppliedActionLimits, SuppliedNoiseActions, SuppliedPauliAction, TaskError,
    TaskEvaluator, TaskOptions, parse_task,
};
use rmagna::unitary::CompilerStrategy;
use std::path::Path;

const PHYSICAL: &str = "X_ERROR(1/10) 0\nM 0\nDETECTOR rec[-1]\nPOSTSELECT !det[0]\n";
const ORIGINAL_WRONG_WIRE: u64 = 1 << 32;

struct NoEntropy;

impl RandomSource for NoEntropy {
    fn next_u64(&mut self) -> u64 {
        panic!("the supplied-noise boundary case must not request entropy")
    }
}

fn x(anchor: usize, wire: usize) -> SuppliedPauliAction {
    SuppliedPauliAction {
        anchor,
        wire,
        basis: Basis::X,
    }
}

fn assert_noise_error(error: Option<TaskError>, expected: &str, compiler: CompilerStrategy) {
    let error = error.expect("the malformed supplied candidate must be rejected");
    assert_eq!(error.field, "noise", "{compiler:?}");
    assert!(error.message.contains(expected), "{compiler:?}: {error}");
}

#[test]
fn original_supplied_noise_is_validated_against_the_circuit() {
    let parse = ParseOptions::default();
    let physical = parse_circuit(PHYSICAL, &parse).unwrap();
    let original_operations = physical.operations().to_vec();
    assert_eq!(physical.num_qubits(), 1);
    assert_eq!(physical.num_records(), 1);
    assert!(matches!(
        original_operations[0].operation,
        Operation::Noise { .. }
    ));
    assert!(matches!(
        original_operations[1].operation,
        Operation::Measure { record: 0, .. }
    ));
    assert_eq!(original_operations[0].id, 0);
    assert_eq!(original_operations[1].id, 1);
    let modules = parse_task("{}", Path::new("."), &TaskOptions::default())
        .unwrap()
        .load_modules(&physical, &parse)
        .unwrap();
    let registry = ControllerRegistry::default();
    let unit = BigRational::from_integer(1.into());
    let zero = BigRational::from_integer(0.into());
    let physical_x = BigRational::new(1.into(), 10.into());

    for compiler in [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ] {
        let mut options = EvaluationOptions::default();
        options.runtime.compile.compiler_strategy = compiler;
        let prepare = |paulis: &[SuppliedPauliAction], report_flips: &[usize]| {
            TaskEvaluator::prepare_supplied_actions(
                &physical,
                &modules,
                &registry,
                options,
                SuppliedNoiseActions {
                    paulis,
                    report_flips,
                    likelihood: &unit,
                },
                SuppliedActionLimits::default(),
            )
        };

        // X makes the true record one. The supplied report flip changes it
        // to zero, while the original inverted detector requires report one.
        for (flips, raw, accepted, categorical_mass) in [
            (&[0][..], ExactScalar::zero(), ExactField::zero(), &zero),
            (&[][..], ExactScalar::one(), ExactField::one(), &physical_x),
        ] {
            let draw = prepare(&[x(0, 0)], flips)
                .unwrap()
                .sample(&mut NoEntropy)
                .unwrap();
            assert_eq!(draw.raw_acceptance, raw, "{compiler:?} {flips:?}");
            assert_eq!(draw.accepted, accepted, "{compiler:?} {flips:?}");
            assert_eq!(draw.raw_error, ExactScalar::zero(), "{compiler:?}");
            assert_eq!(draw.error, ExactField::zero(), "{compiler:?}");
            assert_eq!(draw.history_mass, ExactScalar::one(), "{compiler:?}");
            assert_eq!(draw.history_proposal_mass, None, "{compiler:?}");
            assert_eq!(draw.history_weight, ExactField::one(), "{compiler:?}");
            assert_eq!(draw.noise_likelihood_ratio, unit, "{compiler:?}");
            // The Rust model reports physical mass for the assignment. The
            // supplied native response does not expose that categorical field.
            assert_eq!(&draw.noise_probability, categorical_mass, "{compiler:?}");
            assert!(draw.sampled_records.is_empty(), "{compiler:?}");
        }

        for anchor in [9, 1] {
            assert_noise_error(
                prepare(&[x(anchor, 0)], &[]).err(),
                &format!("unknown supplied noise anchor {anchor}"),
                compiler,
            );
        }
        match usize::try_from(ORIGINAL_WRONG_WIRE) {
            Ok(wire) => assert_noise_error(
                prepare(&[x(0, wire)], &[]).err(),
                &format!("supplied anchor 0 does not target wire {wire}"),
                compiler,
            ),
            Err(_) => {
                // The original wire is rejected at conversion on narrower hosts.
                let wire_max = u64::try_from(usize::MAX).unwrap_or(u64::MAX);
                assert!(ORIGINAL_WRONG_WIRE > wire_max, "{compiler:?}");
            }
        }
        assert_noise_error(
            prepare(&[], &[1]).err(),
            "unknown supplied report record 1",
            compiler,
        );
        assert_eq!(physical.to_text(), PHYSICAL, "{compiler:?}");
        assert_eq!(physical.operations(), original_operations, "{compiler:?}");
    }
}
