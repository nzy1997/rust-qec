//! Addressed full-T draws with exact contribution histograms and bounded chunks.
#[path = "support/cultivation_flow.rs"]
mod cultivation_flow;
#[path = "support/cultivation_proposal.rs"]
mod cultivation_proposal;
#[path = "support/cultivation_rng.rs"]
mod cultivation_rng;

use cultivation_proposal::{Sampler, WeightKey};
use num_rational::BigRational;
use rmagna::circuit::{ParseOptions, parse_circuit};
use rmagna::costs::{ExecutionLimits, ExecutionMeter};
use rmagna::field::ExactField;
use rmagna::instrument::{InsertionPreparation, QueryPreparation};
use rmagna::noise::{NoiseSampling, OriginalNoiseOptions, original_noise_categories};
use rmagna::phase::{NativePhaseOptions, NativeTransportMode};
use rmagna::runtime::{ControllerRegistry, HistorySampler};
use rmagna::task::{EvaluationOptions, TaskEvaluator, TaskOptions, parse_task};
use rmagna::unitary::{CompilerStrategy, InsertionPreparationOptions};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::HashMap, error::Error, path::Path, time::Instant};

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Configuration {
    schema: String,
    physical_rate: String,
    mode: String,
    closing_sites: usize,
    seed: u32,
    start: u32,
    shots: u32,
    max_signature_work: u64,
    max_entropy_words: usize,
    max_histogram_entries: usize,
}

#[derive(PartialEq, Eq, Hash)]
struct Contribution {
    weight: WeightKey,
    accepted: ExactField,
    error: ExactField,
}

fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn inventory(evaluator: &TaskEvaluator) -> Result<Value, Box<dyn Error>> {
    Ok(json!({"work":evaluator.query_work_statistics(),
        "storage":evaluator.reference_storage_statistics()?,
        "source_assignments":evaluator.source_insertion_assignments()}))
}

fn resources(evaluator: &TaskEvaluator) -> Result<Value, Box<dyn Error>> {
    let export = |s: rmagna::instrument::PreparedQueryStatistics| {
        json!({
        "queries":s.cached_queries,"resources":s.fixed_resources,
        "maximum_quotient_rank":s.maximum_quotient_rank,"maximum_terms":s.maximum_terms})
    };
    Ok(
        json!({"acceptance":evaluator.prepared_probability_statistics()?.map(export),
        "target":evaluator.prepared_density_statistics()?.map(export)}),
    )
}

pub(crate) fn run(mut args: impl Iterator<Item = String>) -> Result<Value, Box<dyn Error>> {
    let physical_path = args
        .next()
        .ok_or("usage: paper_cultivation_run PHYSICAL TASK CONFIGURATION")?;
    let task_path = args.next().ok_or("missing task JSON path")?;
    let config_path = args.next().ok_or("missing configuration JSON path")?;
    if args.next().is_some() {
        return Err("unexpected argument after configuration path".into());
    }
    let config: Configuration = serde_json::from_str(&std::fs::read_to_string(config_path)?)?;
    let end = config
        .start
        .checked_add(config.shots)
        .ok_or("cultivation range overflow")?;
    if config.schema != "rmagna.paper-cultivation-configuration.v1"
        || config.shots == 0
        || config.shots > 1 << 20
        || end == u32::MAX
        || !(1..=1 << 24).contains(&config.max_entropy_words)
        || !(1..=1 << 18).contains(&config.max_histogram_entries)
        || !(1..=1 << 36).contains(&config.max_signature_work)
        || !matches!(
            (config.mode.as_str(), config.closing_sites),
            ("physical", 0) | ("closing", 1 | 2)
        )
    {
        return Err("invalid bounded cultivation configuration".into());
    }
    let physical_text = std::fs::read_to_string(&physical_path)?;
    let task_text = std::fs::read_to_string(&task_path)?;
    let parse = ParseOptions {
        source: Some(physical_path.clone()),
        ..Default::default()
    };
    let physical = parse_circuit(&physical_text, &parse)?;
    let spec = parse_task(
        &task_text,
        Path::new(&task_path).parent().unwrap_or(Path::new(".")),
        &TaskOptions::default(),
    )?;
    let modules = spec.load_modules(&physical, &parse)?;
    let ideal_sha256 = sha(&std::fs::read(
        spec.ideal_state()
            .ok_or("missing full ideal target")?
            .circuit_path(),
    )?);
    if physical.needs_history_sampling()
        || physical.external_calls() != 0
        || modules.recovery().is_some()
        || modules.task().outputs().len() != 1
        || modules
            .ideal_state()
            .is_none_or(|target| target.outputs().len() != 1)
    {
        return Err("cultivation requires the full one-output T task/target and no history/recovery/callbacks".into());
    }
    let categories = original_noise_categories(&physical, &OriginalNoiseOptions::default())?;
    // Always derive and report the exact safe-constraint boundary, including
    // physical acceptance runs. It is never a replacement quantum circuit.
    let closure = cultivation_flow::prepare(&physical, &categories)?;
    let flow = json!({"retained":closure.retained,"unsafe_t":closure.unsafe_t,
        "signature_words":closure.signature_words,"offset":closure.offset.ones().collect::<Vec<_>>()});
    let physical_rate: BigRational = config.physical_rate.parse()?;
    let closing = (config.mode == "closing").then_some((closure, config.closing_sites));
    let sampler = Sampler::new(&categories, physical_rate, closing)?;
    let mut native = NativePhaseOptions::default();
    native.selection.max_quotient_rank = 0;
    native.selection.max_signature_work = u128::from(config.max_signature_work);
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
    let registry = ControllerRegistry::default();
    let meter = ExecutionMeter::new(ExecutionLimits::default());
    let started = Instant::now();
    let evaluator = TaskEvaluator::new_with_costs(
        &physical,
        &modules,
        &registry,
        options,
        &NoiseSampling::Physical,
        &meter,
    )?;
    let compile_seconds = started.elapsed().as_secs_f64();
    let insertions = evaluator
        .prepared_insertion_statistics()
        .ok_or("missing cultivation insertion catalog")?;
    let prepared = evaluator.prepare_sampling_queries(Default::default(), &meter);
    prepared.result.clone()?;
    if !prepared.all_families_retained() {
        return Err("cultivation query preparation was not fully retained".into());
    }
    let preparation_seconds = started.elapsed().as_secs_f64();
    let after_preparation = inventory(&evaluator)?;
    // u32::MAX is reserved and disjoint from every production range.
    let mut random = cultivation_rng::draw_stream(config.seed, u32::MAX);
    let warmup = sampler.draw(&mut random, config.max_entropy_words)?;
    evaluator.linear_prescribed_contributions(&warmup.faults)?;
    let after_warmup = inventory(&evaluator)?;
    let mut histogram: HashMap<Contribution, u64> = HashMap::new();
    let mut completed = 0u32;
    let mut entropy_words = 0u64;
    let mut failure = None;
    let sampling = Instant::now();
    for index in config.start..end {
        let result = (|| -> Result<(), Box<dyn Error>> {
            let mut random = cultivation_rng::draw_stream(config.seed, index);
            let draw = sampler.draw(&mut random, config.max_entropy_words)?;
            let result = evaluator.linear_prescribed_contributions(&draw.faults)?;
            if result.acceptance_resources.quotient_rank != 0
                || result.acceptance_resources.terms > 1
                || result
                    .target_resources
                    .is_none_or(|s| s.quotient_rank != 0 || s.terms > 1)
            {
                return Err(
                    "cultivation draw violated its fixed prepared full-T contribution policy"
                        .into(),
                );
            }
            let key = Contribution {
                weight: draw.weight,
                accepted: result.accepted,
                error: result.error,
            };
            if histogram.len() == config.max_histogram_entries && !histogram.contains_key(&key) {
                return Err("cultivation max_histogram_entries exceeded".into());
            }
            *histogram.entry(key).or_default() += 1;
            entropy_words += draw.entropy_words as u64;
            completed += 1;
            Ok(())
        })();
        if let Err(error) = result {
            failure = Some(json!({"index":index,"reason":error.to_string()}));
            break;
        }
        if completed.is_multiple_of(4096) {
            eprintln!(
                "cultivation start={} completed={completed}/{}",
                config.start, config.shots
            );
        }
    }
    let sampling_seconds = sampling.elapsed().as_secs_f64();
    let mut rows = histogram.into_iter().map(|(key,count)| json!({
        "weight":key.weight,"accepted":key.accepted.coefficients().each_ref().map(ToString::to_string),
        "error":key.error.coefficients().each_ref().map(ToString::to_string),"count":count
    })).collect::<Vec<_>>();
    rows.sort_by_cached_key(Value::to_string);
    Ok(json!({"schema":"rmagna.paper-cultivation-run.v1",
        "status":if failure.is_none() {"complete"} else {"failed"},"failure":failure,
        "configuration":config,"completed":completed,"histogram":rows,
        "output_contract":"unit_prescribed_full_T_joint_acceptance_error_with_external_P_over_Q",
        "rng_protocol":cultivation_rng::PROTOCOL,"entropy_words":entropy_words,
        "law":{"physical_rate":sampler.physical_rate.to_string(),"proposal_rate":sampler.proposal_rate.to_string(),
            "original_sites":sampler.num_sites(),"conditional_channel_counts":categories.iter().map(|c|c.channels.len()).collect::<Vec<_>>(),
            "mixture_alpha":"19/20","flow":flow},
        "rust_source_sha256":env!("RMAGNA_PAPER_SOURCE_SHA256"),
        "executable_sha256":sha(&std::fs::read(std::env::current_exe()?)?),
        "inputs":{"physical_sha256":sha(physical_text.as_bytes()),"task_sha256":sha(task_text.as_bytes()),"ideal_sha256":ideal_sha256},
        "build":{"debug_assertions":cfg!(debug_assertions),"architecture":std::env::consts::ARCH,"os":std::env::consts::OS},
        "preparation":{"seconds":preparation_seconds,"compile_seconds":compile_seconds,
            "planned":prepared.counts.planned,"created":prepared.counts.created,"reused":prepared.counts.reused,
            "insertions":{"catalog_entries":insertions.catalog_entries,"estimated_peak_bytes":insertions.estimated_peak_bytes}},
        "excluded_warmup":{"draws":1,"seed":config.seed,"index":u32::MAX},
        "sampling_seconds":sampling_seconds,
        "inventory":{"after_preparation":after_preparation,"after_warmup":after_warmup,"after_sampling":inventory(&evaluator)?},
        "resources":resources(&evaluator)?}))
}

fn main() {
    let value = run(std::env::args().skip(1)).unwrap_or_else(|error| {
        json!({
        "schema":"rmagna.paper-cultivation-run.v1","status":"failed","failure":error.to_string()})
    });
    println!("{value}");
    if value["status"] != "complete" {
        std::process::exit(1);
    }
}
