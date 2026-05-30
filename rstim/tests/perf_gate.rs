use rstim::perf::{
    PerfGateConfig, PerfGateStatus, PerfSummary, PerfSummaryIssue, PerfSummaryIssueKind,
    evaluate_summary, summarize_jsonl_str,
};

const FULL_RAW_JSONL: &str = concat!(
    "{\"case_label\":\"rep-sample-d13-r13\",\"tool_variant\":\"stim-cli\",\"workload\":\"sample\",\"tier\":\"gating\",\"measurement_index\":0,\"warmup\":false,\"qubits\":25,\"measurements\":48,\"detectors\":0,\"observables\":0,\"repeat_depth\":1,\"repeat_count\":13,\"shots\":20000,\"wall_time_ns\":130,\"peak_memory_bytes\":1024}\n",
    "{\"case_label\":\"rep-sample-d13-r13\",\"tool_variant\":\"rstim-interpreted\",\"workload\":\"sample\",\"tier\":\"gating\",\"measurement_index\":0,\"warmup\":false,\"qubits\":25,\"measurements\":48,\"detectors\":0,\"observables\":0,\"repeat_depth\":1,\"repeat_count\":13,\"shots\":20000,\"wall_time_ns\":100,\"peak_memory_bytes\":4096}\n",
    "{\"case_label\":\"rep-sample-d13-r13\",\"tool_variant\":\"rstim-compiled\",\"workload\":\"sample\",\"tier\":\"gating\",\"measurement_index\":0,\"warmup\":false,\"qubits\":25,\"measurements\":48,\"detectors\":0,\"observables\":0,\"repeat_depth\":1,\"repeat_count\":13,\"shots\":20000,\"wall_time_ns\":80,\"peak_memory_bytes\":2048}\n",
    "{\"case_label\":\"surface-detect-d13-r13\",\"tool_variant\":\"stim-cli\",\"workload\":\"detect\",\"tier\":\"gating\",\"measurement_index\":0,\"warmup\":false,\"qubits\":169,\"measurements\":312,\"detectors\":144,\"observables\":1,\"repeat_depth\":1,\"repeat_count\":13,\"shots\":10000,\"wall_time_ns\":240,\"peak_memory_bytes\":4096}\n",
    "{\"case_label\":\"surface-detect-d13-r13\",\"tool_variant\":\"rstim-interpreted\",\"workload\":\"detect\",\"tier\":\"gating\",\"measurement_index\":0,\"warmup\":false,\"qubits\":169,\"measurements\":312,\"detectors\":144,\"observables\":1,\"repeat_depth\":1,\"repeat_count\":13,\"shots\":10000,\"wall_time_ns\":210,\"peak_memory_bytes\":8192}\n",
    "{\"case_label\":\"surface-detect-d13-r13\",\"tool_variant\":\"rstim-compiled\",\"workload\":\"detect\",\"tier\":\"gating\",\"measurement_index\":0,\"warmup\":false,\"qubits\":169,\"measurements\":312,\"detectors\":144,\"observables\":1,\"repeat_depth\":1,\"repeat_count\":13,\"shots\":10000,\"wall_time_ns\":170,\"peak_memory_bytes\":6144}\n",
    "{\"case_label\":\"repeat-analyze-large\",\"tool_variant\":\"stim-cli\",\"workload\":\"analyze_errors\",\"tier\":\"gating\",\"measurement_index\":0,\"warmup\":false,\"qubits\":1,\"measurements\":1,\"detectors\":1,\"observables\":0,\"repeat_depth\":1,\"repeat_count\":4096,\"shots\":null,\"wall_time_ns\":700,\"peak_memory_bytes\":512}\n",
    "{\"case_label\":\"repeat-analyze-large\",\"tool_variant\":\"rstim-analyzer-flattened\",\"workload\":\"analyze_errors\",\"tier\":\"gating\",\"measurement_index\":0,\"warmup\":false,\"qubits\":1,\"measurements\":1,\"detectors\":1,\"observables\":0,\"repeat_depth\":1,\"repeat_count\":4096,\"shots\":null,\"wall_time_ns\":600,\"peak_memory_bytes\":1024}\n",
    "{\"case_label\":\"repeat-analyze-large\",\"tool_variant\":\"rstim-analyzer-compiled\",\"workload\":\"analyze_errors\",\"tier\":\"gating\",\"measurement_index\":0,\"warmup\":false,\"qubits\":1,\"measurements\":1,\"detectors\":1,\"observables\":0,\"repeat_depth\":1,\"repeat_count\":4096,\"shots\":null,\"wall_time_ns\":500,\"peak_memory_bytes\":768}\n",
    "{\"case_label\":\"loss-protection-sample\",\"tool_variant\":\"stim-cli\",\"workload\":\"sample\",\"tier\":\"gating\",\"measurement_index\":0,\"warmup\":false,\"qubits\":1,\"measurements\":1,\"detectors\":1,\"observables\":0,\"repeat_depth\":1,\"repeat_count\":0,\"shots\":128,\"wall_time_ns\":80,\"peak_memory_bytes\":128}\n",
    "{\"case_label\":\"loss-protection-sample\",\"tool_variant\":\"rstim-interpreted\",\"workload\":\"sample\",\"tier\":\"gating\",\"measurement_index\":0,\"warmup\":false,\"qubits\":1,\"measurements\":1,\"detectors\":1,\"observables\":0,\"repeat_depth\":1,\"repeat_count\":0,\"shots\":128,\"wall_time_ns\":70,\"peak_memory_bytes\":256}\n"
);

fn full_summary() -> PerfSummary {
    summarize_jsonl_str(FULL_RAW_JSONL).expect("full perf summary")
}

#[test]
fn gate_rejects_missing_required_variant() {
    let mut summary = full_summary();
    let case = summary
        .cases
        .iter_mut()
        .find(|case| case.case_label == "rep-sample-d13-r13")
        .expect("rep sample case");
    case.present_variants = vec!["stim-cli".to_string(), "rstim-interpreted".to_string()];
    case.variants
        .retain(|variant| variant.tool_variant != "rstim-compiled");
    case.comparisons.clear();

    let verdict = evaluate_summary(&summary, PerfGateConfig::default());
    assert_eq!(verdict.status, PerfGateStatus::ContractFailure);
    assert!(verdict.messages.iter().any(|message| {
        message.contains("rep-sample-d13-r13 missing required variant rstim-compiled")
    }));
}

#[test]
fn gate_rejects_sampler_regressions_above_threshold() {
    let mut summary = full_summary();
    summary
        .cases
        .iter_mut()
        .find(|case| case.case_label == "rep-sample-d13-r13")
        .expect("rep sample case")
        .variants
        .iter_mut()
        .find(|variant| variant.tool_variant == "rstim-compiled")
        .expect("compiled variant")
        .median_wall_time_ns = 111;

    let verdict = evaluate_summary(&summary, PerfGateConfig::default());
    assert_eq!(verdict.status, PerfGateStatus::RegressionFailure);
    assert!(verdict.messages.iter().any(|message| {
        message.contains("rep-sample-d13-r13 sampler_compiled_vs_interpreted ratio 1.110000")
    }));
}

#[test]
fn gate_accepts_gating_fallback_case_without_compiled_variant() {
    let summary = full_summary();

    let verdict = evaluate_summary(&summary, PerfGateConfig::default());
    assert_eq!(verdict.status, PerfGateStatus::Pass);
    assert!(verdict.messages.is_empty());
}

#[test]
fn gate_ignores_report_only_summary_issues() {
    let mut summary = full_summary();
    summary.issues.push(PerfSummaryIssue {
            kind: PerfSummaryIssueKind::MissingComparisonVariants,
            case_label: "repeat-analyze-stress-report".to_string(),
            message: "missing comparison variants for analyzer_compiled_vs_flattened".to_string(),
    });

    let verdict = evaluate_summary(&summary, PerfGateConfig::default());
    assert_eq!(verdict.status, PerfGateStatus::Pass);
    assert!(verdict.messages.is_empty());
}

#[test]
fn gate_rejects_gating_summary_issues() {
    let mut summary = full_summary();
    summary.issues.push(PerfSummaryIssue {
        kind: PerfSummaryIssueKind::MissingBenchmarkCaseData,
        case_label: "surface-detect-d13-r13".to_string(),
        message: "missing benchmark case data for surface-detect-d13-r13".to_string(),
    });

    let verdict = evaluate_summary(&summary, PerfGateConfig::default());
    assert_eq!(verdict.status, PerfGateStatus::ContractFailure);
    assert!(verdict.messages.iter().any(|message| {
        message.contains("MissingBenchmarkCaseData")
            && message.contains("surface-detect-d13-r13")
    }));
}

#[test]
fn gate_uses_repo_owned_gating_contract_when_summary_downgrades_case() {
    let mut summary = full_summary();
    let case = summary
        .cases
        .iter_mut()
        .find(|case| case.case_label == "rep-sample-d13-r13")
        .expect("rep sample case");
    case.tier = "report_only".to_string();
    case.expected_variants = vec!["stim-cli".to_string()];
    case.present_variants = vec!["stim-cli".to_string(), "rstim-interpreted".to_string()];
    case.variants
        .retain(|variant| variant.tool_variant != "rstim-compiled");
    case.comparisons.clear();

    let verdict = evaluate_summary(&summary, PerfGateConfig::default());
    assert_eq!(verdict.status, PerfGateStatus::ContractFailure);
    assert!(verdict.messages.iter().any(|message| {
        message.contains("rep-sample-d13-r13 missing required variant rstim-compiled")
    }));
}

#[test]
fn gate_recomputes_regressions_from_variant_medians_when_summary_comparisons_are_missing() {
    let mut summary = full_summary();
    let case = summary
        .cases
        .iter_mut()
        .find(|case| case.case_label == "rep-sample-d13-r13")
        .expect("rep sample case");
    case.expected_variants = vec!["stim-cli".to_string()];
    case.comparisons.clear();
    case.variants
        .iter_mut()
        .find(|variant| variant.tool_variant == "rstim-compiled")
        .expect("compiled variant")
        .median_wall_time_ns = 111;

    let verdict = evaluate_summary(&summary, PerfGateConfig::default());
    assert_eq!(verdict.status, PerfGateStatus::RegressionFailure);
    assert!(verdict.messages.iter().any(|message| {
        message.contains("rep-sample-d13-r13 sampler_compiled_vs_interpreted ratio 1.110000")
    }));
}

#[test]
fn gate_rejects_missing_gating_case_even_without_summary_issue() {
    let mut summary = full_summary();
    summary
        .cases
        .retain(|case| case.case_label != "surface-detect-d13-r13");

    let verdict = evaluate_summary(&summary, PerfGateConfig::default());
    assert_eq!(verdict.status, PerfGateStatus::ContractFailure);
    assert!(verdict.messages.iter().any(|message| {
        message.contains("missing benchmark case summary for surface-detect-d13-r13")
    }));
}
