//! Evaluate one prescribed no-fault task draw without external controller bindings.
use rmagna::circuit::{ParseOptions, parse_circuit};
use rmagna::field::ExactField;
use rmagna::noise::SplitMix64;
use rmagna::runtime::ControllerRegistry;
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
    let seed = args
        .next()
        .map(|s| s.parse::<u64>())
        .transpose()?
        .unwrap_or(0);
    if args.next().is_some() {
        return Err("too many arguments".into());
    }
    let parse = ParseOptions {
        source: Some(physical_path.clone()),
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
    let evaluator = TaskEvaluator::new(
        &physical,
        &modules,
        &ControllerRegistry::default(),
        EvaluationOptions::default(),
    )?;
    let compile_ms = start.elapsed().as_secs_f64() * 1000.;
    let query = std::time::Instant::now();
    let draw = evaluator.sample_prescribed(&[], &mut SplitMix64::new(seed))?;
    println!(
        "{}",
        serde_json::json!({"resources":{"acceptance":{"quotient_rank":draw.acceptance_resources.quotient_rank,"terms":draw.acceptance_resources.terms},"target":draw.target_resources.map(|r|serde_json::json!({"quotient_rank":r.quotient_rank,"terms":r.terms}))},"accepted":field(&draw.accepted),"error":field(&draw.error),"target_success":draw.target_success.as_ref().map(field),"moments":draw.moments.iter().map(field).collect::<Vec<_>>(),"history_mass":scalar(&draw.history_mass),"raw_acceptance":scalar(&draw.raw_acceptance),"raw_error":scalar(&draw.raw_error),"raw_target_success":draw.raw_target_success.as_ref().map(scalar),"target_normalization":draw.target_normalization.as_ref().map(scalar),"sampled_records":draw.sampled_records,"compile_ms":compile_ms,"query_ms":query.elapsed().as_secs_f64()*1000.})
    );
    Ok(())
}
