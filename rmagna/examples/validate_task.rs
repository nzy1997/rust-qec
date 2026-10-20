//! Validate task schema and circuit modules without executing controllers.
use rmagna::circuit::{ParseOptions, parse_circuit};
use rmagna::task::{TaskOptions, parse_task};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let physical = args
        .next()
        .ok_or("usage: validate_task PHYSICAL TASK [CONTROLLER...]")?;
    let task = args.next().ok_or("missing task JSON path")?;
    let options = ParseOptions {
        source: Some(physical.clone()),
        controllers: args.collect(),
        ..Default::default()
    };
    let physical = parse_circuit(&std::fs::read_to_string(physical)?, &options)?;
    let task_path = std::path::Path::new(&task);
    let spec = parse_task(
        &std::fs::read_to_string(task_path)?,
        task_path.parent().unwrap_or(std::path::Path::new(".")),
        &TaskOptions::default(),
    )?;
    let loaded = spec.load_modules(&physical, &options)?;
    let target=loaded.ideal_state().map(|target|serde_json::json!({"qubits":target.circuit().num_qubits(),"outputs":target.outputs(),"postselection":target.postselection()}));
    let recovery=loaded.recovery().map(|circuit|serde_json::json!({"qubits":circuit.num_qubits(),"records":circuit.num_records(),"history":circuit.needs_history_sampling()}));
    println!(
        "{}",
        serde_json::json!({"schema_version":1,"outputs":loaded.task().outputs(),"sample_records":loaded.task().sample_records(),"error_observables":loaded.task().error_observables(),"moments":loaded.task().observables().len(),"target":target,"recovery":recovery})
    );
    Ok(())
}
