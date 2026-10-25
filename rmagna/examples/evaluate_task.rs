//! Evaluate one prescribed no-fault task draw with optional explicit decoder bindings.
use rmagna::circuit::{ParseOptions, parse_circuit};
use rmagna::field::ExactField;
use rmagna::noise::{RandomSource, SplitMix64};
use std::io::Write;
struct Entropy {
    random: SplitMix64,
    fixed: Option<u64>,
}
impl RandomSource for Entropy {
    fn next_u64(&mut self) -> u64 {
        self.fixed.unwrap_or_else(|| self.random.next_u64())
    }
}
use rmagna::runtime::ControllerRegistry;
use rmagna::runtime::process::{ControllerProcess, ProcessOptions};
use rmagna::scalar::ExactScalar;
use rmagna::task::{
    EvaluationOptions, TaskError, TaskEvaluation, TaskEvaluator, TaskOptions, parse_task,
};
fn scalar(value: &ExactScalar) -> serde_json::Value {
    serde_json::json!({"coefficients":value.coefficients().each_ref().map(|c|c.to_string()),"power":value.power()})
}
fn field(value: &ExactField) -> serde_json::Value {
    serde_json::json!(value.coefficients().each_ref().map(|c| c.to_string()))
}
fn evaluate_once(
    evaluator: &TaskEvaluator,
    history: Option<&[Vec<bool>]>,
    seed: u64,
    fixed: Option<u64>,
) -> Result<TaskEvaluation, TaskError> {
    if let Some(groups) = history {
        evaluator.evaluate_prescribed_history(&[], groups)
    } else {
        evaluator.sample_prescribed(
            &[],
            &mut Entropy {
                random: SplitMix64::new(seed),
                fixed,
            },
        )
    }
}
/// Execute the diagnostic with the same path used by the command-line entry.
/// An injected argument stream and writer let integration tests exercise it.
pub fn run(
    mut args: impl Iterator<Item = String>,
    output: &mut impl Write,
) -> Result<(), Box<dyn std::error::Error>> {
    let physical_path = args
        .next()
        .ok_or("usage: evaluate_task PHYSICAL TASK [SEED]")?;
    let task_path = args.next().ok_or("missing task JSON path")?;
    let mut seed = 0;
    let mut warm_replays = 0_usize;
    let mut history = None;
    let mut options = EvaluationOptions::default();
    let mut fixed = None;
    let mut probability_mode = "cold".to_owned();
    let mut density_mode = "cold".to_owned();
    let mut insertion_mode = "direct".to_owned();
    let mut insertion_bytes = None;
    let mut insertion_entries = None;
    let mut fiber_options = rmagna::fibers::FiberOptions::default();
    let mut controllers = None;
    let mut python = "python3".to_owned();
    let mut module_root = None;
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--insertion-preparation" => {
                insertion_mode = args.next().ok_or("missing insertion preparation mode")?
            }
            "--max-insertion-table-bytes" => {
                insertion_bytes = Some(
                    args.next()
                        .ok_or("missing insertion table byte limit")?
                        .parse::<usize>()?,
                )
            }
            "--max-insertion-catalog-entries" => {
                insertion_entries = Some(
                    args.next()
                        .ok_or("missing insertion catalog limit")?
                        .parse::<usize>()?,
                )
            }
            "--warm-replays" => {
                warm_replays = args.next().ok_or("missing warm replay count")?.parse()?;
            }
            "--density-preparation" => {
                density_mode = args.next().ok_or("missing density preparation mode")?;
            }
            "--max-prepared-densities" => {
                options.runtime.compile.max_prepared_densities =
                    args.next().ok_or("missing density query limit")?.parse()?;
            }
            "--probability-preparation" => {
                probability_mode = args.next().ok_or("missing probability preparation mode")?;
            }
            "--fiber-search-rank" => {
                fiber_options.max_search_rank =
                    args.next().ok_or("missing fiber search rank")?.parse()?;
            }
            "--max-prepared-probabilities" => {
                options.runtime.compile.max_prepared_probabilities =
                    args.next().ok_or("missing prepared query limit")?.parse()?;
            }
            "--compiler" => {
                options.runtime.compile.compiler_strategy =
                    match args.next().ok_or("missing compiler strategy")?.as_str() {
                        "shared" => rmagna::unitary::CompilerStrategy::SharedCommutingLayer,
                        "parity_copies" => rmagna::unitary::CompilerStrategy::ParityCopies,
                        _ => return Err("compiler must be shared or parity_copies".into()),
                    };
            }
            "--history-sampler" => {
                options.runtime.history_sampler =
                    match args.next().ok_or("missing history sampler")?.as_str() {
                        "exact_marginal" => rmagna::runtime::HistorySampler::ExactMarginal,
                        "affine_support" => rmagna::runtime::HistorySampler::AffineSupport,
                        _ => {
                            return Err(
                                "history sampler must be exact_marginal or affine_support".into()
                            );
                        }
                    };
            }
            "--causal-strategy" => {
                options.runtime.strategy =
                    match args.next().ok_or("missing causal strategy")?.as_str() {
                        "fixed" => rmagna::runtime::CausalStrategy::FixedTemplate,
                        "cold" => rmagna::runtime::CausalStrategy::ColdPrefix,
                        _ => return Err("causal strategy must be fixed or cold".into()),
                    }
            }
            "--history" => history = Some(args.next().ok_or("missing history JSON")?),
            "--max-terms" => {
                options.runtime.sampling.max_terms =
                    args.next().ok_or("missing term budget")?.parse::<usize>()?
            }
            "--constant-word" => {
                fixed = Some(
                    args.next()
                        .ok_or("missing constant entropy word")?
                        .parse::<u64>()?,
                )
            }
            "--controllers" => controllers = Some(args.next().ok_or("missing controller file")?),
            "--python" => python = args.next().ok_or("missing Python executable")?,
            "--module-root" => {
                module_root = Some(args.next().ok_or("missing decoder module root")?)
            }
            _ => seed = argument.parse::<u64>()?,
        }
    }
    if warm_replays > 16 {
        return Err("warm replay count must be between zero and 16".into());
    }
    let mut insertion_options = rmagna::unitary::InsertionPreparationOptions::default();
    if let Some(bytes) = insertion_bytes {
        insertion_options.max_table_bytes = bytes;
    }
    if let Some(entries) = insertion_entries {
        insertion_options.max_catalog_entries = entries;
    }
    options.runtime.compile.insertion_preparation = match insertion_mode.as_str() {
        "direct" => {
            if insertion_bytes.is_some() || insertion_entries.is_some() {
                return Err("insertion limits require prepared insertion mode".into());
            }
            rmagna::instrument::InsertionPreparation::Direct
        }
        "prepared" => rmagna::instrument::InsertionPreparation::Prepared(insertion_options),
        _ => return Err("insertion preparation must be direct or prepared".into()),
    };
    options.runtime.compile.probability_preparation = match probability_mode.as_str() {
        "cold" => rmagna::instrument::ProbabilityPreparation::Cold,
        "trace" => rmagna::instrument::ProbabilityPreparation::Trace,
        "fixed" => rmagna::instrument::ProbabilityPreparation::FixedFibers(fiber_options),
        "catalogue" => rmagna::instrument::ProbabilityPreparation::FixedCatalogue(
            rmagna::phase::PhaseResourceOptions {
                fibers: fiber_options,
                ..Default::default()
            },
        ),
        _ => return Err("probability preparation must be cold, trace, fixed or catalogue".into()),
    };
    options.runtime.compile.density_preparation = match density_mode.as_str() {
        "cold" => rmagna::instrument::QueryPreparation::Cold,
        "trace" => rmagna::instrument::QueryPreparation::Trace,
        "fixed" => rmagna::instrument::QueryPreparation::FixedFibers(fiber_options),
        "catalogue" => rmagna::instrument::QueryPreparation::FixedCatalogue(
            rmagna::phase::PhaseResourceOptions {
                fibers: fiber_options,
                ..Default::default()
            },
        ),
        _ => return Err("density preparation must be cold, trace, fixed or catalogue".into()),
    };
    if !matches!(probability_mode.as_str(), "fixed" | "catalogue")
        && !matches!(density_mode.as_str(), "fixed" | "catalogue")
        && fiber_options.max_search_rank != rmagna::fibers::FiberOptions::default().max_search_rank
    {
        return Err(
            "fiber search rank requires fixed or catalogue probability or density preparation"
                .into(),
        );
    }
    let mut registry = ControllerRegistry::default();
    let worker = if let Some(config) = controllers {
        let root = module_root
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                std::path::Path::new(&config)
                    .parent()
                    .unwrap_or(std::path::Path::new("."))
                    .to_path_buf()
            });
        let mut command = std::process::Command::new(python);
        command.args(["-I", "-B", "-u"]);
        command.arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/python/decoder_worker.py"
        ));
        command
            .arg("--controllers")
            .arg(config)
            .arg("--module-root")
            .arg(root);
        let worker = ControllerProcess::spawn(command, ProcessOptions::default())?;
        worker.register_all(&mut registry)?;
        Some(worker)
    } else {
        None
    };
    let parse = ParseOptions {
        source: Some(physical_path.clone()),
        controllers: registry.names(),
        ..Default::default()
    };
    let physical = parse_circuit(&std::fs::read_to_string(physical_path)?, &parse)?;
    let path = std::path::Path::new(&task_path);
    let spec = parse_task(
        &std::fs::read_to_string(path)?,
        path.parent().unwrap_or(std::path::Path::new(".")),
        &TaskOptions::default(),
    )?;
    let modules = spec.load_modules(&physical, &parse)?;
    let start = std::time::Instant::now();
    let evaluator = TaskEvaluator::new(&physical, &modules, &registry, options)?;
    let compile_ms = start.elapsed().as_secs_f64() * 1000.;
    let history: Option<Vec<Vec<bool>>> = history
        .map(|path| -> Result<_, Box<dyn std::error::Error>> {
            Ok(serde_json::from_str(&std::fs::read_to_string(path)?)?)
        })
        .transpose()?;
    let query = std::time::Instant::now();
    let draw = evaluate_once(&evaluator, history.as_deref(), seed, fixed)?;
    let query_ms = query.elapsed().as_secs_f64() * 1000.;
    let first_cache = (
        evaluator.prepared_probability_statistics()?,
        evaluator.prepared_density_statistics()?,
        evaluator.prepared_insertion_statistics(),
    );
    let mut replay_ms = Vec::new();
    for _ in 0..warm_replays {
        let start = std::time::Instant::now();
        let repeated = evaluate_once(&evaluator, history.as_deref(), seed, fixed)?;
        let elapsed = start.elapsed().as_secs_f64() * 1000.;
        if repeated != draw {
            return Err(
                "warm replay changed an exact evaluation field or controller transcript".into(),
            );
        }
        if (
            evaluator.prepared_probability_statistics()?,
            evaluator.prepared_density_statistics()?,
            evaluator.prepared_insertion_statistics(),
        ) != first_cache
        {
            return Err("warm replay changed retained probability or density cache counts".into());
        }
        replay_ms.push(elapsed);
    }
    let warm = (warm_replays > 0).then(|| serde_json::json!({"count":warm_replays,"query_ms":replay_ms,
        "cache_unchanged":true,"scope":"same-assignment replay with reset entropy/history; not independent Monte Carlo or formal performance rounds"}));
    let probability_cache = first_cache.0.map(|s|
        serde_json::json!({"cached_queries":s.cached_queries,"fixed_resources":s.fixed_resources,"catalogue_resources":s.catalogue_resources}));
    let density_cache = first_cache.1.map(|s|
        serde_json::json!({"cached_queries":s.cached_queries,"fixed_resources":s.fixed_resources,"catalogue_resources":s.catalogue_resources}));
    let insertion_catalog=first_cache.2.map(|s|serde_json::json!({"catalog_entries":s.catalog_entries,"estimated_peak_bytes":s.estimated_peak_bytes}));
    let compilation_scope = if evaluator.sampled_history() {
        match options.runtime.strategy {
            rmagna::runtime::CausalStrategy::FixedTemplate => "initial_fixed_template",
            rmagna::runtime::CausalStrategy::ColdPrefix => "initial_potential",
        }
    } else {
        "linear_instrument"
    };
    let d = evaluator.compilation();
    let compilation = serde_json::json!({"scope":compilation_scope,"strategy":format!("{:?}",d.strategy),"layer_slots":d.layer_slots,
        "reference_layer":d.reference_layer,"reference_layer_slots":d.reference_layer_slots,
        "phase_ancillas":d.phase_ancillas,"shared_phase_coordinates":d.shared_phase_coordinates,
        "coherent_width":d.coherent_width,"reference_coherent_width":d.reference_coherent_width,"shared_observations_before_reduction":d.shared_observations_before_reduction,"clifford_gates":d.clifford_gates,"path_variables":d.path_variables});
    writeln!(
        output,
        "{}",
        serde_json::json!({"prepared_insertions":insertion_catalog,"compilation":compilation,"prepared_density_cache":density_cache,"warm_replays":warm,"prepared_probability_cache":probability_cache,"configuration":{"insertion_preparation":insertion_mode,"max_insertion_table_bytes":if insertion_mode=="prepared"{Some(insertion_options.max_table_bytes)}else{None},"max_insertion_catalog_entries":if insertion_mode=="prepared"{Some(insertion_options.max_catalog_entries)}else{None},"density_preparation":options.runtime.compile.density_preparation.name(),"max_prepared_densities":options.runtime.compile.max_prepared_densities,"seed":seed,"constant_word":fixed,"warm_replays":warm_replays,"probability_preparation":options.runtime.compile.probability_preparation.name(),"fiber_search_rank":if options.runtime.compile.probability_preparation.is_fixed() || options.runtime.compile.density_preparation.is_fixed(){Some(fiber_options.max_search_rank)}else{None},"max_prepared_probabilities":options.runtime.compile.max_prepared_probabilities,"max_terms":options.runtime.sampling.max_terms,"causal_strategy":format!("{:?}",options.runtime.strategy),"history_sampler":format!("{:?}",options.runtime.history_sampler)},"network_compilations":draw.network_compilations,"history_queries":draw.history_queries,"decoder_manifest":worker.as_ref().map(ControllerProcess::manifest),"controller_calls":draw.controller_calls.iter().map(|call|serde_json::json!({"name":call.name,"inputs":call.inputs,"outputs":call.outputs})).collect::<Vec<_>>(),"resources":{"acceptance":{"quotient_rank":draw.acceptance_resources.quotient_rank,"terms":draw.acceptance_resources.terms},"target":draw.target_resources.map(|r|serde_json::json!({"quotient_rank":r.quotient_rank,"terms":r.terms}))},"accepted":field(&draw.accepted),"error":field(&draw.error),"target_success":draw.target_success.as_ref().map(field),"moments":draw.moments.iter().map(field).collect::<Vec<_>>(),"history_mass":scalar(&draw.history_mass),"history_proposal_mass":draw.history_proposal_mass.as_ref().map(scalar),"history_weight":field(&draw.history_weight),"raw_acceptance":scalar(&draw.raw_acceptance),"raw_error":scalar(&draw.raw_error),"raw_target_success":draw.raw_target_success.as_ref().map(scalar),"target_normalization":draw.target_normalization.as_ref().map(scalar),"sampled_records":draw.sampled_records,"compile_ms":compile_ms,"query_ms":query_ms})
    )?;
    Ok(())
}

#[cfg(not(test))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    run(std::env::args().skip(1), &mut std::io::stdout().lock())
}
