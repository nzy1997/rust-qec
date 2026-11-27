//! Exact three-fault contribution for the pinned RP2 d5-T benchmark input.
#[allow(dead_code)]
#[path = "support/cultivation_flow.rs"]
mod cultivation_flow;

use num_rational::BigRational;
use num_traits::{One, ToPrimitive, Zero};
use rmagna::circuit::{ParseOptions, parse_circuit};
use rmagna::costs::{ExecutionLimits, ExecutionMeter};
use rmagna::instrument::{FaultSelection, InsertionPreparation, QueryPreparation};
use rmagna::noise::{OriginalNoiseOptions, original_noise_categories};
use rmagna::phase::{NativePhaseOptions, NativeTransportMode};
use rmagna::runtime::HistorySampler;
use rmagna::task::{EvaluationOptions, TaskEvaluator, TaskOptions, parse_task};
use rmagna::unitary::{CompilerStrategy, InsertionPreparationOptions};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};
use std::error::Error;
use std::path::Path;
use std::time::Instant;

type Detector = [u64; 3];
type Score = [u64; 6];
type Fault = (usize, usize);
type Pattern = [Fault; 3];
type Coefficients = [BigRational; 4];

struct ChannelClass {
    score: Score,
    alternatives: Vec<Fault>,
    witness_choices: Vec<Fault>,
}

fn xor<const N: usize>(a: [u64; N], b: [u64; N]) -> [u64; N] {
    std::array::from_fn(|i| a[i] ^ b[i])
}

fn scan_classes(
    groups: &BTreeMap<Detector, Vec<ChannelClass>>,
    mut visit: impl FnMut(Score, [&ChannelClass; 3]),
) {
    for (&d, first) in groups {
        for (&e, second) in groups.range(d..) {
            let f = xor(d, e);
            if f < e {
                continue;
            }
            let Some(third) = groups.get(&f) else {
                continue;
            };
            for (i, a) in first.iter().enumerate() {
                let second_start = if d == e { i } else { 0 };
                for (j, b) in second.iter().enumerate().skip(second_start) {
                    let third_start = if e == f { j } else { 0 };
                    let pair = xor(a.score, b.score);
                    for c in &third[third_start..] {
                        visit(xor(pair, c.score), [a, b, c]);
                    }
                }
            }
        }
    }
}

fn witness([a, b, c]: [&ChannelClass; 3]) -> Option<Pattern> {
    // With three classes, retaining the first three different sites in each
    // class preserves feasibility of any assignment to three distinct sites.
    for &x in &a.witness_choices {
        for &y in &b.witness_choices {
            for &z in &c.witness_choices {
                if x.0 != y.0 && y.0 != z.0 && x.0 != z.0 {
                    let mut pattern = [x, y, z];
                    pattern.sort_unstable();
                    return Some(pattern);
                }
            }
        }
    }
    None
}

fn check_input(path: &Path, bytes: &[u8], sha256: &str) -> Result<(), Box<dyn Error>> {
    if format!("{:x}", Sha256::digest(bytes)) != sha256 {
        return Err(format!(
            "expected the pinned public Magna RP2 d5-T input at {}",
            path.display()
        )
        .into());
    }
    Ok(())
}

fn run(input: &Path, jobs: usize) -> Result<Value, Box<dyn Error>> {
    let start = Instant::now();
    let physical_path = input.join("physical.magna");
    let task_path = input.join("task.json");
    let target_path = input.join("target.magna");
    let physical_text = std::fs::read_to_string(&physical_path)?;
    let task_text = std::fs::read_to_string(&task_path)?;
    check_input(
        &physical_path,
        physical_text.as_bytes(),
        "b96a03334391c439e3497702e8037cf410cd1f4e83f1df87bc538a17ca04253c",
    )?;
    check_input(
        &task_path,
        task_text.as_bytes(),
        "23b9d9a1b9ea570a183abe07e3b588ae11bf42fec24847ef113983e2cbfb2f08",
    )?;
    check_input(
        &target_path,
        &std::fs::read(&target_path)?,
        "7a22a487ecd642413901c4d8bcd61d32288f54e1b80671a3220659e2f87a67b3",
    )?;
    let parse = ParseOptions {
        source: Some(physical_path.display().to_string()),
        ..Default::default()
    };
    let physical = parse_circuit(&physical_text, &parse)?;
    let spec = parse_task(&task_text, input, &TaskOptions::default())?;
    let modules = spec.load_modules(&physical, &parse)?;
    let categories = original_noise_categories(&physical, &OriginalNoiseOptions::default())?;
    let closure = cultivation_flow::prepare(&physical, &categories)?;
    if categories.len() != 2405
        || closure.retained.len() != 102
        || closure.offset.ones().next().is_some()
    {
        return Err("the pinned noise categories or signed detector constraints changed".into());
    }

    let mut native = NativePhaseOptions::default();
    native.selection.max_quotient_rank = 0;
    native.selection.max_signature_work = 1 << 34;
    native.selection.preparation.max_terms = 1;
    native.transport.mode = NativeTransportMode::Prepared;
    let mut options = EvaluationOptions::default();
    options.runtime.history_sampler = HistorySampler::AffineSupport;
    options.runtime.sampling.max_terms = 1;
    options.runtime.compile.compiler_strategy = CompilerStrategy::SharedCommutingLayer;
    options.runtime.compile.max_path_bits = 65536;
    options.runtime.compile.insertion_preparation =
        InsertionPreparation::Prepared(InsertionPreparationOptions {
            max_table_bytes: 128 << 20,
            ..Default::default()
        });
    options.runtime.compile.probability_preparation = QueryPreparation::Native(native);
    options.runtime.compile.density_preparation = QueryPreparation::Native(native);
    options.runtime.compile.moment_preparation = QueryPreparation::Native(native);
    let meter = ExecutionMeter::new(ExecutionLimits::default());
    let evaluator =
        TaskEvaluator::new_linear_prescribed_with_costs(&physical, &modules, options, &meter)?;
    let prepared = evaluator.prepare_sampling_queries(Default::default(), &meter);
    prepared.result.clone()?;
    if !prepared.all_families_retained() {
        return Err("exact query preparation did not retain all families".into());
    }
    let labels = evaluator
        .linear_target_fault_signatures(1 << 20)?
        .ok_or("exact XOR score labels unavailable")?;

    let mut alternatives: BTreeMap<Detector, BTreeMap<Score, Vec<Fault>>> = BTreeMap::new();
    let p = BigRational::new(1.into(), 1000.into());
    let mut channel_counts = HashMap::new();
    for (category_index, category) in categories.iter().enumerate() {
        let site = category.internal_site;
        let count = category.channels.len();
        if count == 0 || 15 % count != 0 {
            return Err("unexpected alternative count at a pinned noise category".into());
        }
        let channel_probability = &p / BigRational::from_integer(count.into());
        if category.guard.is_some()
            || category.identity_probability.value() != &(BigRational::one() - &p)
            || category
                .channels
                .iter()
                .any(|c| c.probability.value() != &channel_probability)
        {
            return Err(
                "expected independent equal-probability alternatives at each category".into(),
            );
        }
        channel_counts.insert(site, count);
        for alternative in 0..count {
            let detector = closure.signatures[category_index][alternative]
                .words()
                .try_into()?;
            let label = labels[site][alternative].words().try_into()?;
            alternatives
                .entry(detector)
                .or_default()
                .entry(label)
                .or_default()
                .push((site, alternative));
        }
    }
    let groups: BTreeMap<_, _> = alternatives
        .into_iter()
        .map(|(detector, classes)| {
            let classes: Vec<_> = classes
                .into_iter()
                .map(|(score, alternatives)| {
                    let mut witness_choices = Vec::new();
                    for &fault in &alternatives {
                        if !witness_choices.iter().any(|&(site, _)| site == fault.0) {
                            witness_choices.push(fault);
                        }
                        if witness_choices.len() == 3 {
                            break;
                        }
                    }
                    ChannelClass {
                        score,
                        alternatives,
                        witness_choices,
                    }
                })
                .collect();
            (detector, classes)
        })
        .collect();

    let mut class_triples = 0;
    let mut witnesses = HashMap::new();
    scan_classes(&groups, |score, classes| {
        class_triples += 1;
        if let std::collections::hash_map::Entry::Vacant(entry) = witnesses.entry(score)
            && let Some(pattern) = witness(classes)
        {
            entry.insert(pattern);
        }
    });
    let cases: Vec<_> = witnesses.into_iter().collect();
    if cases.is_empty() {
        return Err("no admissible closure classes were enumerated".into());
    }
    eprintln!("Scoring {} full-T classes on {jobs} workers", cases.len());
    let positive = std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for chunk in cases.chunks(cases.len().div_ceil(jobs)) {
            let evaluator = &evaluator;
            handles.push(scope.spawn(move || {
                let mut positive = HashMap::new();
                for (score, pattern) in chunk {
                    let faults =
                        pattern.map(|(site, alternative)| FaultSelection { site, alternative });
                    let masses = evaluator
                        .linear_prescribed_contributions(&faults)
                        .map_err(|e| e.to_string())?;
                    if !masses.error.is_zero() {
                        positive.insert(*score, (masses.accepted, masses.error));
                    }
                }
                Ok::<_, String>(positive)
            }));
        }
        let mut positive = HashMap::new();
        for handle in handles {
            positive.extend(
                handle
                    .join()
                    .map_err(|_| "exact scoring worker panicked")??,
            );
        }
        Ok::<_, String>(positive)
    })?;

    // Expand all physical patterns in the positive classes. Repeated labels and
    // three distinct-site alternatives can generate the same unordered pattern.
    let mut patterns = BTreeMap::new();
    scan_classes(&groups, |score, [a, b, c]| {
        if !positive.contains_key(&score) {
            return;
        }
        for &x in &a.alternatives {
            for &y in &b.alternatives {
                for &z in &c.alternatives {
                    if x.0 != y.0 && x.0 != z.0 && y.0 != z.0 {
                        let mut pattern = [x, y, z];
                        pattern.sort_unstable();
                        patterns.insert(pattern, score);
                    }
                }
            }
        }
    });
    let mut multiplier: Coefficients = std::array::from_fn(|_| BigRational::zero());
    let mut positive_faults = Vec::new();
    for (pattern, score) in patterns {
        let (accepted, error) = &positive[&score];
        let denominator = pattern
            .iter()
            .map(|(site, _)| channel_counts[site])
            .product::<usize>();
        let channel_mass = BigRational::new(1.into(), denominator.into());
        for (total, coefficient) in multiplier.iter_mut().zip(error.coefficients()) {
            *total += coefficient * &channel_mass;
        }
        positive_faults.push(json!({
            "faults":pattern,
            "accepted":accepted.coefficients().each_ref().map(ToString::to_string),
            "error":error.coefficients().each_ref().map(ToString::to_string),
        }));
    }
    let [a, b, c, d] = &multiplier;
    if !c.is_zero() || b != &-d {
        return Err("the joint error is not real".into());
    }
    let prefactor = p.pow(3) * (BigRational::one() - p).pow(2402);
    let real = (a * &prefactor)
        .to_f64()
        .ok_or("joint error is outside binary64")?;
    let radical = (b * prefactor)
        .to_f64()
        .ok_or("joint error is outside binary64")?;
    let decimal = real + radical * 2_f64.sqrt();
    Ok(json!({
        "status":"complete",
        "noise_categories":categories.len(),
        "retained_detectors":closure.retained,
        "eligible_class_triples":class_triples,
        "full_t_score_classes":cases.len(),
        "positive_score_classes":positive.len(),
        "positive_configurations":positive_faults.len(),
        "joint_error_multiplier":multiplier.each_ref().map(ToString::to_string),
        "joint_error_factor":"p^3 * (1-p)^2402",
        "joint_error":decimal,
        "sampling_se":0,
        "wall_seconds":start.elapsed().as_secs_f64(),
        "positive_faults":positive_faults,
    }))
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: paper_cultivation_exact3 RP2_D5_INPUT_DIR WORKERS".into());
    }
    let jobs = args[1]
        .to_str()
        .ok_or("invalid worker count")?
        .parse::<usize>()?;
    if jobs == 0 {
        return Err("worker count must be positive".into());
    }
    println!("{}", run(Path::new(&args[0]), jobs)?);
    Ok(())
}
