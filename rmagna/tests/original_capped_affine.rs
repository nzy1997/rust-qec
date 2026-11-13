// SPDX-License-Identifier: MPL-2.0
// Five finite CappedAffineTests methods, itpyi/magna
// 56847478c840395735146ef1e1ccdc3d8aedc10f/tests/core/test_capped_affine.py.
use num_rational::BigRational;
use rmagna::circuit::{ParseOptions, parse_circuit};
use rmagna::field::{ExactField, FieldArithmetic};
use rmagna::instrument::QueryPreparation;
use rmagna::noise::SplitMix64;
use rmagna::runtime::{CausalStrategy, ControllerRegistry, HistorySampler};
use rmagna::scalar::{ExactScalar, ScalarArithmetic};
use rmagna::task::{EvaluationOptions, TaskEvaluator, TaskOptions, parse_task};
use rmagna::unitary::CompilerStrategy;
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};
fn r(n: i64, d: i64) -> BigRational {
    BigRational::new(n.into(), d.into())
}
fn field(value: &ExactScalar) -> ExactField {
    FieldArithmetic::default().from_scalar(value).unwrap()
}
fn biased(bit: bool, power: i64) -> ExactScalar {
    ScalarArithmetic::default()
        .scalar(
            [2, if bit { -1 } else { 1 }, 0, if bit { 1 } else { -1 }].map(Into::into),
            power,
        )
        .unwrap()
}
struct Files(PathBuf);
impl Files {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "rmagna-capped-original-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn evaluator(
        &self,
        source: &str,
        mut task: Value,
        target: Option<(&str, Value)>,
        registry: &ControllerRegistry,
        options: EvaluationOptions,
    ) -> TaskEvaluator {
        if let Some((text, mut spec)) = target {
            std::fs::write(self.0.join("target.magna"), text).unwrap();
            if spec.is_object() {
                spec["circuit"] = json!("target.magna");
            } else {
                spec = json!("target.magna");
            }
            task["ideal_state"] = spec;
        }
        let parse = ParseOptions {
            controllers: registry.names(),
            ..Default::default()
        };
        let source = parse_circuit(source, &parse).unwrap();
        let modules = parse_task(&task.to_string(), &self.0, &TaskOptions::default())
            .unwrap()
            .load_modules(&source, &parse)
            .unwrap();
        TaskEvaluator::new(&source, &modules, registry, options).unwrap()
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn configs() -> Vec<EvaluationOptions> {
    let mut result = vec![];
    for compiler in [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ] {
        for causal in [CausalStrategy::ColdPrefix, CausalStrategy::FixedTemplate] {
            for preparation in [
                QueryPreparation::Cold,
                QueryPreparation::FixedFibers(Default::default()),
            ] {
                let mut options = EvaluationOptions::default();
                options.runtime.compile.compiler_strategy = compiler;
                options.runtime.strategy = causal;
                options.runtime.history_sampler = HistorySampler::AffineSupport;
                options.runtime.compile.probability_preparation = preparation;
                options.runtime.compile.density_preparation = preparation;
                options.runtime.compile.max_prepared_probabilities = 256;
                result.push(options);
            }
        }
    }
    result
}
type Seen = Arc<Mutex<Vec<Vec<bool>>>>;
fn echo() -> (ControllerRegistry, Seen) {
    let seen = Arc::new(Mutex::new(vec![]));
    let saved = seen.clone();
    let mut registry = ControllerRegistry::default();
    registry
        .register("echo", "1", move |bits: &[bool]| {
            saved.lock().unwrap().push(bits.to_vec());
            Ok(bits.to_vec())
        })
        .unwrap();
    (registry, seen)
}
#[test]
fn original_biased_t_full_support_retains_correlated_records_and_exact_weights() {
    let files = Files::new();
    let a = ScalarArithmetic::default();
    let half = a.omega(0, -2).unwrap();
    for options in configs() {
        let evaluator = files.evaluator(
            "H 0\nT 0\nMX 0\nMX 0",
            json!({"outputs":[0],"sample_records":[0,1]}),
            Some(("H 0 1", json!({"outputs":[0],"postselection":{"1":1}}))),
            &ControllerRegistry::default(),
            options,
        );
        let mut observed = BTreeSet::new();
        let mut warm = None;
        for seed in 0..32 {
            let row = evaluator.sample(&mut SplitMix64::new(seed)).unwrap();
            let bit = row.sampled_records[0];
            assert_eq!(row.sampled_records, [bit, bit]);
            observed.insert(bit);
            let p = biased(bit, -2);
            let weight = biased(bit, -1);
            assert_eq!(row.history_mass, p);
            assert_eq!(row.history_proposal_mass, Some(half.clone()));
            assert_eq!(row.history_weight, field(&weight));
            assert_eq!(row.accepted, field(&weight));
            assert_eq!(
                row.error,
                if bit {
                    field(&weight)
                } else {
                    ExactField::zero()
                }
            );
            assert_eq!(
                row.target_success,
                Some(if bit {
                    ExactField::zero()
                } else {
                    field(&weight)
                })
            );
            assert_eq!(row.target_normalization, Some(half.clone()));
            assert_eq!(row.noise_likelihood_ratio, r(1, 1));
            assert!(row.controller_calls.is_empty());
            if options.runtime.strategy == CausalStrategy::FixedTemplate {
                assert_eq!(row.network_compilations, 0);
                let stats = evaluator.prepared_support_statistics().unwrap();
                if let Some(previous) = &warm {
                    assert_eq!(&stats, previous);
                } else {
                    warm = Some(stats);
                }
            }
        }
        assert_eq!(observed, BTreeSet::from([false, true]));
        assert_eq!(
            a.add(&biased(false, -2), &biased(true, -2)).unwrap(),
            ExactScalar::one()
        );
    }
}
#[test]
fn original_general_transport_reuse_respects_actual_z_feedback_and_biased_weights() {
    let files = Files::new();
    let half = ScalarArithmetic::default().omega(0, -2).unwrap();
    for options in configs() {
        let (registry, seen) = echo();
        let evaluator=files.evaluator("H 0\nT 0\nMX 0\nDETECTOR rec[-1]\nCONTROL[echo] det[-1] -> ctrl[0]\nCZ ctrl[0] 0\nMX 0",json!({"sample_records":[0,1]}),None,&registry,options);
        let mut found = BTreeSet::new();
        let mut warm = None;
        for seed in 0..32 {
            seen.lock().unwrap().clear();
            let row = evaluator.sample(&mut SplitMix64::new(seed)).unwrap();
            let bit = row.sampled_records[0];
            found.insert(bit);
            assert_eq!(row.sampled_records, [bit, false]);
            assert_eq!(*seen.lock().unwrap(), vec![vec![bit]]);
            assert_eq!(row.controller_calls.len(), 1);
            assert_eq!(row.controller_calls[0].inputs, [bit]);
            assert_eq!(row.controller_calls[0].outputs, [bit]);
            assert_eq!(row.history_mass, biased(bit, -2));
            assert_eq!(row.history_proposal_mass, Some(half.clone()));
            assert_eq!(row.history_weight, field(&biased(bit, -1)));
            assert_eq!(row.accepted, field(&biased(bit, -1)));
            assert_eq!(row.error, ExactField::zero());
            if options.runtime.strategy == CausalStrategy::FixedTemplate {
                assert_eq!(row.network_compilations, 0);
                let stats = evaluator.prepared_support_statistics().unwrap();
                if let Some(previous) = &warm {
                    assert_eq!(&stats, previous);
                } else {
                    warm = Some(stats);
                }
            }
        }
        assert_eq!(found, BTreeSet::from([false, true]));
    }
}
#[test]
fn original_large_affine_envelope_samples_82_records_without_enumerating_support() {
    let files = Files::new();
    let source = format!(
        "H {}\nT 0\nMX 0\nMX 0\nM {}",
        (0..81).map(|q| q.to_string()).collect::<Vec<_>>().join(" "),
        (1..81).map(|q| q.to_string()).collect::<Vec<_>>().join(" ")
    );
    let q = ScalarArithmetic::default().omega(0, -162).unwrap();
    for options in configs() {
        let evaluator = files.evaluator(
            &source,
            json!({"sample_records":(0..82).collect::<Vec<_>>()}),
            None,
            &ControllerRegistry::default(),
            options,
        );
        let mut rng = SplitMix64::new(15);
        let mut warm = None;
        for _ in 0..8 {
            let row = evaluator.sample(&mut rng).unwrap();
            assert_eq!(row.sampled_records.len(), 82);
            let bit = row.sampled_records[0];
            assert_eq!(row.sampled_records[1], bit);
            assert_eq!(row.history_proposal_mass, Some(q.clone()));
            let p = ScalarArithmetic::default()
                .multiply(
                    &biased(bit, -2),
                    &ScalarArithmetic::default().omega(0, -160).unwrap(),
                )
                .unwrap();
            assert_eq!(row.history_mass, p);
            assert_eq!(row.history_weight, field(&biased(bit, -1)));
            assert_eq!(row.accepted, field(&biased(bit, -1)));
            assert_eq!(row.error, ExactField::zero());
            assert!(row.controller_calls.is_empty());
            if options.runtime.strategy == CausalStrategy::FixedTemplate {
                assert_eq!(row.network_compilations, 0);
                let stats = evaluator.prepared_support_statistics().unwrap().unwrap();
                assert_eq!(stats.labels, 82);
                // This Rust support plan retains the doubled probability network.
                assert_eq!(stats.quotient_rank, 2);
                assert!(stats.terms <= 4);
                if let Some(previous) = &warm {
                    assert_eq!(&stats, previous);
                } else {
                    warm = Some(stats);
                }
            } else {
                assert!(evaluator.prepared_support_statistics().unwrap().is_none());
            }
        }
    }
}

#[test]
fn original_nonaffine_walsh_zeros_and_adaptive_report_flip_preserve_exact_proposals() {
    use std::collections::BTreeMap;
    let files = Files::new();
    let a = ScalarArithmetic::default();
    let f = FieldArithmetic::default();
    let triples = [(0, 1, 2), (0, 1, 3), (0, 2, 3)];
    let amplitudes = (0usize..16)
        .map(|y| {
            (0usize..16)
                .map(|x| {
                    let phase = triples
                        .iter()
                        .map(|&(i, j, k)| ((x >> i) & 1) * ((x >> j) & 1) * ((x >> k) & 1))
                        .sum::<usize>()
                        + (x & y).count_ones() as usize;
                    if phase.is_multiple_of(2) { 1i64 } else { -1 }
                })
                .sum::<i64>()
        })
        .collect::<Vec<_>>();
    let probabilities = amplitudes
        .iter()
        .map(|&v| r(v * v, 256))
        .collect::<Vec<_>>();
    assert_eq!(probabilities.iter().filter(|p| **p == r(0, 1)).count(), 6);
    assert_eq!(probabilities.iter().cloned().sum::<BigRational>(), r(1, 1));
    let source = "H 0 1 2 3\nCCZ 0 1 2\nCCZ 0 1 3\nCCZ 0 2 3\nH 0 1 2 3\nM 0 1 2 3";
    for options in configs() {
        let evaluator = files.evaluator(
            source,
            json!({"sample_records":[0,1,2,3]}),
            None,
            &ControllerRegistry::default(),
            options,
        );
        let mut found = BTreeMap::new();
        for seed in 0..128 {
            let row = evaluator.sample(&mut SplitMix64::new(seed)).unwrap();
            let word = row
                .sampled_records
                .iter()
                .enumerate()
                .map(|(j, &b)| usize::from(b) << j)
                .sum::<usize>();
            let p = &probabilities[word];
            assert_eq!(
                field(&row.history_mass),
                f.from_rational(p.clone()).unwrap()
            );
            assert_eq!(row.history_proposal_mass, Some(a.omega(0, -8).unwrap()));
            assert_eq!(row.accepted, f.from_rational(p * r(16, 1)).unwrap());
            assert_eq!(row.history_weight, row.accepted);
            assert_eq!(row.error, ExactField::zero());
            found.insert(word, row.accepted);
        }
        assert_eq!(found.len(), 16);
        assert_eq!(
            found.values().filter(|v| **v == ExactField::zero()).count(),
            6
        );
        let (registry, seen) = echo();
        let adaptive = format!(
            "{source}\nDETECTOR rec[-3]\nCONTROL[echo] det[-1] -> ctrl[0]\nCX ctrl[0] 4\nDETECTOR rec[-4]\nPOSTSELECT det[1]"
        );
        let evaluator = files.evaluator(
            &adaptive,
            json!({"outputs":[4],"sample_records":[0,1,2,3]}),
            Some(("I 0", Value::Null)),
            &registry,
            options,
        );
        // All four original readouts reserve sites even without explicit noise.
        let faults = [rmagna::instrument::FaultSelection {
            site: 1,
            alternative: 0,
        }];
        let mut found = BTreeMap::new();
        let mut warm = None;
        for seed in 0..128 {
            seen.lock().unwrap().clear();
            let row = evaluator
                .sample_prescribed(&faults, &mut SplitMix64::new(seed))
                .unwrap();
            let word = row
                .sampled_records
                .iter()
                .enumerate()
                .map(|(j, &b)| usize::from(b) << j)
                .sum::<usize>();
            let true_word = word ^ 2;
            let branch = word & 2 != 0;
            let q = if branch { r(1, 16) } else { r(1, 8) };
            assert_eq!(*seen.lock().unwrap(), vec![vec![branch]]);
            assert_eq!(row.controller_calls.len(), 1);
            assert_eq!(row.controller_calls[0].inputs, [branch]);
            assert_eq!(
                field(&row.history_proposal_mass.clone().unwrap()),
                f.from_rational(q.clone()).unwrap()
            );
            assert_eq!(
                field(&row.history_mass),
                f.from_rational(probabilities[true_word].clone()).unwrap()
            );
            let accepted = if word & 1 == 0 {
                &probabilities[true_word] / &q
            } else {
                r(0, 1)
            };
            assert_eq!(row.accepted, f.from_rational(accepted.clone()).unwrap());
            assert_eq!(
                row.error,
                f.from_rational(if branch { accepted.clone() } else { r(0, 1) })
                    .unwrap()
            );
            assert_eq!(
                row.target_success,
                Some(
                    f.from_rational(if branch { r(0, 1) } else { accepted })
                        .unwrap()
                )
            );
            found.insert(word, q);
            if options.runtime.strategy == CausalStrategy::FixedTemplate {
                assert_eq!(row.network_compilations, 0);
                let stats = evaluator.prepared_support_statistics().unwrap();
                if let Some(previous) = &warm {
                    assert_eq!(&stats, previous);
                } else {
                    warm = Some(stats);
                }
            }
        }
        let expected = (0usize..16)
            .filter(|word| word & 2 != 0 || ((word >> 2) & 1) == ((word >> 3) & 1))
            .collect::<BTreeSet<_>>();
        assert_eq!(found.keys().copied().collect::<BTreeSet<_>>(), expected);
        assert_eq!(
            found
                .keys()
                .filter(|&&word| probabilities[word ^ 2] == r(0, 1))
                .count(),
            2
        );
        assert_eq!(found.values().cloned().sum::<BigRational>(), r(1, 1));
    }
}
#[test]
fn original_complete_support_weights_encoded_target_and_exact_physical_integration() {
    use rmagna::circuit::{Basis, Operation};
    use rmagna::noise::{OriginalNoiseAction, OriginalNoiseOptions, original_noise_categories};
    use std::collections::BTreeMap;
    let files = Files::new();
    let a = ScalarArithmetic::default();
    let f = FieldArithmetic::default();
    let source = "H 0 1\nM(1/5) 0\nDETECTOR rec[-1]\nCONTROL[echo] det[-1] -> ctrl[0]\nCS ctrl[0] 1\nMX(1/7) 1\nZ 1\nMX 1\nDETECTOR rec[-1]\nCONTROL[echo] det[-1] -> ctrl[1]\nCX ctrl[1] 2\nCX 2 3\nX_ERROR(1/3) 0\nM 0\nDETECTOR rec[-4] rec[-3]\nPOSTSELECT det[2]";
    for options in configs() {
        let (registry, seen) = echo();
        let evaluator = files.evaluator(
            source,
            json!({"outputs":[2,3],"sample_records":[0,1,2,3]}),
            Some((
                "I 0 1\nH 2 3",
                json!({"outputs":[0,1],"postselection":{"2":1}}),
            )),
            &registry,
            options,
        );
        let circuit = parse_circuit(
            source,
            &ParseOptions {
                controllers: registry.names(),
                ..Default::default()
            },
        )
        .unwrap();
        let categories =
            original_noise_categories(&circuit, &OriginalNoiseOptions::default()).unwrap();
        let mut sites = vec![];
        for action in [
            OriginalNoiseAction::ReportFlip { record: 0 },
            OriginalNoiseAction::ReportFlip { record: 1 },
            OriginalNoiseAction::Pauli {
                factors: vec![(0, Basis::X)],
            },
        ] {
            let category = categories
                .iter()
                .find(|c| c.channels.iter().any(|ch| ch.action == action))
                .unwrap();
            let operation = category.source_operations[0].operation;
            assert_eq!(
                evaluator.potential_site(operation),
                Some(category.internal_site)
            );
            if matches!(action, OriginalNoiseAction::Pauli { .. }) {
                assert!(matches!(
                    circuit.operations()[operation].operation,
                    Operation::Noise { .. }
                ));
            }
            sites.push(category.internal_site);
        }
        assert_eq!(sites, [0, 1, 3]);
        let mut integrated = [r(0, 1), r(0, 1), r(0, 1)];
        let mut warm = None;
        for mask in 0..8 {
            let flip0 = mask & 1 != 0;
            let flip1 = mask & 2 != 0;
            let fault = mask & 4 != 0;
            let mut oracle = BTreeMap::new();
            for true0 in [false, true] {
                let first = true0 ^ flip0;
                for true1 in [false, true] {
                    if true1 && !first {
                        continue;
                    }
                    let word = vec![first, true1 ^ flip1, !true1, true0 ^ fault];
                    oracle.insert(word, r(1, if first { 4 } else { 2 }));
                }
            }
            let faults = (0..3)
                .filter(|j| mask & (1 << j) != 0)
                .map(|j| rmagna::instrument::FaultSelection {
                    site: sites[j],
                    alternative: 0,
                })
                .collect::<Vec<_>>();
            let noise_p = r(if flip0 { 1 } else { 4 }, 5)
                * r(if flip1 { 1 } else { 6 }, 7)
                * r(if fault { 1 } else { 2 }, 3);
            let mut observed = BTreeSet::new();
            for seed in 0..64 {
                seen.lock().unwrap().clear();
                let row = evaluator
                    .sample_supplied_noise(&faults, &r(3, 2), &mut SplitMix64::new(seed))
                    .unwrap();
                let word = &row.sampled_records;
                let p = oracle.get(word).unwrap();
                assert_eq!(*seen.lock().unwrap(), vec![vec![word[0]], vec![word[2]]]);
                assert_eq!(row.controller_calls.len(), 2);
                assert_eq!(row.controller_calls[0].inputs, [word[0]]);
                assert_eq!(row.controller_calls[1].inputs, [word[2]]);
                assert_eq!(
                    field(&row.history_mass),
                    f.from_rational(p.clone()).unwrap()
                );
                assert_eq!(row.history_proposal_mass, Some(row.history_mass.clone()));
                assert_eq!(row.history_weight, ExactField::one());
                assert_eq!(row.target_normalization, Some(a.omega(0, -2).unwrap()));
                assert_eq!(row.noise_likelihood_ratio, r(3, 2));
                assert_eq!(row.noise_probability, noise_p);
                let accepted = word[0] == word[1];
                assert_eq!(
                    row.accepted,
                    f.from_rational(if accepted { r(3, 2) } else { r(0, 1) })
                        .unwrap()
                );
                assert_eq!(
                    row.error,
                    f.from_rational(if accepted && word[2] {
                        r(3, 2)
                    } else {
                        r(0, 1)
                    })
                    .unwrap()
                );
                assert_eq!(
                    row.target_success,
                    Some(
                        f.from_rational(if accepted && !word[2] {
                            r(3, 2)
                        } else {
                            r(0, 1)
                        })
                        .unwrap()
                    )
                );
                observed.insert(word.clone());
                if options.runtime.strategy == CausalStrategy::FixedTemplate {
                    assert_eq!(row.network_compilations, 0);
                    let stats = evaluator.prepared_support_statistics().unwrap();
                    if let Some(previous) = &warm {
                        assert_eq!(&stats, previous);
                    } else {
                        warm = Some(stats);
                    }
                }
            }
            assert_eq!(observed, oracle.keys().cloned().collect());
            assert_eq!(oracle.values().cloned().sum::<BigRational>(), r(1, 1));
            integrated[0] += &noise_p;
            for (word, p) in oracle {
                if word[0] == word[1] {
                    integrated[1] += &noise_p * &p;
                    if !word[2] {
                        integrated[2] += &noise_p * p;
                    }
                }
            }
        }
        assert_eq!(integrated, [r(1, 1), r(19, 28), r(3, 14)]);
    }
}
