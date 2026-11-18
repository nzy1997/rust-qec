//! Bounded, addressed code-switching draws for the paper evidence capture.
#[path = "support/cultivation_rng.rs"]
mod addressed_rng;
#[path = "support/decoder_worker.rs"]
mod decoder_worker;

use num_rational::BigRational;
use num_traits::{One, Zero};
use rmagna::circuit::{ParseOptions, parse_circuit};
use rmagna::costs::ExecutionMeter;
use rmagna::field::ExactField;
use rmagna::instrument::{FaultSelection, QueryPreparation};
use rmagna::noise::{
    NoiseModel, NoiseOptions, NoiseSampler, OriginalNoiseOptions, original_noise_categories,
};
use rmagna::phase::NativePhaseOptions;
use rmagna::probability::Probability;
use rmagna::runtime::process::ControllerProcess;
use rmagna::runtime::{ControllerRegistry, HistorySampler};
use rmagna::task::{EvaluationOptions, TaskEvaluator, TaskOptions, parse_task};
use rmagna::unitary::CompilerStrategy;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};
use std::error::Error;
use std::path::{Path, PathBuf};
use std::time::Instant;

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Configuration {
    schema: String,
    family: String,
    physical_rate: String,
    proposal_rate: String,
    noise_seed: u32,
    born_seed: u32,
    start: u32,
    shots: u32,
    max_histogram_entries: usize,
}

#[derive(Eq, Hash, PartialEq)]
struct Contribution {
    occurrences: usize,
    acceptance: ExactField,
    error: ExactField,
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn run(mut args: impl Iterator<Item = String>) -> Result<Value, Box<dyn Error>> {
    let started = Instant::now();
    let physical_path = args
        .next()
        .ok_or("usage: paper_decoder_run PHYSICAL TASK CONFIGURATION [CONTROLLERS PYTHON]")?;
    let task_path = PathBuf::from(args.next().ok_or("missing task path")?);
    let config_path = args.next().ok_or("missing configuration path")?;
    let config: Configuration = serde_json::from_str(&std::fs::read_to_string(config_path)?)?;
    let (expected_sites, expected_calls) = match config.family.as_str() {
        "dk" => (184, 1),
        "wg-S" | "wg-T" => (730, 0),
        _ => return Err("unknown code-switching family".into()),
    };
    let end = config
        .start
        .checked_add(config.shots)
        .ok_or("draw range overflow")?;
    if config.schema != "rmagna.paper-code-switch-configuration.v1"
        || !(1..=1 << 20).contains(&config.shots)
        || !(1..=1 << 18).contains(&config.max_histogram_entries)
        || config.noise_seed == config.born_seed
    {
        return Err("invalid bounded code-switching configuration".into());
    }
    let p: BigRational = config.physical_rate.parse()?;
    let q: BigRational = config.proposal_rate.parse()?;
    let thousandth = BigRational::new(1.into(), 1000.into());
    let three_thousandths = BigRational::new(3.into(), 1000.into());
    let hundredth = BigRational::new(1.into(), 100.into());
    if !((p == thousandth || p == three_thousandths) && q == three_thousandths
        || p == hundredth && q == hundredth
        || p == q && p == thousandth)
    {
        return Err("code-switching rates differ from the supported paper grid".into());
    }
    let mut registry = ControllerRegistry::default();
    let mut controller_sha256 = None;
    let process = if expected_calls != 0 {
        let path = PathBuf::from(args.next().ok_or("missing DK controller manifest")?);
        let python = args.next().ok_or("missing DK Python command")?;
        let process = decoder_worker::worker(&path, &python)?;
        process.register_all(&mut registry)?;
        controller_sha256 = Some(digest(&std::fs::read(path)?));
        Some(process)
    } else {
        None
    };
    if args.next().is_some() {
        return Err("unexpected code-switching arguments".into());
    }
    let physical_text = std::fs::read_to_string(&physical_path)?;
    let task_text = std::fs::read_to_string(&task_path)?;
    let parse = ParseOptions {
        source: Some(physical_path),
        controllers: registry.names(),
        ..Default::default()
    };
    let physical = parse_circuit(&physical_text, &parse)?;
    let spec = parse_task(
        &task_text,
        task_path.parent().unwrap_or(Path::new(".")),
        &TaskOptions::default(),
    )?;
    let modules = spec.load_modules(&physical, &parse)?;
    let recovery_calls = modules
        .recovery()
        .map_or(0, |circuit| circuit.external_calls());
    if physical.external_calls() + recovery_calls != expected_calls {
        return Err(
            "code-switching external call count differs from the pinned task design".into(),
        );
    }
    let recovery_sha256 = spec
        .recovery_path()
        .map(std::fs::read)
        .transpose()?
        .map(|bytes| digest(&bytes));
    let ideal_path = spec
        .ideal_state()
        .ok_or("code switching requires the full ideal target")?
        .circuit_path();
    let ideal_sha256 = digest(&std::fs::read(ideal_path)?);
    let noise_options = OriginalNoiseOptions::default();
    if let Some(recovery) = modules.recovery()
        && original_noise_categories(recovery, &noise_options)?
            .iter()
            .any(|category| !category.identity_probability.value().is_one())
    {
        return Err("code-switching recovery adds nontrivial fault categories".into());
    }
    let mut categories = original_noise_categories(&physical, &noise_options)?;
    let declared_sites = categories.len();
    // WG declares eight ideal readout sites with an exactly zero flip rate.
    // Keep their circuit/record identities and exclude them from both laws.
    categories.retain(|category| !category.identity_probability.value().is_one());
    let ideal_sites = declared_sites - categories.len();
    if categories.len() != expected_sites {
        return Err(format!(
            "expected {expected_sites} original noise categories, found {}",
            categories.len()
        )
        .into());
    }
    let mut rows = Vec::with_capacity(categories.len());
    let mut sites = Vec::with_capacity(categories.len());
    let identity = Probability::from_rational(BigRational::one() - &q)?;
    for category in &categories {
        let first = category
            .channels
            .first()
            .ok_or("empty code-switching noise category")?
            .probability
            .value();
        if category.guard.is_some()
            || first.is_zero()
            || category
                .channels
                .iter()
                .any(|channel| channel.probability.value() != first)
        {
            return Err("code switching requires independent positive, uniform conditional noise categories".into());
        }
        let count = category.channels.len();
        let alternative = Probability::from_rational(&q / BigRational::from_integer(count.into()))?;
        rows.push(
            std::iter::once(identity.clone())
                .chain(std::iter::repeat_n(alternative, count))
                .collect(),
        );
        sites.push(category.internal_site);
    }
    let noise_options = NoiseOptions::default();
    let sampler = NoiseSampler::physical(NoiseModel::new(rows, &noise_options)?, noise_options);
    let mut options = EvaluationOptions::default();
    let mut native = NativePhaseOptions::default();
    native.selection.max_quotient_rank = 0;
    native.selection.preparation.max_terms = 1;
    native.selection.max_signature_work = 1 << 36;
    options.runtime.history_sampler = HistorySampler::AffineSupport;
    options.runtime.sampling.max_terms = 1;
    options.runtime.compile.compiler_strategy = CompilerStrategy::SharedCommutingLayer;
    options.runtime.compile.max_path_bits = 65536;
    options.runtime.compile.probability_preparation = QueryPreparation::Native(native);
    options.runtime.compile.density_preparation = QueryPreparation::Native(native);
    options.runtime.compile.moment_preparation = QueryPreparation::Native(native);
    let compile = Instant::now();
    let evaluator = TaskEvaluator::new(&physical, &modules, &registry, options)?;
    let compile_seconds = compile.elapsed().as_secs_f64();
    let unit = BigRational::one();
    let draw = |index, meter: &ExecutionMeter| -> Result<_, Box<dyn Error>> {
        let mut noise = addressed_rng::draw_stream(config.noise_seed, index);
        let mut born = addressed_rng::draw_stream(config.born_seed, index);
        let generated = sampler.draw(&mut noise)?;
        let faults = generated
            .faults
            .iter()
            .map(|fault| FaultSelection {
                site: sites[fault.site],
                alternative: fault.alternative,
            })
            .collect::<Vec<_>>();
        let (acceptance, error, acceptance_resources, target_resources, calls) =
            if expected_calls == 0 {
                let value = evaluator.linear_prescribed_contributions(&faults)?;
                (
                    value.accepted,
                    value.error,
                    value.acceptance_resources,
                    value.target_resources,
                    Vec::new(),
                )
            } else {
                let value =
                    evaluator.sample_supplied_noise_with_costs(&faults, &unit, &mut born, meter)?;
                (
                    value.accepted,
                    value.error,
                    value.acceptance_resources,
                    value.target_resources,
                    value.controller_calls,
                )
            };
        if calls.len() != expected_calls
            || calls.iter().any(|call| {
                call.name != "steane_css" || call.inputs.len() != 6 || call.outputs.len() != 14
            })
            || acceptance_resources.quotient_rank != 0
            || acceptance_resources.terms > 1
            || target_resources.is_none_or(|r| r.quotient_rank != 0 || r.terms > 1)
        {
            return Err(
                "code-switching draw violated its target resources or decoder call policy".into(),
            );
        }
        Ok((
            Contribution {
                occurrences: faults.len(),
                acceptance,
                error,
            },
            calls.into_iter().next(),
        ))
    };
    // The maximum index is reserved for one excluded warmup, with fresh addressed entropy.
    draw(u32::MAX, &ExecutionMeter::default())?;
    let prepare_seconds = started.elapsed().as_secs_f64();
    let meter = ExecutionMeter::default();
    let mut histogram = HashMap::<Contribution, u64>::new();
    let mut decoder_transcript_digest = Sha256::new();
    let mut decoder_histogram = BTreeMap::<Vec<bool>, (Vec<bool>, u64)>::new();
    let mut completed = 0;
    let mut failure = None;
    let sampling = Instant::now();
    for index in config.start..end {
        match draw(index, &meter) {
            Ok((row, calls)) => {
                if histogram.len() == config.max_histogram_entries && !histogram.contains_key(&row)
                {
                    failure = Some(json!({"index":index,"error":"max_histogram_entries exceeded"}));
                    break;
                }
                if let Some(call) = calls {
                    let entry = decoder_histogram
                        .entry(call.inputs.clone())
                        .or_insert_with(|| (call.outputs.clone(), 0));
                    if entry.0 != call.outputs {
                        failure = Some(
                            json!({"index":index,"error":"inconsistent decoder response on repeated syndrome"}),
                        );
                        break;
                    }
                    entry.1 += 1;
                    decoder_transcript_digest.update(serde_json::to_vec(
                        &json!({"name":call.name,"inputs":call.inputs,"outputs":call.outputs}),
                    )?);
                    decoder_transcript_digest.update(b"\n");
                }
                *histogram.entry(row).or_default() += 1;
                completed += 1;
            }
            Err(error) => {
                failure = Some(json!({"index":index,"error":error.to_string()}));
                break;
            }
        }
    }
    let sampling_seconds = sampling.elapsed().as_secs_f64();
    let outer_seconds = started.elapsed().as_secs_f64();
    let costs = meter.snapshot();
    let mut histogram = histogram.into_iter().map(|(row, count)| json!({
        "occurrences":row.occurrences,"acceptance":row.acceptance.coefficients().each_ref().map(ToString::to_string),
        "error":row.error.coefficients().each_ref().map(ToString::to_string),"count":count
    })).collect::<Vec<_>>();
    histogram.sort_by_cached_key(Value::to_string);
    let decoder_histogram = decoder_histogram
        .into_iter()
        .map(|(inputs, (outputs, count))| json!({"inputs":inputs,"outputs":outputs,"count":count}))
        .collect::<Vec<_>>();
    Ok(json!({
        "schema":"rmagna.paper-code-switch-run.v1",
        "status":if failure.is_none() {"complete"} else {"failed"},"failure":failure,
        "configuration":config,"completed":completed,"original_sites":sites.len(),"excluded_ideal_sites":ideal_sites,
        "histogram":histogram,"output_contract":"unit_noise_history_normalized_masses_with_external_occurrence_ratio",
        "rng_protocol":addressed_rng::PROTOCOL,"execution_route":evaluator.execution_route(),
        "excluded_warmup":{"draws":1,"index":u32::MAX},
        "build":{"debug_assertions":cfg!(debug_assertions),"os":std::env::consts::OS,"architecture":std::env::consts::ARCH},
        "decoder":process.as_ref().map(ControllerProcess::manifest),"decoder_histogram":decoder_histogram,
        "decoder_transcript_sha256":(expected_calls != 0).then(|| format!("{:x}",decoder_transcript_digest.finalize())),
        "timings":{"outer_seconds":outer_seconds,"preparation_seconds":prepare_seconds,"compile_seconds":compile_seconds,
            "sampling_seconds":sampling_seconds,"decoder_seconds":costs.decoder.seconds,
            "decoder_attempted":costs.decoder.attempted,"decoder_completed":costs.decoder.succeeded},
        "rust_source_sha256":env!("RMAGNA_PAPER_SOURCE_SHA256"),"executable_sha256":digest(&std::fs::read(std::env::current_exe()?)?),
        "inputs":{"physical_sha256":digest(physical_text.as_bytes()),"task_sha256":digest(task_text.as_bytes()),
            "ideal_sha256":ideal_sha256,"recovery_sha256":recovery_sha256,"controllers_sha256":controller_sha256}
    }))
}

fn main() {
    let result = run(std::env::args().skip(1)).unwrap_or_else(|error| {
        json!({
        "schema":"rmagna.paper-code-switch-run.v1","status":"failed","failure":error.to_string()})
    });
    println!("{result}");
    if result["status"] != "complete" {
        std::process::exit(1);
    }
}
