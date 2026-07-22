#[test]
fn performance_doc_mentions_perf_ci_and_artifacts() {
    let doc = include_str!("../doc/performance_parity.md");
    assert!(doc.contains("perf ci"));
    assert!(doc.contains("raw.jsonl"));
    assert!(doc.contains("summary.json"));
    assert!(doc.contains("report.md"));
}

#[test]
fn ci_workflow_has_perf_gate_job() {
    let workflow = include_str!("../../.github/workflows/ci.yml");
    assert!(workflow.contains("perf-gate:"));
    assert!(workflow.contains("cargo run --locked -p rstim --bin rstim -- perf ci --out-dir"));
    assert!(!workflow.contains("cargo run -p rstim --bin rstim -- perf ci --out-dir"));
    assert!(workflow.contains("continue-on-error: true"));
    assert!(workflow.contains("actions/upload-artifact"));
    assert!(workflow.contains("path: perf-artifacts/"));
    assert!(workflow.contains("GITHUB_STEP_SUMMARY"));
    assert!(workflow.contains("perf-artifacts/report.md"));
    assert!(workflow.contains("if: steps.perf_ci.outcome != 'success'"));
}
