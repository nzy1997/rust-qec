// SPDX-License-Identifier: MPL-2.0
// Complete PackedNoiseTests::test_guarded_sites_share_a_group_but_wait_for_their_causal_instruction,
// itpyi/magna@56847478c840395735146ef1e1ccdc3d8aedc10f/tests/core/test_packed_noise.py.
use num_rational::BigRational;
use rmagna::circuit::{Basis, BitAtom, BitExpr, ParseOptions, parse_circuit};
use rmagna::field::ExactField;
use rmagna::file_api::{FileLoadOptions, FileTask, FileTaskOptions};
use rmagna::instrument::QueryPreparation;
use rmagna::noise::{
    NoiseSampling, OriginalNoiseAction, OriginalNoiseOptions, RandomSource,
    original_noise_categories,
};
use rmagna::runtime::{CausalStrategy, ControllerRegistry, HistorySampler};
use rmagna::scalar::ExactScalar;
use rmagna::task::{
    EvaluationOptions, TaskEvaluator, TaskOptions, TaskRunOptions, TaskSampleDiagnosticOptions,
    parse_task,
};
use rmagna::unitary::CompilerStrategy;
use serde_json::json;
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};
struct NoEntropy;
impl RandomSource for NoEntropy {
    fn next_u64(&mut self) -> u64 {
        panic!("deterministic packed guarded sites must use zero entropy")
    }
}
struct Files(PathBuf);
impl Files {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "rmagna-packed-guards-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
#[test]
fn original_guarded_locations_preserve_causal_positions_across_two_full_blocks() {
    let files = Files::new();
    let mut configurations = 0;
    for control in [false, true] {
        let mut text = if control {
            String::from("X 0\nM 0\n")
        } else {
            String::from("M 0\n")
        };
        for _ in 0..65 {
            text.push_str("X_ERROR(1) rec[-1] 1\nX_ERROR(1) 2\n");
        }
        let target = if control { "X 0\nX 1" } else { "X 1" };
        std::fs::write(files.0.join("target.magna"), target).unwrap();
        std::fs::write(files.0.join("physical.magna"), &text).unwrap();
        let task=json!({"outputs":[1,2],"sample_records":[0],"observables":[[0,1],[0,2],[0,3]],"ideal_state":"target.magna"}).to_string();
        std::fs::write(files.0.join("task.json"), &task).unwrap();
        let parse = ParseOptions::default();
        let physical = parse_circuit(&text, &parse).unwrap();
        let before = physical.operations().to_vec();
        let modules = parse_task(&task, &files.0, &TaskOptions::default())
            .unwrap()
            .load_modules(&physical, &parse)
            .unwrap();
        let categories =
            original_noise_categories(&physical, &OriginalNoiseOptions::default()).unwrap();
        assert_eq!(categories.len(), 130);
        for (index, category) in categories.iter().enumerate() {
            assert_eq!(
                category.internal_site,
                index + 1,
                "implicit zero-rate readout keeps site0 reserved"
            );
            assert_eq!(
                category.identity_probability.value(),
                &"0".parse::<BigRational>().unwrap()
            );
            assert_eq!(category.channels.len(), 1);
            let channel = &category.channels[0];
            assert_eq!(
                channel.probability.value(),
                &"1".parse::<BigRational>().unwrap()
            );
            assert_eq!(
                channel.action,
                OriginalNoiseAction::Pauli {
                    factors: vec![(if index % 2 == 0 { 1 } else { 2 }, Basis::X)]
                }
            );
            assert_eq!(
                category.guard,
                if index % 2 == 0 {
                    Some(BitExpr::atom(BitAtom::Record(0)))
                } else {
                    None
                }
            );
            assert_eq!(category.source_operations.len(), 1);
            assert_eq!(channel.source, category.source_operations[0]);
            assert_eq!(
                channel.source.location.line,
                index + 2 + usize::from(control)
            );
        }
        for compiler in [
            CompilerStrategy::ParityCopies,
            CompilerStrategy::SharedCommutingLayer,
        ] {
            for causal in [CausalStrategy::ColdPrefix, CausalStrategy::FixedTemplate] {
                for sampler in [HistorySampler::ExactMarginal, HistorySampler::AffineSupport] {
                    for query in [
                        QueryPreparation::Cold,
                        QueryPreparation::FixedFibers(Default::default()),
                    ] {
                        let mut evaluation = EvaluationOptions::default();
                        evaluation.runtime.compile.compiler_strategy = compiler;
                        // Reserve bounded families for all 65 ordered conditional prefixes;
                        // Native grouping and Rust query-family entries have different scopes.
                        evaluation.runtime.compile.max_prepared_probabilities = 256;
                        evaluation.runtime.strategy = causal;
                        evaluation.runtime.history_sampler = sampler;
                        evaluation.runtime.compile.probability_preparation = query;
                        evaluation.runtime.compile.density_preparation = query;
                        evaluation.runtime.compile.moment_preparation = query;
                        let evaluator = TaskEvaluator::new(
                            &physical,
                            &modules,
                            &ControllerRegistry::default(),
                            evaluation,
                        )
                        .unwrap();
                        for category in &categories {
                            assert_eq!(
                                evaluator.potential_site(category.source_operations[0].operation),
                                Some(category.internal_site)
                            );
                        }
                        let sign = if control {
                            ExactField::one().negated()
                        } else {
                            ExactField::one()
                        };
                        let expected = [sign.clone(), ExactField::one().negated(), sign.negated()];
                        for _seed in 0..4 {
                            let (draw, diagnostics) = evaluator
                                .sample_with_diagnostics(
                                    &mut NoEntropy,
                                    &TaskSampleDiagnosticOptions::default(),
                                )
                                .unwrap();
                            assert_eq!(draw.accepted, ExactField::one());
                            assert_eq!(draw.error, ExactField::zero());
                            assert_eq!(draw.target_success, Some(ExactField::one()));
                            assert_eq!(draw.raw_acceptance, ExactScalar::one());
                            assert_eq!(draw.raw_error, ExactScalar::zero());
                            assert_eq!(draw.raw_target_success, Some(ExactScalar::one()));
                            assert_eq!(draw.target_normalization, Some(ExactScalar::one()));
                            assert_eq!(draw.history_mass, ExactScalar::one());
                            assert_eq!(draw.moments, expected);
                            assert_eq!(draw.noise_probability, "1".parse::<BigRational>().unwrap());
                            assert_eq!(draw.sampled_records, [control]);
                            assert!(draw.controller_calls.is_empty());
                            assert_eq!(diagnostics.learned_record_rank, 1);
                            assert_eq!(diagnostics.noise_entropy_words, 0);
                            assert_eq!(diagnostics.history_entropy_words, 0);
                            if causal == CausalStrategy::FixedTemplate {
                                assert_eq!(draw.network_compilations, 0);
                            }
                        }
                        let file = FileTask::load(
                            &files.0.join("physical.magna"),
                            Some(&files.0.join("task.json")),
                            &ControllerRegistry::default(),
                            NoiseSampling::Physical,
                            FileTaskOptions {
                                evaluation,
                                ..Default::default()
                            },
                            &FileLoadOptions::default(),
                            BTreeMap::new(),
                        )
                        .unwrap();
                        let run = TaskRunOptions {
                            shots: 4,
                            seed: 0,
                            ..Default::default()
                        };
                        let fingerprints = file.fingerprints(&run).unwrap().to_json();
                        let report = file.run(&run).unwrap();
                        drop(file);
                        let value = report.to_json().unwrap();
                        assert_eq!(value["status"], "complete");
                        assert_eq!(value["completed"], 4);
                        assert_eq!(value["estimates"]["acceptance"], 1.0);
                        assert_eq!(value["estimates"]["error"], 0.0);
                        assert_eq!(
                            value["estimates"]["moments"],
                            json!([
                                if control { -1.0 } else { 1.0 },
                                -1.0,
                                if control { 1.0 } else { -1.0 }
                            ])
                        );
                        assert_eq!(
                            value["sampled_records"],
                            json!([[control], [control], [control], [control]])
                        );
                        assert_eq!(value["evaluation"]["output_mapping"], json!([1, 2]));
                        assert_eq!(value["fingerprints"], fingerprints);
                        assert_eq!(physical.operations(), before);
                        assert_eq!(physical.to_text(), text);
                        assert_eq!(
                            std::fs::read_to_string(files.0.join("physical.magna")).unwrap(),
                            text
                        );
                        configurations += 1;
                    }
                }
            }
        }
    }
    assert_eq!(configurations, 32);
}
