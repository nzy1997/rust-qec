// SPDX-License-Identifier: MPL-2.0
// Complete KernelContractTests.test_options_change_revalidates_resource_budget,
// itpyi/magna@56847478c840395735146ef1e1ccdc3d8aedc10f/tests/core/test_kernel_contract.py.
use rmagna::gf2::BitVector;
use rmagna::instrument::{
    OriginalAssignment, OriginalError, OriginalGate as G, OriginalInstrument, OriginalLimits,
    OriginalNetworkResource, OriginalOperation, OriginalSourceCertificateOptions, ReadoutBasis,
};
use rmagna::phase::{NativePhaseOptions, PhaseResourceKind};
use rmagna::quadratic::QuadraticForm;
use rmagna::scalar::{ExactScalar, ScalarArithmetic};
use rmagna::stabilizer::AffineKet;
use rmagna::unitary::CompilerStrategy;
fn bits(width: usize, value: usize) -> BitVector {
    BitVector::from_indices(width, (0..width).filter(|j| value >> j & 1 != 0)).unwrap()
}
fn sum(states: &[AffineKet], a: &ScalarArithmetic) -> ExactScalar {
    states.iter().fold(ExactScalar::zero(), |v, k| {
        a.add(&v, &k.prepare().unwrap().amplitude(&bits(0, 0), a).unwrap())
            .unwrap()
    })
}
#[test]
fn original_changed_source_budget_rejects_stale_resource_without_projected_absorption() {
    let a = ScalarArithmetic::default();
    let initial = AffineKet::new(
        bits(1, 0),
        vec![bits(1, 1)],
        QuadraticForm::new(0, vec![0], vec![bits(1, 0)]).unwrap(),
        a.omega(0, -1).unwrap(),
    )
    .unwrap();
    let ir = OriginalInstrument::new(
        vec![0],
        vec![],
        initial,
        vec![
            OriginalOperation {
                id: "t".into(),
                gate: G::Phase {
                    wires: vec![0],
                    turns: 1,
                },
            },
            OriginalOperation {
                id: "m".into(),
                gate: G::Destructive {
                    wire: 0,
                    basis: ReadoutBasis::X,
                    record: "m".into(),
                },
            },
        ],
        vec![],
        OriginalLimits::default(),
        &a,
    )
    .unwrap();
    let assignment = OriginalAssignment {
        true_values: vec![("m".into(), false)],
        ..Default::default()
    };
    // Literal H/T/X projection gives (1+omega)/2 and (2+sqrt(2))/4.
    let amplitude = a
        .add(&a.omega(0, -2).unwrap(), &a.omega(1, -2).unwrap())
        .unwrap();
    let probability = a
        .scalar([2.into(), 1.into(), 0.into(), (-1).into()], -2)
        .unwrap();
    for compiler in [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ] {
        let branch = ir.compile(&assignment, compiler, &a).unwrap();
        let origins = branch.origins().to_vec();
        let records = branch.records().to_vec();
        let diagnostics = branch.compilation().clone();
        let mut options = OriginalSourceCertificateOptions {
            resource: OriginalNetworkResource::Native(NativePhaseOptions::default()),
            max_quotient_rank: 1,
            max_terms: 2,
            ..Default::default()
        };
        options.completed.max_pairs = 4;
        let valid = branch.compile_before_projection(&options, &a).unwrap();
        assert_eq!(valid.resource().kind(), PhaseResourceKind::NativeSelection);
        assert!(valid.resource().native_resource().is_some());
        assert_eq!(valid.resource().quotient_rank(), 1);
        assert_eq!(valid.statistics().quotient_rank, 1);
        assert_eq!(valid.statistics().certified_terms, 2);
        assert_eq!(valid.statistics().side_entries, valid.states().len());
        assert!((1..=2).contains(&valid.states().len()));
        assert_eq!(
            valid.statistics().probability.pair_work,
            valid.states().len().pow(2)
        );
        assert!(valid.statistics().probability.pair_work <= 4);
        assert_eq!(sum(valid.states(), &a), amplitude);
        assert_eq!(valid.probability(), &probability);
        assert_eq!(branch.amplitude(&bits(0, 0), 2, &a).unwrap(), amplitude);
        // The same owned branch is recompiled using a new immutable rank ceiling;
        // post-projection absorption cannot hide its rank-one SOURCE resource.
        let rejected = OriginalSourceCertificateOptions {
            max_quotient_rank: 0,
            ..options
        };
        assert!(matches!(
            branch.compile_before_projection(&rejected, &a),
            Err(OriginalError::ResourceLimit {
                option: "source.max_quotient_rank",
                limit: 0,
                required: 1
            })
        ));
        assert_eq!(sum(valid.states(), &a), amplitude);
        assert_eq!(valid.probability(), &probability);
        assert_eq!(valid.resource().kind(), PhaseResourceKind::NativeSelection);
        let mut native = NativePhaseOptions::default();
        native.selection.max_quotient_rank = 0;
        let native_refused = OriginalSourceCertificateOptions {
            resource: OriginalNetworkResource::Native(native),
            ..options
        };
        let refusal = branch.compile_before_projection(&native_refused, &a);
        assert!(refusal.is_err());
        assert!(
            refusal
                .err()
                .unwrap()
                .to_string()
                .contains("max_quotient_rank")
        );
        assert_eq!(valid.resource().kind(), PhaseResourceKind::NativeSelection);
        let restored = branch.compile_before_projection(&options, &a).unwrap();
        assert_eq!(restored.statistics(), valid.statistics());
        assert_eq!(restored.states(), valid.states());
        assert_eq!(restored.probability(), valid.probability());
        assert_eq!(branch.origins(), origins);
        assert_eq!(branch.records(), records);
        assert_eq!(branch.compilation(), &diagnostics);
        assert!(branch.environment().is_empty());
        assert!(branch.outputs().is_empty());
        // Each structural/resource refusal leaves both earlier certificates usable.
        let mut term = options;
        term.max_terms = 1;
        let mut pairs = options;
        pairs.completed.max_pairs = 3;
        let mut paths = options;
        paths.max_raw_path_bits = 0;
        let mut rows = options;
        rows.max_projection_rows = 0;
        let mut geometry = options;
        geometry.max_geometry_words = 0;
        for refused in [term, pairs, paths, rows, geometry] {
            assert!(branch.compile_before_projection(&refused, &a).is_err());
            assert_eq!(sum(valid.states(), &a), amplitude);
            assert_eq!(restored.probability(), &probability);
        }
    }
}
