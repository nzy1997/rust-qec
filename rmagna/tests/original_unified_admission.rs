// SPDX-License-Identifier: MPL-2.0
// Complete UnifiedTests.test_initial_path_budget_and_invalid_gate_lifetimes,
// itpyi/magna@56847478c840395735146ef1e1ccdc3d8aedc10f/tests/core/test_unified_simulator.py.
use rmagna::circuit::{Gate, Operation, OperationProgram, OperationProgramErrorKind};
use rmagna::clifford::CliffordCircuit;
use rmagna::gf2::BitVector;
use rmagna::instrument::{
    OriginalAssignment, OriginalBranch, OriginalError, OriginalGate as G, OriginalInstrument,
    OriginalLimits, OriginalOperation,
};
use rmagna::quadratic::QuadraticForm;
use rmagna::scalar::{ExactScalar, ScalarArithmetic};
use rmagna::stabilizer::AffineKet;
use rmagna::unitary::{CompilerStrategy, UnitaryError};
fn bits(width: usize, word: usize) -> BitVector {
    BitVector::from_indices(width, (0..width).filter(|j| word >> j & 1 != 0)).unwrap()
}
fn plus(a: &ScalarArithmetic) -> AffineKet {
    AffineKet::new(
        bits(2, 0),
        vec![bits(2, 1), bits(2, 2)],
        QuadraticForm::new(0, vec![0; 2], vec![bits(2, 0); 2]).unwrap(),
        a.omega(0, -2).unwrap(),
    )
    .unwrap()
}
fn op(id: &str, gate: G) -> OriginalOperation {
    OriginalOperation {
        id: id.into(),
        gate,
    }
}
fn check(branch: &OriginalBranch, a: &ScalarArithmetic) {
    assert_eq!(branch.outputs(), &[0, 1]);
    assert!(branch.environment().is_empty());
    assert!(branch.records().is_empty());
    let terms = branch.expansion(a).unwrap();
    for word in 0..4 {
        let expected = a.omega(0, -2).unwrap();
        assert_eq!(branch.amplitude(&bits(2, word), 4, a).unwrap(), expected);
        assert_eq!(
            terms.iter().fold(ExactScalar::zero(), |v, k| a
                .add(
                    &v,
                    &k.prepare().unwrap().amplitude(&bits(2, word), a).unwrap()
                )
                .unwrap()),
            expected
        );
    }
    assert_eq!(branch.probability(a).unwrap(), ExactScalar::one());
}
#[test]
fn original_initial_path_budget_and_invalid_gate_lifetimes_leave_valid_branch_usable() {
    let a = ScalarArithmetic::default();
    for compiler in [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ] {
        let valid = OriginalInstrument::new(
            vec![0, 1],
            vec![0, 1],
            plus(&a),
            vec![],
            vec![],
            OriginalLimits {
                max_path_bits: 2,
                ..Default::default()
            },
            &a,
        )
        .unwrap();
        let saved = valid
            .compile(&OriginalAssignment::default(), compiler, &a)
            .unwrap();
        check(&saved, &a);
        let rejected = OriginalInstrument::new(
            vec![0, 1],
            vec![0, 1],
            plus(&a),
            vec![],
            vec![],
            OriginalLimits {
                max_path_bits: 1,
                ..Default::default()
            },
            &a,
        );
        assert!(matches!(
            rejected,
            Err(OriginalError::ResourceLimit {
                option: "max_path_bits",
                limit: 1,
                required: 2
            })
        ));
        check(&saved, &a);
        // Starting from zero admits construction; the two lowered H paths must
        // still be charged during compilation, with no returned branch on error.
        let lower = OriginalInstrument::new(
            vec![0, 1],
            vec![0, 1],
            CliffordCircuit::computational(bits(2, 0)).unwrap(),
            vec![op("h0", G::H(0)), op("h1", G::H(1))],
            vec![],
            OriginalLimits {
                max_path_bits: 1,
                ..Default::default()
            },
            &a,
        )
        .unwrap();
        assert!(matches!(
            lower.compile(&OriginalAssignment::default(), compiler, &a),
            Err(OriginalError::Unitary(UnitaryError::RawPathLimit {
                limit: 1,
                required: 2
            }))
        ));
        check(&saved, &a);
        // OriginalGate::Cx has statically fixed arity. The checked owned input
        // API is the runtime boundary for a source CX with three targets.
        assert!(
            matches!(OperationProgram::new(3,vec![Operation::Unitary{gate:Gate::Cx,qubits:vec![0,1,2],control:None}]),Err(error)if error.operation==Some(0)&&error.kind==OperationProgramErrorKind::GateArity{gate:Gate::Cx,expected:2,actual:3})
        );
        check(&saved, &a);
        let inactive = OriginalInstrument::new(
            vec![0, 1, 2],
            vec![0, 1, 2],
            CliffordCircuit::computational(bits(3, 0)).unwrap(),
            vec![op(
                "reset",
                G::Reset {
                    wire: 9,
                    plus: false,
                },
            )],
            vec![],
            OriginalLimits::default(),
            &a,
        )
        .unwrap();
        assert!(matches!(
            inactive.compile(&OriginalAssignment::default(), compiler, &a),
            Err(OriginalError::Lifetime {
                wire: 9,
                expected_live: true
            })
        ));
        check(&saved, &a);
        check(
            &valid
                .compile(&OriginalAssignment::default(), compiler, &a)
                .unwrap(),
            &a,
        );
    }
}
