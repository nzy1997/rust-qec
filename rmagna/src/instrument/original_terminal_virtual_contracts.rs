//! Complete VirtualCliffordFeedbackTests::test_protocol_and_terminal_virtual_wires_do_not_alias_outputs_or_each_other,
//! itpyi/magna@56847478c840395735146ef1e1ccdc3d8aedc10f/tests/core/test_virtual_clifford_feedback.py.
use super::original_feedback_support::*;
use super::*;
use crate::noise::SplitMix64;
use crate::runtime::{CausalStrategy, ControllerRegistry, HistorySampler};
use crate::task::{
    EvaluationOptions, TaskEvaluator, TaskOptions, TaskSampleDiagnosticOptions, parse_task,
};
use serde_json::Value;
use std::{
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

fn composed(stage: &str) -> String {
    // Recovery labels 0,1 map in order onto physical outputs 2,0. Its echo
    // occupies a fresh decision slot. These are independent stage boundaries.
    let mut text = String::from(
        "H 0 1 2\nT 2\nM 1\nDETECTOR rec[-1]\nCONTROL[echo] det[-1] -> ctrl[0]\nCH ctrl[0] 2\nM 2\nDETECTOR rec[-1]\nCONTROL[echo] det[-1] -> ctrl[1]\nCS ctrl[1] 2\nCH ctrl[1] 2\nCCZ ctrl[1] 2 0\nCCX ctrl[1] 0 2\n",
    );
    if stage == "target" {
        // The target boundary uncomputes and then records both outputs.
        // Unconditional moment queries trace these deferred records rather
        // than conditioning them on zero as a target-overlap query would.
        text.push_str("T_DAG 2\nH 2 0\nM 2 0\n");
    }
    text
}
fn literal(v: bool, w: bool, stage: &str) -> String {
    let mut text = String::from("H 0 1 2\nT 2\nM 1\n");
    if v {
        text.push_str("H 2\n");
    }
    text.push_str("M 2\n");
    if w {
        text.push_str("S 2\nH 2\nCZ 2 0\nCX 0 2\n");
    }
    if stage == "target" {
        text.push_str("T_DAG 2\nH 2 0\nM 2 0\n");
    }
    text
}
fn exact(row: &Value) -> ExactScalar {
    let c = row["coefficients"].as_array().unwrap();
    assert_eq!(c.len(), 4);
    ScalarArithmetic::default()
        .scalar(
            std::array::from_fn(|j| c[j].as_i64().unwrap().into()),
            row["power"].as_i64().unwrap(),
        )
        .unwrap()
}
struct Files(PathBuf);
impl Files {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "rmagna-terminal-virtual-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        std::fs::write(path.join("target.magna"), "H 0 1\nT 0").unwrap();
        std::fs::write(path.join("recovery.magna"),"M 0\nDETECTOR rec[-1]\nCONTROL[echo] det[-1] -> ctrl[0]\nCS ctrl[0] 0\nCH ctrl[0] 0\nCCZ ctrl[0] 0 1\nCCX ctrl[0] 1 0").unwrap();
        Self(path)
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
#[test]
fn original_protocol_and_terminal_virtual_wires_keep_independent_boundaries() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../tests/fixtures/original_terminal_virtual.json"
    ))
    .unwrap();
    assert_eq!(fixture["outputs"], serde_json::json!([2, 0]));
    assert_eq!(
        fixture["provenance"]["upstream_commit"],
        "56847478c840395735146ef1e1ccdc3d8aedc10f"
    );
    let rows = fixture["cases"].as_array().unwrap();
    assert_eq!(rows.len(), 128);
    let a = ScalarArithmetic::default();
    let mut checks = 0;
    let mut irrational = 0;
    let half_root = a.omega(0, -1).unwrap();
    assert!(rows.iter().any(|row| exact(row) == half_root));
    assert!(rows.iter().any(|row| exact(row) == half_root.negated()));
    for config in configurations() {
        for stage in ["recovery", "target"] {
            let compiled = template(&parse(&composed(stage)), config);
            assert_eq!(compiled.decision_wires.len(), 2);
            assert_ne!(
                compiled.decision_wires[&(0, 0)],
                compiled.decision_wires[&(1, 0)]
            );
            for v in [false, true] {
                for w in [false, true] {
                    let view = compiled
                        .branch(
                            &BTreeMap::from([((0, 0), v), ((1, 0), w)]),
                            &BTreeSet::new(),
                        )
                        .unwrap();
                    assert!(Arc::ptr_eq(&view.unitary, &compiled.unitary));
                    assert!(Arc::ptr_eq(&view.moment_queries, &compiled.moment_queries));
                    for wire in view.decision_wires.values() {
                        assert!(!view.output_wires(&[2, 0]).unwrap().contains(wire));
                    }
                    let options = CompileOptions {
                        compiler_strategy: config.0,
                        moment_preparation: config.1,
                        ..Default::default()
                    };
                    let expected =
                        compile_affine(&parse(&literal(v, w, stage)), &options, &a).unwrap();
                    for x in 0..4 {
                        for z in 0..4 {
                            let axes =
                                (x & 1) + 2 * (z & 1) + 4 * ((x >> 1) & 1) + 8 * ((z >> 1) & 1);
                            let word = pauli(2, axes);
                            let value = view
                                .pauli_moment(&[2, 0], &word, &[], 256, &a)
                                .unwrap()
                                .value;
                            assert_eq!(
                                value,
                                expected
                                    .pauli_moment(&[2, 0], &word, &[], 256, &a)
                                    .unwrap()
                                    .value
                            );
                            let row = rows
                                .iter()
                                .find(|row| {
                                    row["protocol"].as_u64().unwrap() == u64::from(v)
                                        && row["recovery"].as_u64().unwrap() == u64::from(w)
                                        && row["stage"] == stage
                                        && row["x"].as_u64().unwrap() == x as u64
                                        && row["z"].as_u64().unwrap() == z as u64
                                })
                                .unwrap();
                            assert_eq!(
                                value,
                                exact(row),
                                "stage={stage},protocol={v},recovery={w},x={x},z={z}"
                            );
                            if value == half_root || value == half_root.negated() {
                                irrational += 1;
                            }
                            checks += 1;
                        }
                    }
                }
            }
        }
    }
    assert_eq!(checks, 512);
    assert!(irrational > 0);
    let files = Files::new();
    let mut registry = ControllerRegistry::default();
    registry
        .register("echo", "1", |bits: &[bool]| Ok(bits.to_vec()))
        .unwrap();
    let parse_options = crate::circuit::ParseOptions {
        controllers: registry.names(),
        ..Default::default()
    };
    let physical = crate::circuit::parse_circuit(
        "H 0 1 2\nT 2\nM 1\nDETECTOR rec[-1]\nCONTROL[echo] det[-1] -> ctrl[0]\nCH ctrl[0] 2",
        &parse_options,
    )
    .unwrap();
    let modules=parse_task(r#"{"outputs":[2,0],"sample_records":[0],"ideal_state":"target.magna","recovery":"recovery.magna"}"#,&files.0,&TaskOptions::default()).unwrap().load_modules(&physical,&parse_options).unwrap();
    for compiler in [
        crate::unitary::CompilerStrategy::ParityCopies,
        crate::unitary::CompilerStrategy::SharedCommutingLayer,
    ] {
        for causal in [CausalStrategy::ColdPrefix, CausalStrategy::FixedTemplate] {
            for sampler in [HistorySampler::ExactMarginal, HistorySampler::AffineSupport] {
                let mut options = EvaluationOptions::default();
                options.runtime.compile.compiler_strategy = compiler;
                options.runtime.strategy = causal;
                options.runtime.history_sampler = sampler;
                let evaluator =
                    TaskEvaluator::new(&physical, &modules, &registry, options).unwrap();
                for seed in 0..8 {
                    let (draw, diagnostics) = evaluator
                        .sample_with_diagnostics(
                            &mut SplitMix64::new(seed),
                            &TaskSampleDiagnosticOptions::default(),
                        )
                        .unwrap();
                    assert_eq!(diagnostics.protocol_controller_calls, 1);
                    assert_eq!(diagnostics.recovery_controller_calls, 1);
                    assert_eq!(draw.controller_calls.len(), 2);
                    assert_eq!(draw.controller_calls[0].call, 0);
                    assert_eq!(draw.controller_calls[1].call, 1);
                    for call in &draw.controller_calls {
                        assert_eq!(call.name, "echo");
                        assert_eq!(call.inputs, call.outputs);
                    }
                    assert_eq!(draw.sampled_records.len(), 1);
                    assert_eq!(draw.sampled_records[0], draw.controller_calls[0].inputs[0]);
                    assert_eq!(draw.target_normalization, Some(ExactScalar::one()));
                    if causal == CausalStrategy::FixedTemplate {
                        assert_eq!(draw.network_compilations, 0);
                    }
                }
            }
        }
    }
}
