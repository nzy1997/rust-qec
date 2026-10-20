//! Inspect exact no-fault acceptance and pure-target mass of an affine circuit.
use rmagna::circuit::{ParseOptions, parse_circuit};
use rmagna::instrument::{CompileOptions, compile_affine, compile_unitary};
use rmagna::scalar::{ExactScalar, ScalarArithmetic};

fn scalar(value: &ExactScalar) -> serde_json::Value {
    serde_json::json!({"coefficients":value.coefficients().each_ref().map(|c|c.to_string()),"power":value.power()})
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let physical = args
        .next()
        .ok_or("usage: affine_mass PHYSICAL TARGET OUTPUT_CSV [MAX_TERMS]")?;
    let target_path = args.next().ok_or("missing target circuit path")?;
    let outputs = args
        .next()
        .ok_or("missing comma-separated physical output indices")?;
    let outputs = if outputs.is_empty() {
        Vec::new()
    } else {
        outputs
            .split(',')
            .map(str::parse::<usize>)
            .collect::<Result<Vec<_>, _>>()?
    };
    let max_terms = args.next().map(|s| s.parse()).transpose()?.unwrap_or(256);
    if args.next().is_some() {
        return Err("too many arguments".into());
    }
    let physical_circuit = parse_circuit(
        &std::fs::read_to_string(&physical)?,
        &ParseOptions {
            source: Some(physical),
            ..Default::default()
        },
    )?;
    let target_circuit = parse_circuit(
        &std::fs::read_to_string(&target_path)?,
        &ParseOptions {
            source: Some(target_path),
            ..Default::default()
        },
    )?;
    let arithmetic = ScalarArithmetic::default();
    let start = std::time::Instant::now();
    let target = compile_unitary(&target_circuit, &CompileOptions::default(), &arithmetic)?;
    let protocol = compile_affine(&physical_circuit, &CompileOptions::default(), &arithmetic)?;
    let compile_ms = start.elapsed().as_secs_f64() * 1000.;
    let query = std::time::Instant::now();
    let result = protocol.masses(&outputs, &target, &[], max_terms, &arithmetic)?;
    println!(
        "{}",
        serde_json::json!({"accepted":scalar(&result.accepted.value),"target_success":scalar(&result.target_success.value),"error":scalar(&result.error),"acceptance_rank":result.accepted.quotient_rank,"acceptance_terms":result.accepted.terms,"fidelity_rank":result.target_success.quotient_rank,"fidelity_terms":result.target_success.terms,"physical_qubits":protocol.num_qubits(),"coherent_qubits":protocol.coherent_qubits(),"records":protocol.num_records(),"noise_sites":protocol.noise_sites().len(),"compile_ms":compile_ms,"query_ms":query.elapsed().as_secs_f64()*1000.})
    );
    Ok(())
}
