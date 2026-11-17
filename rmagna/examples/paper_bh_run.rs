//! Real BH draws with separate preparation/warmup and exact stream totals.
use rmagna::circuit::{ParseOptions, parse_circuit};
use rmagna::costs::{ExecutionLimits, ExecutionMeter};
use rmagna::instrument::QueryPreparation;
use rmagna::noise::NoiseSampling;
use rmagna::phase::{NativePhaseOptions, NativeTransportMode};
use rmagna::runtime::{ControllerRegistry, HistorySampler};
use rmagna::statistics::CheckpointOptions;
use rmagna::task::{
    EvaluationOptions, FingerprintInputs, TaskEvaluator, TaskOptions, TaskRunOptions,
    TaskStreamIdentity, TaskStreamOptions, export_batch_metrics, export_stream_metrics,
    fingerprint_content, parse_task,
};
use rmagna::unitary::CompilerStrategy;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, error::Error, path::Path, time::Instant};

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Configuration {
    schema: String,
    outputs: usize,
    shots: u64,
    seed: u64,
    proposal_scale: String,
    max_signature_work: usize,
}

fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn inventory(evaluator: &TaskEvaluator) -> Result<Value, Box<dyn Error>> {
    Ok(json!({
        "work": evaluator.query_work_statistics(),
        "storage": evaluator.reference_storage_statistics()?,
        "source_assignments": evaluator.source_insertion_assignments(),
    }))
}

fn resources(evaluator: &TaskEvaluator) -> Result<Value, Box<dyn Error>> {
    let family = |value: Option<rmagna::instrument::PreparedQueryStatistics>| {
        value.map(|s| {
            json!({"queries":s.cached_queries,"resources":s.fixed_resources,
            "maximum_quotient_rank":s.maximum_quotient_rank,"maximum_terms":s.maximum_terms})
        })
    };
    Ok(
        json!({"acceptance":family(evaluator.prepared_probability_statistics()?),
        "target":family(evaluator.prepared_density_statistics()?)}),
    )
}

pub(crate) fn run(mut args: impl Iterator<Item = String>) -> Result<Value, Box<dyn Error>> {
    let physical_path = args
        .next()
        .ok_or("usage: paper_bh_run PHYSICAL TASK CONFIGURATION")?;
    let task_path = args.next().ok_or("missing task JSON path")?;
    let configuration_path = args.next().ok_or("missing configuration JSON path")?;
    if args.next().is_some() {
        return Err("unexpected argument after configuration path".into());
    }
    let configuration: Configuration =
        serde_json::from_str(&std::fs::read_to_string(configuration_path)?)?;
    if configuration.schema != "rmagna.paper-bh-configuration.v1"
        || configuration.shots < 2
        || configuration.outputs < 2
        || !configuration.outputs.is_multiple_of(2)
    {
        return Err(
            "BH configuration requires its v1 schema, positive even outputs and at least two draws"
                .into(),
        );
    }
    let registry = ControllerRegistry::default();
    let parse = ParseOptions {
        source: Some(physical_path.clone()),
        ..Default::default()
    };
    let physical_text = std::fs::read_to_string(&physical_path)?;
    let physical = parse_circuit(&physical_text, &parse)?;
    let task_text = std::fs::read_to_string(&task_path)?;
    let task_path = Path::new(&task_path);
    let spec = parse_task(
        &task_text,
        task_path.parent().unwrap_or(Path::new(".")),
        &TaskOptions::default(),
    )?;
    let modules = spec.load_modules(&physical, &parse)?;
    if physical.needs_history_sampling()
        || physical.external_calls() != 0
        || modules.recovery().is_some()
        || modules.task().outputs().len() != configuration.outputs
        || modules
            .ideal_state()
            .is_none_or(|target| target.outputs().len() != configuration.outputs)
    {
        return Err("BH runner requires an affine protocol, complete joint target and no recovery/external callbacks".into());
    }
    let sampling = NoiseSampling::Importance {
        proposal_scale: configuration.proposal_scale.parse()?,
    };
    let mut native = NativePhaseOptions::default();
    native.selection.max_quotient_rank = 0;
    native.selection.max_signature_work = configuration.max_signature_work as u128;
    native.selection.preparation.max_terms = 1;
    native.transport.mode = NativeTransportMode::Prepared;
    let mut options = EvaluationOptions::default();
    options.runtime.history_sampler = HistorySampler::AffineSupport;
    options.runtime.sampling.max_terms = 1;
    options.runtime.compile.compiler_strategy = CompilerStrategy::SharedCommutingLayer;
    options.runtime.compile.max_path_bits = 65_536;
    options.runtime.compile.probability_preparation = QueryPreparation::Native(native);
    options.runtime.compile.density_preparation = QueryPreparation::Native(native);
    options.runtime.compile.moment_preparation = QueryPreparation::Native(native);
    let run_options = TaskRunOptions {
        shots: configuration.shots,
        seed: configuration.seed,
        ..Default::default()
    };
    let identities = BTreeMap::new();
    let content = fingerprint_content(
        &FingerprintInputs {
            physical: &physical,
            modules: &modules,
            controllers: &registry,
            controller_identities: &identities,
            evaluation: options,
            run: run_options,
            sampling: &sampling,
            numeric: Default::default(),
        },
        Default::default(),
    )?;
    let executable_sha256 = sha(&std::fs::read(std::env::current_exe()?)?);
    let identity = TaskStreamIdentity {
        content,
        implementation: executable_sha256.clone(),
    };
    let preparation_started = Instant::now();
    let meter = ExecutionMeter::new(ExecutionLimits::default());
    let evaluator =
        TaskEvaluator::new_with_costs(&physical, &modules, &registry, options, &sampling, &meter)?;
    let compile_seconds = preparation_started.elapsed().as_secs_f64();
    let prepared = evaluator.prepare_sampling_queries(Default::default(), &meter);
    prepared.result.clone()?;
    if !prepared.all_families_retained() {
        return Err("BH preparation did not retain all configured families".into());
    }
    let preparation_seconds = preparation_started.elapsed().as_secs_f64();
    let after_preparation = inventory(&evaluator)?;
    // One independent seed-zero warmup, excluded from all production statistics,
    // quotas and timers. The production stream starts freshly at its own seed.
    let warmup = evaluator.run_batch(&TaskRunOptions {
        shots: 1,
        seed: 0,
        ..Default::default()
    })?;
    if warmup.completed != 1 || warmup.failed != 0 {
        return Err(format!("BH excluded warmup failed: {:?}", warmup.failures).into());
    }
    let after_warmup = inventory(&evaluator)?;
    let mut stream = evaluator.start_stream(TaskStreamOptions {
        run: run_options,
        ..Default::default()
    })?;
    let report = stream.advance(configuration.shots)?;
    let checkpoint: Value =
        serde_json::from_str(&stream.checkpoint_json(&identity, CheckpointOptions::default())?)?;
    let metrics = export_stream_metrics(&report, Default::default())?;
    let d = evaluator.compilation();
    Ok(json!({
        "schema":"rmagna.paper-bh-run.v1",
        "status":if metrics["status"]=="complete" {"complete"} else {"failed"},
        "output_contract":"integrated_joint_acceptance_error_masses",
        "configuration":configuration,
        "rust_source_sha256":env!("RMAGNA_PAPER_SOURCE_SHA256"),
        "executable_sha256":executable_sha256,
        "build":{"debug_assertions":cfg!(debug_assertions),
            "architecture":std::env::consts::ARCH,"os":std::env::consts::OS},
        "inputs":{"physical_sha256":sha(physical_text.as_bytes()),"task_sha256":sha(task_text.as_bytes()),
            "fingerprints":identity.content.to_json()},
        "widths":{"physical":physical.num_qubits(),"compiled":d.coherent_width,"reference":d.reference_coherent_width},
        "policy":{"max_quotient_rank":0,"max_terms":1,"compiler":"SharedCommutingLayer",
            "native":options.runtime.compile.probability_preparation.configuration()},
        "preparation":{"seconds":preparation_seconds,"compile_seconds":compile_seconds,
            "planned":prepared.counts.planned,"created":prepared.counts.created,"reused":prepared.counts.reused},
        "excluded_warmup":{"draws":1,"seed":0,"metrics":export_batch_metrics(&warmup,Default::default())?},
        "inventory":{"after_preparation":after_preparation,"after_warmup":after_warmup,"after_sampling":inventory(&evaluator)?},
        "resources":resources(&evaluator)?,
        "metrics":metrics,
        "checkpoint":checkpoint,
    }))
}

fn main() {
    let value = run(std::env::args().skip(1)).unwrap_or_else(|error| {
        json!({
            "schema":"rmagna.paper-bh-run.v1","status":"failed","failure":error.to_string()
        })
    });
    println!("{value}");
    if value["status"] != "complete" {
        std::process::exit(1);
    }
}
