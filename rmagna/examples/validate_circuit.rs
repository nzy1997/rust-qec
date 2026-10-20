//! Inspect parsed circuit counts without running quantum simulation or decoders.
use rmagna::circuit::{ParseOptions, parse_circuit};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = std::env::args().skip(1);
    let path = arguments
        .next()
        .ok_or("usage: validate_circuit FILE [CONTROLLER ...]")?;
    let options = ParseOptions {
        source: Some(path.clone()),
        controllers: arguments.collect(),
        ..Default::default()
    };
    let source = std::fs::read_to_string(&path)?;
    let circuit = parse_circuit(&source, &options)?;
    println!(
        "{}",
        serde_json::json!({"format":circuit.format_version(),"qubits":circuit.num_qubits(),"records":circuit.num_records(),"instructions":circuit.num_instructions(),"operations":circuit.operations().len(),"detectors":circuit.detectors().len(),"external_calls":circuit.external_calls(),"history":circuit.needs_history_sampling()})
    );
    Ok(())
}
