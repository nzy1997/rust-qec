use num_rational::BigRational;
use rmagna::{
    circuit::{Basis, Operation, ParseOptions, parse_circuit},
    field::ExactField,
    noise::SplitMix64,
    runtime::{CausalStrategy, ControllerRegistry, HistorySampler},
    scalar::ExactScalar,
    task::{
        EvaluationOptions, SuppliedActionLimits, SuppliedNoiseActions, SuppliedPauliAction,
        TaskEvaluator, TaskOptions, parse_task,
    },
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub fn check() -> Value {
    let raw = include_bytes!("../data/paper-proposal-runtime.json");
    assert_eq!(
        format!("{:x}", Sha256::digest(raw)),
        "b1fcc6b8d56019ff0a9b6a8d4de5d5e5ad8d3bbb85f431e5554c30ca14de0b1d"
    );
    let data: Value = serde_json::from_slice(raw).unwrap();
    assert_eq!(
        data["model_sha256"],
        format!(
            "{:x}",
            Sha256::digest(include_bytes!(
                "../../../benchmarks/magna_reproduction/paper_proposals.py"
            ))
        )
    );
    assert_eq!(
        data["generator_sha256"],
        format!(
            "{:x}",
            Sha256::digest(include_bytes!(
                "../../../benchmarks/magna_reproduction/paper_proposal_runtime_vectors.py"
            ))
        )
    );
    let temporary =
        Scratch(std::env::temp_dir().join(format!("rmagna-paper-proposal-{}", std::process::id())));
    std::fs::create_dir(&temporary.0).unwrap();
    std::fs::write(temporary.0.join("target.magna"), "I 0").unwrap();
    let calls = Arc::new(Mutex::new(Vec::<Vec<bool>>::new()));
    let observed = Arc::clone(&calls);
    let mut registry = ControllerRegistry::default();
    registry
        .register("echo", "1", move |bits: &[bool]| {
            observed.lock().unwrap().push(bits.to_vec());
            Ok(bits.to_vec())
        })
        .unwrap();
    let parse = ParseOptions {
        controllers: registry.names(),
        ..Default::default()
    };
    let mut samples = 0usize;
    let mut distinct_true_reported = 0usize;
    for rate in data["rates"].as_array().unwrap() {
        let rate = rate.as_str().unwrap();
        let physical=parse_circuit(&format!("DEPOLARIZE1({rate}) 0\nM({rate}) 0\nDETECTOR rec[-1]\nCONTROL[echo] det[0] -> ctrl[0]\nCX ctrl[0] 1\nDEPOLARIZE1({rate}) 1\nX_ERROR({rate}) 1\nPOSTSELECT ctrl[0]"),&parse).unwrap();
        let modules = parse_task(
            r#"{"outputs":[1],"ideal_state":"target.magna","sample_records":[0]}"#,
            &temporary.0,
            &TaskOptions::default(),
        )
        .unwrap()
        .load_modules(&physical, &parse)
        .unwrap();
        let anchors: Vec<_> = physical
            .operations()
            .iter()
            .filter_map(|op| matches!(op.operation, Operation::Noise { .. }).then_some(op.id))
            .collect();
        assert_eq!(anchors.len(), 3);
        for strategy in [CausalStrategy::FixedTemplate, CausalStrategy::ColdPrefix] {
            let mut options = EvaluationOptions::default();
            options.runtime.strategy = strategy;
            options.runtime.history_sampler = HistorySampler::ExactMarginal;
            for case in data["cases"].as_array().unwrap() {
                let rows = case["rows"].as_array().unwrap();
                assert_eq!(rows.len(), 64);
                let mut acceptance = ExactField::zero();
                let mut error = ExactField::zero();
                let mut direct_acceptance = BigRational::from_integer(0.into());
                let mut direct_error = BigRational::from_integer(0.into());
                let mut sum_p = BigRational::from_integer(0.into());
                let mut sum_q = BigRational::from_integer(0.into());
                for row in rows {
                    let pattern: [usize; 4] =
                        std::array::from_fn(|j| row["pattern"][j].as_u64().unwrap() as usize);
                    let q: BigRational = row["proposal"].as_str().unwrap().parse().unwrap();
                    let p: BigRational = row["physical"][rate].as_str().unwrap().parse().unwrap();
                    let likelihood = &p / &q;
                    let flips = if pattern[1] == 0 { vec![] } else { vec![0] };
                    let mut paulis = Vec::new();
                    for (choice, anchor, wire) in [
                        (pattern[0], anchors[0], 0),
                        (pattern[2], anchors[1], 1),
                        (pattern[3], anchors[2], 1),
                    ] {
                        if choice != 0 {
                            paulis.push(SuppliedPauliAction {
                                anchor,
                                wire,
                                basis: [Basis::X, Basis::Z, Basis::Y][choice - 1],
                            });
                        }
                    }
                    let true_bit = matches!(pattern[0], 1 | 3);
                    let reported = true_bit ^ (pattern[1] != 0);
                    let accepted = !reported;
                    let wrong = reported ^ matches!(pattern[2], 1 | 3) ^ (pattern[3] != 0);
                    let before = calls.lock().unwrap().len();
                    let plan = TaskEvaluator::prepare_supplied_actions(
                        &physical,
                        &modules,
                        &registry,
                        options,
                        SuppliedNoiseActions {
                            paulis: &paulis,
                            report_flips: &flips,
                            likelihood: &likelihood,
                        },
                        SuppliedActionLimits::default(),
                    )
                    .unwrap();
                    let draw = plan.sample(&mut SplitMix64::new(19)).unwrap();
                    let recorded = calls.lock().unwrap();
                    assert_eq!(recorded.len(), before + 1);
                    assert_eq!(
                        recorded[before],
                        vec![reported],
                        "decoder must receive only the reported bit"
                    );
                    drop(recorded);
                    if true_bit != reported {
                        distinct_true_reported += 1;
                    }
                    assert_eq!(draw.sampled_records, vec![reported]);
                    assert_eq!(draw.history_mass, ExactScalar::one());
                    assert_eq!(draw.noise_probability, p);
                    assert_eq!(draw.noise_likelihood_ratio, likelihood);
                    let scalar = |b: bool| {
                        if b {
                            ExactScalar::one()
                        } else {
                            ExactScalar::zero()
                        }
                    };
                    assert_eq!(draw.raw_acceptance, scalar(accepted));
                    assert_eq!(draw.raw_error, scalar(accepted && wrong));
                    let weight = options.fields.from_rational(likelihood).unwrap();
                    assert_eq!(
                        draw.accepted,
                        if accepted {
                            weight.clone()
                        } else {
                            ExactField::zero()
                        }
                    );
                    assert_eq!(
                        draw.error,
                        if accepted && wrong {
                            weight
                        } else {
                            ExactField::zero()
                        }
                    );
                    acceptance = options
                        .fields
                        .add(
                            &acceptance,
                            &options.fields.scale(&draw.accepted, &q).unwrap(),
                        )
                        .unwrap();
                    error = options
                        .fields
                        .add(&error, &options.fields.scale(&draw.error, &q).unwrap())
                        .unwrap();
                    if accepted {
                        direct_acceptance += &p;
                        if wrong {
                            direct_error += &p;
                        }
                    }
                    sum_p += p;
                    sum_q += q;
                    samples += 1;
                }
                assert_eq!(sum_p, BigRational::from_integer(1.into()));
                assert_eq!(sum_q, BigRational::from_integer(1.into()));
                assert_eq!(
                    acceptance,
                    options.fields.from_rational(direct_acceptance).unwrap()
                );
                assert_eq!(error, options.fields.from_rational(direct_error).unwrap());
            }
        }
    }
    assert_eq!(samples, 768);
    assert_eq!(distinct_true_reported, 384);
    json!({"schema":"rmagna.paper-proposal-integration.v1","status":"PASS","samples":samples,
        "true_reported_differ":distinct_true_reported,"decoder_calls":calls.lock().unwrap().len(),
        "rates":3,"causal_strategies":2,"closing_policies":2,"patterns_per_policy":64,
        "fixture_sha256":format!("{:x}",Sha256::digest(raw)),
        "source_sha256":format!("{:x}",Sha256::digest(include_bytes!("paper_proposal_probe.rs")))})
}
