use rmagna::circuit::{ParseOptions, parse_circuit};
use rmagna::instrument::{
    CompileOptions, InsertionPreparation, InstrumentError, QueryPreparation, compile_affine,
};
use rmagna::noise::{NoiseSampling, SplitMix64};
use rmagna::runtime::{CausalProgram, CausalStrategy, ControllerRegistry, HistorySampler};
use rmagna::scalar::ScalarArithmetic;
use rmagna::task::{EvaluationOptions, TaskEvaluator, TaskOptions, TaskRunOptions, parse_task};
use rmagna::unitary::{CompilerStrategy, InsertionPreparationOptions, UnitaryError};
use std::path::{Path, PathBuf};
fn preparation() -> InsertionPreparation {
    InsertionPreparation::Prepared(InsertionPreparationOptions::default())
}
fn evaluator(
    source: &str,
    task: &str,
    path: &Path,
    registry: &ControllerRegistry,
    options: EvaluationOptions,
    sampling: &NoiseSampling,
) -> TaskEvaluator {
    let parse = ParseOptions {
        controllers: registry.names(),
        ..Default::default()
    };
    let circuit = parse_circuit(source, &parse).unwrap();
    let modules = parse_task(task, path, &TaskOptions::default())
        .unwrap()
        .load_modules(&circuit, &parse)
        .unwrap();
    TaskEvaluator::new_with_sampling(&circuit, &modules, registry, options, sampling).unwrap()
}
#[test]
fn exact_correlated_report_faults_projected_targets_recovery_and_moments_match() {
    let files = Files::new();
    files.put("target.magna", "H 0\nCX 0 1");
    files.put("recovery.magna", "H 0");
    let source = "H 0\nT 0\nH 0\nE(1/5) X0 Z1\nELSE_CORRELATED_ERROR(1/4) Y1\nH 1\nM(1/7) 0\nCX rec[-1] 1\nM(1/11) 1\nDETECTOR rec[-2] rec[-1]\nPOSTSELECT det[0]";
    let task = r#"{"outputs":[1],"ideal_state":{"circuit":"target.magna","outputs":[1],"postselection":{"0":0}},"recovery":"recovery.magna","observables":[[1,0],[0,1],[1,1]]}"#;
    let registry = ControllerRegistry::default();
    for strategy in [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ] {
        let mut options = EvaluationOptions::default();
        options.runtime.compile.compiler_strategy = strategy;
        options.runtime.compile.probability_preparation =
            QueryPreparation::FixedFibers(Default::default());
        options.runtime.compile.density_preparation =
            QueryPreparation::FixedFibers(Default::default());
        options.runtime.sampling.max_terms = 1024;
        let direct = evaluator(
            source,
            task,
            &files.0,
            &registry,
            options,
            &NoiseSampling::Physical,
        );
        options.runtime.compile.insertion_preparation = preparation();
        let fast = evaluator(
            source,
            task,
            &files.0,
            &registry,
            options,
            &NoiseSampling::Physical,
        );
        assert!(direct.prepared_insertion_statistics().is_none());
        let snapshot = fast.prepared_insertion_statistics().unwrap();
        assert_eq!(snapshot.catalog_entries, 4);
        assert!(snapshot.estimated_peak_bytes > 0);
        for category in 0..3 {
            for first in [false, true] {
                for second in [false, true] {
                    let mut faults = vec![];
                    if category > 0 {
                        faults.push(rmagna::instrument::FaultSelection {
                            site: 0,
                            alternative: category - 1,
                        });
                    }
                    for (site, active) in [(1, first), (2, second)] {
                        if active {
                            faults.push(rmagna::instrument::FaultSelection {
                                site,
                                alternative: 0,
                            });
                        }
                    }
                    let mut a = SplitMix64::new(8);
                    let mut b = a;
                    assert!(
                        direct.sample_prescribed(&faults, &mut a).unwrap()
                            == fast.sample_prescribed(&faults, &mut b).unwrap()
                    );
                    assert_eq!(a.state(), b.state());
                }
            }
        }
        assert_eq!(fast.prepared_insertion_statistics(), Some(snapshot));
        std::thread::scope(|scope| {
            let handles = (0..4)
                .map(|seed| {
                    scope.spawn({
                        let direct = &direct;
                        let fast = &fast;
                        move || {
                            let mut a = SplitMix64::new(seed);
                            let mut b = a;
                            assert!(direct.sample(&mut a).unwrap() == fast.sample(&mut b).unwrap());
                            assert_eq!(a.state(), b.state());
                        }
                    })
                })
                .collect::<Vec<_>>();
            for handle in handles {
                handle.join().unwrap();
            }
        });
        assert_eq!(fast.prepared_insertion_statistics(), Some(snapshot));
    }
}
#[test]
fn causal_guards_decisions_proposals_and_seeded_batches_keep_exact_values_and_streams() {
    let source = "H 0\nT 0\nH 0\nM(1/7) 0\nDETECTOR rec[-1]\nCONTROL[copy] det[0] -> ctrl[0]\nCH ctrl[0] 1\nX_ERROR(1/3) ctrl[0] 1\nM 1\nDETECTOR rec[-1]\nCONTROL[parity] det[1] ctrl[0] -> ctrl[1]\nCX ctrl[1] 2\nM 2\nOBSERVABLE_INCLUDE(0) rec[-1]";
    let task = r#"{"error_observables":[0],"sample_records":[0,1,2]}"#;
    let mut registry = ControllerRegistry::default();
    registry
        .register("copy", "1", |b: &[bool]| Ok(b.to_vec()))
        .unwrap();
    registry
        .register("parity", "1", |b: &[bool]| {
            Ok(vec![b.iter().fold(false, |a, b| a ^ b)])
        })
        .unwrap();
    for compiler in [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ] {
        for strategy in [CausalStrategy::FixedTemplate, CausalStrategy::ColdPrefix] {
            for sampler in [HistorySampler::ExactMarginal, HistorySampler::AffineSupport] {
                for sampling in [
                    NoiseSampling::Physical,
                    NoiseSampling::Importance {
                        proposal_scale: "2".parse().unwrap(),
                    },
                ] {
                    let mut options = EvaluationOptions::default();
                    options.runtime.compile.compiler_strategy = compiler;
                    options.runtime.strategy = strategy;
                    options.runtime.history_sampler = sampler;
                    options.runtime.compile.probability_preparation =
                        QueryPreparation::FixedFibers(Default::default());
                    options.runtime.sampling.max_terms = 1024;
                    let direct = evaluator(
                        source,
                        task,
                        Path::new("/tmp"),
                        &registry,
                        options,
                        &sampling,
                    );
                    options.runtime.compile.insertion_preparation = preparation();
                    let fast = evaluator(
                        source,
                        task,
                        Path::new("/tmp"),
                        &registry,
                        options,
                        &sampling,
                    );
                    let snapshot = fast.prepared_insertion_statistics();
                    if strategy == CausalStrategy::FixedTemplate {
                        assert_eq!(snapshot.unwrap().catalog_entries, 6);
                    } else {
                        assert!(snapshot.is_none());
                    }
                    for seed in 0..16 {
                        let mut a = SplitMix64::new(seed);
                        let mut b = a;
                        assert!(
                            direct.sample(&mut a).unwrap() == fast.sample(&mut b).unwrap(),
                            "{compiler:?}/{strategy:?}/{sampler:?}/seed{seed}"
                        );
                        assert_eq!(a.state(), b.state());
                    }
                    let run = TaskRunOptions {
                        shots: 4,
                        seed: 42,
                        ..Default::default()
                    };
                    let a = direct.run_batch(&run).unwrap();
                    let b = fast.run_batch(&run).unwrap();
                    assert_eq!(a.statistics, b.statistics);
                    assert_eq!(a.efficiency, b.efficiency);
                    assert_eq!(a.sampled_records, b.sampled_records);
                    assert_eq!(a.failures, b.failures);
                    assert_eq!(fast.prepared_insertion_statistics(), snapshot);
                }
            }
        }
    }
    // Fixed branches retain the original catalog even when a guard removes and
    // renumbers its category in the active branch.
    let parse = ParseOptions {
        controllers: registry.names(),
        ..Default::default()
    };
    let circuit = parse_circuit(source, &parse).unwrap();
    let mut runtime = rmagna::runtime::RuntimeOptions::default();
    runtime.compile.insertion_preparation = preparation();
    runtime.sampling.max_terms = 1024;
    let program =
        CausalProgram::new(&circuit, &registry, runtime, &ScalarArithmetic::default()).unwrap();
    let snapshot = program.prepared_insertion_statistics();
    for seed in 0..16 {
        let history = program
            .resolve(
                &[],
                &mut SplitMix64::new(seed),
                &ScalarArithmetic::default(),
            )
            .unwrap();
        assert_eq!(
            history.instrument().prepared_insertion_statistics(),
            snapshot
        );
    }
}
#[test]
fn catalog_diagnostics_and_entry_byte_budgets_fail_explicitly_at_exact_boundaries() {
    let a = ScalarArithmetic::default();
    let circuit = parse_circuit("H 0\nM(1/3) 0", &ParseOptions::default()).unwrap();
    let mut options = CompileOptions {
        insertion_preparation: preparation(),
        ..Default::default()
    };
    let prepared = compile_affine(&circuit, &options, &a).unwrap();
    let counts = prepared.prepared_insertion_statistics().unwrap();
    assert_eq!(counts.catalog_entries, 1);
    let bytes = usize::try_from(counts.estimated_peak_bytes).unwrap();
    options.insertion_preparation = InsertionPreparation::Prepared(InsertionPreparationOptions {
        max_catalog_entries: 1,
        max_table_bytes: bytes,
    });
    assert_eq!(
        compile_affine(&circuit, &options, &a)
            .unwrap()
            .prepared_insertion_statistics(),
        Some(counts)
    );
    options.insertion_preparation = InsertionPreparation::Prepared(InsertionPreparationOptions {
        max_catalog_entries: 1,
        max_table_bytes: bytes - 1,
    });
    assert!(
        matches!(compile_affine(&circuit,&options,&a),Err(InstrumentError::Unitary(UnitaryError::InsertionResourceLimit{option:"max_table_bytes",required,..})) if required==bytes as u128)
    );
    options.insertion_preparation = InsertionPreparation::Prepared(InsertionPreparationOptions {
        max_catalog_entries: 0,
        max_table_bytes: bytes,
    });
    assert!(matches!(
        compile_affine(&circuit, &options, &a),
        Err(InstrumentError::Unitary(
            UnitaryError::InsertionResourceLimit {
                option: "max_catalog_entries",
                required: 1,
                ..
            }
        ))
    ));
    let empty = parse_circuit("I 0", &ParseOptions::default()).unwrap();
    options.insertion_preparation = InsertionPreparation::Prepared(InsertionPreparationOptions {
        max_catalog_entries: 0,
        max_table_bytes: 0,
    });
    let empty = compile_affine(&empty, &options, &a).unwrap();
    let empty = empty.prepared_insertion_statistics().unwrap();
    assert_eq!((empty.catalog_entries, empty.estimated_peak_bytes), (0, 0));
    let good = compile_affine(
        &circuit,
        &CompileOptions {
            insertion_preparation: preparation(),
            ..Default::default()
        },
        &a,
    )
    .unwrap();
    for faults in [
        vec![rmagna::instrument::FaultSelection {
            site: 2,
            alternative: 0,
        }],
        vec![rmagna::instrument::FaultSelection {
            site: 0,
            alternative: 1,
        }],
        vec![
            rmagna::instrument::FaultSelection {
                site: 0,
                alternative: 0
            };
            2
        ],
    ] {
        assert!(good.acceptance(&faults, 1024, &a).is_err());
    }
}
struct Files(PathBuf);
impl Files {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let p = std::env::temp_dir().join(format!(
            "rmagna-insertion-catalog-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir(&p).unwrap();
        Self(p)
    }
    fn put(&self, name: &str, text: &str) {
        std::fs::write(self.0.join(name), text).unwrap();
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
