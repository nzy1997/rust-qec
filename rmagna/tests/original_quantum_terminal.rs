// SPDX-License-Identifier: MPL-2.0
// Complete QuantumOutputTests.test_unsampled_quantum_output_is_terminal_conditional_expectation,
// itpyi/magna@56847478c840395735146ef1e1ccdc3d8aedc10f/tests/core/test_quantum_output.py.
// Explicit typed output zero and independently seeded Rust streams translate the
// original interface. Raw Born numerators differ from normalized contributions.
use rmagna::circuit::{ParseOptions, parse_circuit};
use rmagna::field::{ExactField, FieldArithmetic};
use rmagna::noise::SplitMix64;
use rmagna::runtime::{ControllerRegistry, HistorySampler};
use rmagna::scalar::{ExactScalar, ScalarArithmetic};
use rmagna::task::{EvaluationOptions, TaskEvaluator, TaskOptions, parse_task};
use rmagna::unitary::CompilerStrategy;
use std::{
    cmp::Ordering,
    collections::BTreeMap,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering as AtomicOrdering},
};
struct Files(PathBuf);
impl Files {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let p = std::env::temp_dir().join(format!(
            "rmagna-original-quantum-terminal-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, AtomicOrdering::Relaxed)
        ));
        std::fs::create_dir(&p).unwrap();
        std::fs::write(p.join("plus.magna"), "H 0").unwrap();
        Self(p)
    }
    fn evaluator(&self, measured: bool, compiler: CompilerStrategy) -> TaskEvaluator {
        let parse = ParseOptions::default();
        let source = if measured {
            "H 0\nT 0\nMX 0"
        } else {
            "H 0\nT 0"
        };
        let source = parse_circuit(source, &parse).unwrap();
        let task = if measured {
            r#"{"outputs":[0],"sample_records":[0],"ideal_state":"plus.magna"}"#
        } else {
            r#"{"outputs":[0],"ideal_state":"plus.magna"}"#
        };
        let modules = parse_task(task, &self.0, &TaskOptions::default())
            .unwrap()
            .load_modules(&source, &parse)
            .unwrap();
        let mut options = EvaluationOptions::default();
        options.runtime.history_sampler = HistorySampler::ExactMarginal;
        options.runtime.compile.compiler_strategy = compiler;
        TaskEvaluator::new(&source, &modules, &ControllerRegistry::default(), options).unwrap()
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
#[test]
fn original_unsampled_quantum_output_is_terminal_conditional_expectation() {
    let files = Files::new();
    let a = ScalarArithmetic::default();
    let f = FieldArithmetic::default();
    // Independent literals (2 +/- sqrt(2))/4, with sqrt(2)=omega-omega^3.
    let masses = [
        a.scalar([2, 1, 0, -1].map(Into::into), -2).unwrap(),
        a.scalar([2, -1, 0, 1].map(Into::into), -2).unwrap(),
    ];
    for compiler in [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ] {
        let integrated = files
            .evaluator(false, compiler)
            .sample(&mut SplitMix64::new(1))
            .unwrap();
        assert!(integrated.sampled_records.is_empty());
        assert_eq!(integrated.raw_error, masses[1]);
        assert_eq!(integrated.error, f.from_scalar(&masses[1]).unwrap());
        assert_eq!(integrated.error.real_sign().unwrap(), Ordering::Greater);
        assert_eq!(
            f.subtract(&ExactField::one(), &integrated.error)
                .unwrap()
                .real_sign()
                .unwrap(),
            Ordering::Greater
        );
        let measured = files.evaluator(true, compiler);
        let mut branches = BTreeMap::new();
        for seed in 0..64 {
            let draw = measured.sample(&mut SplitMix64::new(seed)).unwrap();
            assert_eq!(draw.sampled_records.len(), 1);
            assert_eq!(draw.history_weight, ExactField::one());
            assert_eq!(
                draw.noise_likelihood_ratio,
                num_rational::BigRational::from_integer(1.into())
            );
            branches.insert(draw.sampled_records[0], draw);
            if branches.len() == 2 {
                break;
            }
        }
        assert_eq!(branches.len(), 2);
        let mut mass = ExactScalar::zero();
        let mut raw_error = ExactScalar::zero();
        for (index, bit) in [false, true].into_iter().enumerate() {
            let branch = branches.get(&bit).unwrap();
            assert_eq!(branch.history_mass, masses[index]);
            assert_eq!(branch.raw_acceptance, masses[index]);
            assert_eq!(branch.accepted, ExactField::one());
            assert_eq!(
                branch.error,
                if bit {
                    ExactField::one()
                } else {
                    ExactField::zero()
                }
            );
            assert_eq!(
                branch.raw_error,
                if bit {
                    masses[1].clone()
                } else {
                    ExactScalar::zero()
                }
            );
            mass = a.add(&mass, &branch.history_mass).unwrap();
            raw_error = a.add(&raw_error, &branch.raw_error).unwrap();
        }
        assert_eq!(mass, ExactScalar::one());
        assert_eq!(raw_error, integrated.raw_error);
    }
}
