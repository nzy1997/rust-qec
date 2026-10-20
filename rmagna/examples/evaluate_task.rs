//! Evaluate one prescribed no-fault task draw with optional explicit decoder bindings.
use rmagna::circuit::{ParseOptions, parse_circuit};
use rmagna::field::ExactField;
use rmagna::noise::{RandomSource, SplitMix64};
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
use rmagna::task::{EvaluationOptions, TaskEvaluator, TaskOptions, parse_task};
fn scalar(value: &ExactScalar) -> serde_json::Value {
    serde_json::json!({"coefficients":value.coefficients().each_ref().map(|c|c.to_string()),"power":value.power()})
}
fn field(value: &ExactField) -> serde_json::Value {
    serde_json::json!(value.coefficients().each_ref().map(|c| c.to_string()))
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let physical_path = args
        .next()
        .ok_or("usage: evaluate_task PHYSICAL TASK [SEED]")?;
    let task_path = args.next().ok_or("missing task JSON path")?;
    let mut seed = 0;
    let mut history = None;
    let mut options = EvaluationOptions::default();
    let mut fixed = None;
    let mut controllers = None;
    let mut python = "python3".to_owned();
    let mut module_root = None;
    while let Some(argument) = args.next() {
        match argument.as_str() {
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
    let query = std::time::Instant::now();
    let draw = if let Some(path) = history {
        let groups: Vec<Vec<bool>> = serde_json::from_str(&std::fs::read_to_string(path)?)?;
        evaluator.evaluate_prescribed_history(&[], &groups)?
    } else {
        evaluator.sample_prescribed(
            &[],
            &mut Entropy {
                random: SplitMix64::new(seed),
                fixed,
            },
        )?
    };
    println!(
        "{}",
        serde_json::json!({"configuration":{"max_terms":options.runtime.sampling.max_terms,"causal_strategy":format!("{:?}",options.runtime.strategy),"history_sampler":format!("{:?}",options.runtime.history_sampler)},"network_compilations":draw.network_compilations,"history_queries":draw.history_queries,"decoder_manifest":worker.as_ref().map(ControllerProcess::manifest),"controller_calls":draw.controller_calls.iter().map(|call|serde_json::json!({"name":call.name,"inputs":call.inputs,"outputs":call.outputs})).collect::<Vec<_>>(),"resources":{"acceptance":{"quotient_rank":draw.acceptance_resources.quotient_rank,"terms":draw.acceptance_resources.terms},"target":draw.target_resources.map(|r|serde_json::json!({"quotient_rank":r.quotient_rank,"terms":r.terms}))},"accepted":field(&draw.accepted),"error":field(&draw.error),"target_success":draw.target_success.as_ref().map(field),"moments":draw.moments.iter().map(field).collect::<Vec<_>>(),"history_mass":scalar(&draw.history_mass),"history_proposal_mass":draw.history_proposal_mass.as_ref().map(scalar),"history_weight":field(&draw.history_weight),"raw_acceptance":scalar(&draw.raw_acceptance),"raw_error":scalar(&draw.raw_error),"raw_target_success":draw.raw_target_success.as_ref().map(scalar),"target_normalization":draw.target_normalization.as_ref().map(scalar),"sampled_records":draw.sampled_records,"compile_ms":compile_ms,"query_ms":query.elapsed().as_secs_f64()*1000.})
    );
    Ok(())
}
