// SPDX-License-Identifier: MPL-2.0
// Sparse FileApi protocol derived from itpyi/magna@56847478c840395735146ef1e1ccdc3d8aedc10f.
//! Sparse-index resource diagnostic; timings are not paper performance evidence.
use num_rational::BigRational;
use rmagna::circuit::{ParseOptions, parse_circuit};
use rmagna::field::FieldArithmetic;
use rmagna::runtime::ControllerRegistry;
use rmagna::task::{
    EvaluationOptions, TaskEnumerationOptions, TaskEvaluator, TaskOptions, parse_task,
};
use rmagna::unitary::CompilerStrategy;
use serde_json::json;
use std::path::PathBuf;
use std::time::Instant;

struct Files(PathBuf);
impl Drop for Files {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let budget: usize = std::env::args()
        .nth(1)
        .map(|s| s.parse())
        .transpose()?
        .unwrap_or(16);
    let root = std::env::temp_dir().join(format!("rmagna-sparse-width-{}", std::process::id()));
    std::fs::create_dir(&root)?;
    let files = Files(root);
    std::fs::write(files.0.join("target.magna"), "H 0\nT 0\n")?;
    let mut rows = vec![];
    for compiler in [
        CompilerStrategy::ParityCopies,
        CompilerStrategy::SharedCommutingLayer,
    ] {
        for label in [0, 3, 63, 1023, 16383] {
            let measured = if label == 0 { 1 } else { label + 2 };
            let auxiliary = if label == 0 { 1 } else { label + 1 };
            let text = format!(
                "H {label}\nT {label}\nX_ERROR(1/4) {label}\nH {measured}\nM {measured}\nDETECTOR rec[-1]\nPOSTSELECT det[0]\n"
            );
            let recovery = format!("H {auxiliary}\nM {auxiliary}\nCZ rec[-1] 0\n");
            std::fs::write(files.0.join("recovery.magna"), recovery)?;
            let parse = ParseOptions::default();
            let circuit = parse_circuit(&text, &parse)?;
            let modules = parse_task(&json!({"outputs":[label],"ideal_state":"target.magna","recovery":"recovery.magna"}).to_string(), &files.0, &TaskOptions::default())?.load_modules(&circuit, &parse)?;
            let mut options = EvaluationOptions::default();
            options.runtime.compile.compiler_strategy = compiler;
            options.runtime.compile.max_coherent_qubits = budget;
            let started = Instant::now();
            let task =
                TaskEvaluator::new(&circuit, &modules, &ControllerRegistry::default(), options);
            let compile_ns = started.elapsed().as_nanos().to_string();
            let mut row = json!({"compiler":format!("{compiler:?}"),"source_output":label,"physical_width":circuit.num_qubits(),"composed_ambient_width":circuit.num_qubits()+auxiliary,"max_coherent_qubits":budget,"compile_ns":compile_ns});
            match task {
                Err(error) => {
                    row["status"] = json!("rejected");
                    row["error"] = json!(error.to_string());
                }
                Ok(task) => {
                    let diagnostics = task.compilation();
                    row["coherent_width"] = json!(diagnostics.coherent_width);
                    row["phase_ancillas"] = json!(diagnostics.phase_ancillas);
                    row["shared_phase_coordinates"] = json!(diagnostics.shared_phase_coordinates);
                    row["reference_coherent_width"] = json!(diagnostics.reference_coherent_width);
                    row["raw_path_variables"] = json!(diagnostics.raw_path_variables);
                    row["clifford_gates"] = json!(diagnostics.clifford_gates);
                    let mut limits = TaskEnumerationOptions::default();
                    limits.noise.max_weight = 1;
                    let started = Instant::now();
                    let result = task.enumerate_physical(&limits)?;
                    row["enumerate_ns"] = json!(started.elapsed().as_nanos().to_string());
                    let f = FieldArithmetic::default();
                    let exact = |n: i64, d: i64| {
                        f.from_rational(BigRational::new(n.into(), d.into()))
                            .unwrap()
                    };
                    assert_eq!(result.masses.accepted, exact(1, 2));
                    assert_eq!(result.masses.error, exact(1, 4));
                    assert_eq!(result.masses.target_success, Some(exact(1, 4)));
                    row["status"] = json!("complete");
                    row["accepted"] = json!("1/2");
                    row["error_mass"] = json!("1/4");
                    row["target_success"] = json!("1/4");
                }
            }
            rows.push(row);
        }
    }
    println!(
        "{}",
        serde_json::to_string_pretty(
            &json!({"schema":"rstim.sparse-task-width.v1","scope":"Sparse-index width and resource admission; single-run timings, no peak-memory or paper-performance claim.","rows":rows})
        )?
    );
    Ok(())
}
