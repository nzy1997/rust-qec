//! Complete original supplied-noise causal-chain method at
//! itpyi/magna@56847478c840395735146ef1e1ccdc3d8aedc10f.
use num_rational::BigRational;
use rmagna::circuit::{ParseOptions, parse_circuit};
use rmagna::field::{ExactField, FieldArithmetic};
use rmagna::instrument::FaultSelection;
use rmagna::noise::SplitMix64;
use rmagna::runtime::{CausalStrategy, ControllerRegistry, HistorySampler};
use rmagna::scalar::{ExactScalar, ScalarArithmetic};
use rmagna::task::{
    EvaluationOptions, TaskEnumerationOptions, TaskEvaluation, TaskEvaluator, TaskOptions,
    parse_task,
};
use rmagna::unitary::CompilerStrategy;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

struct TargetFile(PathBuf);
impl TargetFile {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "rmagna-original-supplied-causal-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        std::fs::write(path.join("target.magna"), "I 0").unwrap();
        Self(path)
    }
}
impl Drop for TargetFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn rational(n: i64, d: i64) -> BigRational {
    BigRational::new(n.into(), d.into())
}
fn field(n: i64, d: i64) -> ExactField {
    FieldArithmetic::default()
        .from_rational(rational(n, d))
        .unwrap()
}
fn original_source() -> &'static str {
    "H 0\nM 0\nX_ERROR(1/3) rec[-1] 1\nM 1\nDETECTOR rec[-1]\nCONTROL[echo] det[-1] -> ctrl[0]\nX_ERROR(1/2) ctrl[0] 2"
}
fn supplied_candidate() -> Vec<FaultSelection> {
    vec![
        FaultSelection {
            site: 1,
            alternative: 0,
        },
        FaultSelection {
            site: 3,
            alternative: 0,
        },
    ]
}
fn check_draw(draw: &TaskEvaluation, new_calls: &[Vec<bool>]) -> bool {
    assert_eq!(draw.sampled_records.len(), 2);
    let a = draw.sampled_records[0];
    let b = draw.sampled_records[1];
    // Preserve all original per-draw assertions, before supplementary checks.
    assert_eq!(a, b, "original causal record equality");
    assert_eq!(
        new_calls,
        &[vec![b]],
        "original echo callback value and count"
    );
    assert_eq!(draw.controller_calls.len(), 1);
    assert_eq!(draw.accepted, field(1, 1));
    assert_eq!(
        draw.error,
        field(i64::from(a), 1),
        "original error equals first record"
    );
    assert_eq!(
        draw.history_mass,
        ScalarArithmetic::default().omega(0, -2).unwrap()
    );
    let call = &draw.controller_calls[0];
    assert_eq!(call.name, "echo");
    assert_eq!(call.call, 0);
    assert_eq!(call.inputs, [b]);
    assert_eq!(call.outputs, [b]);
    assert_eq!(draw.target_success, Some(field(i64::from(!a), 1)));
    assert_eq!(draw.target_normalization, Some(ExactScalar::one()));
    assert_eq!(draw.moments, [field(if a { -1 } else { 1 }, 1)]);
    assert_eq!(draw.history_weight, field(1, 1));
    if let Some(proposal) = &draw.history_proposal_mass {
        assert_eq!(proposal, &draw.history_mass);
    }
    assert_eq!(
        draw.noise_probability,
        if a { rational(1, 6) } else { rational(1, 1) }
    );
    assert_eq!(draw.noise_likelihood_ratio, rational(1, 1));
    a
}

#[test]
fn original_supplied_candidate_respects_guard_and_causal_chain() {
    let files = TargetFile::new();
    let calls = Arc::new(Mutex::new(Vec::<Vec<bool>>::new()));
    let mut registry = ControllerRegistry::default();
    let echo_calls = Arc::clone(&calls);
    registry
        .register("echo", "1", move |bits: &[bool]| {
            echo_calls.lock().unwrap().push(bits.to_vec());
            Ok(bits.to_vec())
        })
        .unwrap();
    let parse = ParseOptions {
        controllers: registry.names(),
        ..Default::default()
    };
    let physical = parse_circuit(original_source(), &parse).unwrap();
    let sampling_task = parse_task(
        r#"{"outputs":[2],"sample_records":[0,1],"ideal_state":"target.magna","observables":[[0,1]]}"#,
        &files.0, &TaskOptions::default(),
    ).unwrap();
    let sampling_modules = sampling_task.load_modules(&physical, &parse).unwrap();
    // The original enumeration tasks omit sampling-only diagnostic records.
    let enumeration_task = parse_task(
        r#"{"outputs":[2],"ideal_state":"target.magna","observables":[[0,1]]}"#,
        &files.0,
        &TaskOptions::default(),
    )
    .unwrap();
    let enumeration_modules = enumeration_task.load_modules(&physical, &parse).unwrap();
    let faults = supplied_candidate();
    let mut sample_calls = 0;
    for compiler in [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ] {
        for strategy in [CausalStrategy::FixedTemplate, CausalStrategy::ColdPrefix] {
            for proposal in [HistorySampler::ExactMarginal, HistorySampler::AffineSupport] {
                let mut options = EvaluationOptions::default();
                options.runtime.compile.compiler_strategy = compiler;
                options.runtime.strategy = strategy;
                options.runtime.history_sampler = proposal;
                let sampling =
                    TaskEvaluator::new(&physical, &sampling_modules, &registry, options).unwrap();
                let mut observed = [false; 2];
                for seed in 0..32 {
                    let mut records = Vec::new();
                    for _ in 0..2 {
                        let before = calls.lock().unwrap().len();
                        let draw = sampling
                            .sample_prescribed(&faults, &mut SplitMix64::new(seed))
                            .unwrap();
                        sample_calls += 1;
                        let trace = calls.lock().unwrap();
                        let a = check_draw(&draw, &trace[before..]);
                        observed[usize::from(a)] = true;
                        records.push(draw.sampled_records);
                    }
                    // Compare semantics, independent of cache/resource counters.
                    assert_eq!(records[0], records[1], "same-seed replay records");
                }
                assert_eq!(observed, [true, true]);
                let enumeration =
                    TaskEvaluator::new(&physical, &enumeration_modules, &registry, options)
                        .unwrap();
                let mut limits = TaskEnumerationOptions::default();
                limits.noise.max_weight = 2;
                limits.include_patterns = true;
                let full = enumeration.enumerate_physical(&limits).unwrap();
                assert_eq!(full.masses.error, field(1, 12), "original full exact error");
                assert_eq!(full.covered_probability.value(), &rational(1, 1));
                assert_eq!(full.omitted_probability.value(), &rational(0, 1));
                assert_eq!(full.masses.accepted, field(1, 1));
                assert_eq!(full.masses.target_success, Some(field(11, 12)));
                assert_eq!(full.masses.moments, [field(5, 6)]);
                assert_eq!((full.patterns, full.histories), (4, 8));
                let rows = full.pattern_responses.unwrap();
                for (sites, probability, error) in [
                    (vec![], rational(1, 3), field(0, 1)),
                    (vec![1], rational(1, 6), field(0, 1)),
                    (vec![3], rational(1, 3), field(0, 1)),
                    (vec![1, 3], rational(1, 6), field(1, 2)),
                ] {
                    let choices = sites
                        .into_iter()
                        .map(|site| FaultSelection {
                            site,
                            alternative: 0,
                        })
                        .collect::<Vec<_>>();
                    let row = rows.iter().find(|row| row.choices == choices).unwrap();
                    assert_eq!(row.probability.value(), &probability);
                    assert_eq!(row.histories, 2);
                    assert_eq!(row.response.accepted, field(1, 1));
                    assert_eq!(row.response.error, error);
                }
                limits.noise.max_weight = 1;
                let truncated = enumeration.enumerate_physical(&limits).unwrap();
                assert_eq!(truncated.covered_probability.value(), &rational(5, 6));
                assert_eq!(truncated.masses.error, field(0, 1));
                assert_eq!(truncated.omitted_probability.value(), &rational(1, 6));
                assert_eq!(truncated.masses.accepted, field(5, 6));
                assert_eq!(truncated.masses.target_success, Some(field(5, 6)));
                assert_eq!(truncated.masses.moments, [field(5, 6)]);
                assert_eq!((truncated.patterns, truncated.histories), (3, 6));
            }
        }
    }
    assert_eq!(sample_calls, 512); // 256 primary draws plus 256 same-seed replays.
}
