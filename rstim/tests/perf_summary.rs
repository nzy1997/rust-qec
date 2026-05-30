use rstim::perf::{render_markdown_report, summarize_jsonl_str};

const RAW_JSONL: &str = concat!(
    "{\"case_label\":\"rep-sample-d13-r13\",\"tool_variant\":\"stim-cli\",\"workload\":\"sample\",\"tier\":\"gating\",\"measurement_index\":0,\"warmup\":true,\"qubits\":25,\"measurements\":48,\"detectors\":0,\"observables\":0,\"repeat_depth\":1,\"repeat_count\":13,\"shots\":20000,\"wall_time_ns\":1200,\"peak_memory_bytes\":1024}\n",
    "{\"case_label\":\"rep-sample-d13-r13\",\"tool_variant\":\"stim-cli\",\"workload\":\"sample\",\"tier\":\"gating\",\"measurement_index\":1,\"warmup\":false,\"qubits\":25,\"measurements\":48,\"detectors\":0,\"observables\":0,\"repeat_depth\":1,\"repeat_count\":13,\"shots\":20000,\"wall_time_ns\":130,\"peak_memory_bytes\":1024}\n",
    "{\"case_label\":\"rep-sample-d13-r13\",\"tool_variant\":\"stim-cli\",\"workload\":\"sample\",\"tier\":\"gating\",\"measurement_index\":2,\"warmup\":false,\"qubits\":25,\"measurements\":48,\"detectors\":0,\"observables\":0,\"repeat_depth\":1,\"repeat_count\":13,\"shots\":20000,\"wall_time_ns\":125,\"peak_memory_bytes\":1024}\n",
    "{\"case_label\":\"rep-sample-d13-r13\",\"tool_variant\":\"stim-cli\",\"workload\":\"sample\",\"tier\":\"gating\",\"measurement_index\":3,\"warmup\":false,\"qubits\":25,\"measurements\":48,\"detectors\":0,\"observables\":0,\"repeat_depth\":1,\"repeat_count\":13,\"shots\":20000,\"wall_time_ns\":135,\"peak_memory_bytes\":1024}\n",
    "{\"case_label\":\"rep-sample-d13-r13\",\"tool_variant\":\"rstim-interpreted\",\"workload\":\"sample\",\"tier\":\"gating\",\"measurement_index\":0,\"warmup\":true,\"qubits\":25,\"measurements\":48,\"detectors\":0,\"observables\":0,\"repeat_depth\":1,\"repeat_count\":13,\"shots\":20000,\"wall_time_ns\":999,\"peak_memory_bytes\":4096}\n",
    "{\"case_label\":\"rep-sample-d13-r13\",\"tool_variant\":\"rstim-interpreted\",\"workload\":\"sample\",\"tier\":\"gating\",\"measurement_index\":1,\"warmup\":false,\"qubits\":25,\"measurements\":48,\"detectors\":0,\"observables\":0,\"repeat_depth\":1,\"repeat_count\":13,\"shots\":20000,\"wall_time_ns\":110,\"peak_memory_bytes\":4096}\n",
    "{\"case_label\":\"rep-sample-d13-r13\",\"tool_variant\":\"rstim-interpreted\",\"workload\":\"sample\",\"tier\":\"gating\",\"measurement_index\":2,\"warmup\":false,\"qubits\":25,\"measurements\":48,\"detectors\":0,\"observables\":0,\"repeat_depth\":1,\"repeat_count\":13,\"shots\":20000,\"wall_time_ns\":100,\"peak_memory_bytes\":4096}\n",
    "{\"case_label\":\"rep-sample-d13-r13\",\"tool_variant\":\"rstim-interpreted\",\"workload\":\"sample\",\"tier\":\"gating\",\"measurement_index\":3,\"warmup\":false,\"qubits\":25,\"measurements\":48,\"detectors\":0,\"observables\":0,\"repeat_depth\":1,\"repeat_count\":13,\"shots\":20000,\"wall_time_ns\":90,\"peak_memory_bytes\":4096}\n",
    "{\"case_label\":\"rep-sample-d13-r13\",\"tool_variant\":\"rstim-compiled\",\"workload\":\"sample\",\"tier\":\"gating\",\"measurement_index\":0,\"warmup\":true,\"qubits\":25,\"measurements\":48,\"detectors\":0,\"observables\":0,\"repeat_depth\":1,\"repeat_count\":13,\"shots\":20000,\"wall_time_ns\":888,\"peak_memory_bytes\":2048}\n",
    "{\"case_label\":\"rep-sample-d13-r13\",\"tool_variant\":\"rstim-compiled\",\"workload\":\"sample\",\"tier\":\"gating\",\"measurement_index\":1,\"warmup\":false,\"qubits\":25,\"measurements\":48,\"detectors\":0,\"observables\":0,\"repeat_depth\":1,\"repeat_count\":13,\"shots\":20000,\"wall_time_ns\":80,\"peak_memory_bytes\":2048}\n",
    "{\"case_label\":\"rep-sample-d13-r13\",\"tool_variant\":\"rstim-compiled\",\"workload\":\"sample\",\"tier\":\"gating\",\"measurement_index\":2,\"warmup\":false,\"qubits\":25,\"measurements\":48,\"detectors\":0,\"observables\":0,\"repeat_depth\":1,\"repeat_count\":13,\"shots\":20000,\"wall_time_ns\":70,\"peak_memory_bytes\":2048}\n",
    "{\"case_label\":\"rep-sample-d13-r13\",\"tool_variant\":\"rstim-compiled\",\"workload\":\"sample\",\"tier\":\"gating\",\"measurement_index\":3,\"warmup\":false,\"qubits\":25,\"measurements\":48,\"detectors\":0,\"observables\":0,\"repeat_depth\":1,\"repeat_count\":13,\"shots\":20000,\"wall_time_ns\":90,\"peak_memory_bytes\":2048}\n",
    "{\"case_label\":\"repeat-analyze-stress-report\",\"tool_variant\":\"stim-cli\",\"workload\":\"analyze_errors\",\"tier\":\"report_only\",\"measurement_index\":0,\"warmup\":true,\"qubits\":1,\"measurements\":1,\"detectors\":1,\"observables\":0,\"repeat_depth\":1,\"repeat_count\":8192,\"shots\":null,\"wall_time_ns\":5000,\"peak_memory_bytes\":1000}\n",
    "{\"case_label\":\"repeat-analyze-stress-report\",\"tool_variant\":\"stim-cli\",\"workload\":\"analyze_errors\",\"tier\":\"report_only\",\"measurement_index\":1,\"warmup\":false,\"qubits\":1,\"measurements\":1,\"detectors\":1,\"observables\":0,\"repeat_depth\":1,\"repeat_count\":8192,\"shots\":null,\"wall_time_ns\":600,\"peak_memory_bytes\":1000}\n",
    "{\"case_label\":\"repeat-analyze-stress-report\",\"tool_variant\":\"stim-cli\",\"workload\":\"analyze_errors\",\"tier\":\"report_only\",\"measurement_index\":2,\"warmup\":false,\"qubits\":1,\"measurements\":1,\"detectors\":1,\"observables\":0,\"repeat_depth\":1,\"repeat_count\":8192,\"shots\":null,\"wall_time_ns\":650,\"peak_memory_bytes\":1000}\n",
    "{\"case_label\":\"repeat-analyze-stress-report\",\"tool_variant\":\"stim-cli\",\"workload\":\"analyze_errors\",\"tier\":\"report_only\",\"measurement_index\":3,\"warmup\":false,\"qubits\":1,\"measurements\":1,\"detectors\":1,\"observables\":0,\"repeat_depth\":1,\"repeat_count\":8192,\"shots\":null,\"wall_time_ns\":700,\"peak_memory_bytes\":1000}\n"
);

#[test]
fn summarize_jsonl_aggregates_medians_and_ratios() {
    let summary = summarize_jsonl_str(RAW_JSONL).expect("summary");
    let case = summary
        .cases
        .iter()
        .find(|case| case.case_label == "rep-sample-d13-r13")
        .expect("sample case");

    let interpreted = case
        .variants
        .iter()
        .find(|variant| variant.tool_variant == "rstim-interpreted")
        .expect("interpreted variant");
    let compiled = case
        .variants
        .iter()
        .find(|variant| variant.tool_variant == "rstim-compiled")
        .expect("compiled variant");

    assert_eq!(interpreted.sample_count, 3);
    assert_eq!(compiled.sample_count, 3);
    assert_eq!(interpreted.median_wall_time_ns, 100);
    assert_eq!(compiled.median_wall_time_ns, 80);

    let ratio = case
        .comparisons
        .iter()
        .find(|comparison| comparison.kind == "sampler_compiled_vs_interpreted")
        .expect("sampler comparison");
    assert!((ratio.ratio - 0.8).abs() < 1e-9);
}

#[test]
fn markdown_report_groups_gating_cases() {
    let summary = summarize_jsonl_str(RAW_JSONL).expect("summary");
    let report = render_markdown_report(&summary, None);

    assert!(report.contains("# rstim Performance Evidence Report"));
    assert!(report.contains("## Gating Cases"));
    assert!(report.contains("## Report-Only Cases"));
    assert!(report.contains("rep-sample-d13-r13"));
    assert!(report.contains("repeat-analyze-stress-report"));
    assert!(report.contains("sampler_compiled_vs_interpreted"));
}

#[test]
fn report_surfaces_skipped_comparison_context_when_variants_are_missing() {
    let summary = summarize_jsonl_str(RAW_JSONL).expect("summary");

    assert!(summary.issues.iter().any(|issue| {
        issue.case_label == "repeat-analyze-stress-report"
            && issue.message.contains("analyzer_compiled_vs_flattened")
    }));

    let report = render_markdown_report(&summary, None);
    assert!(report.contains("## Summary Issues"));
    assert!(report.contains("repeat-analyze-stress-report"));
    assert!(report.contains("analyzer_compiled_vs_flattened"));
    assert!(report.contains("missing comparison variants"));
}

#[test]
fn report_surfaces_missing_benchmark_cases() {
    let summary = summarize_jsonl_str(RAW_JSONL).expect("summary");

    assert!(summary.issues.iter().any(|issue| {
        issue.case_label == "surface-detect-d13-r13"
            && issue.message.contains("missing benchmark case data")
    }));

    let report = render_markdown_report(&summary, None);
    assert!(report.contains("## Summary Issues"));
    assert!(report.contains("surface-detect-d13-r13"));
    assert!(report.contains("missing benchmark case data"));
}

#[test]
fn duplicate_measurements_do_not_pollute_variant_aggregates() {
    const DUPLICATE_RAW_JSONL: &str = concat!(
        "{\"case_label\":\"rep-sample-d13-r13\",\"tool_variant\":\"rstim-interpreted\",\"workload\":\"sample\",\"tier\":\"gating\",\"measurement_index\":1,\"warmup\":false,\"qubits\":25,\"measurements\":48,\"detectors\":0,\"observables\":0,\"repeat_depth\":1,\"repeat_count\":13,\"shots\":20000,\"wall_time_ns\":10,\"peak_memory_bytes\":100}\n",
        "{\"case_label\":\"rep-sample-d13-r13\",\"tool_variant\":\"rstim-interpreted\",\"workload\":\"sample\",\"tier\":\"gating\",\"measurement_index\":2,\"warmup\":false,\"qubits\":25,\"measurements\":48,\"detectors\":0,\"observables\":0,\"repeat_depth\":1,\"repeat_count\":13,\"shots\":20000,\"wall_time_ns\":30,\"peak_memory_bytes\":100}\n",
        "{\"case_label\":\"rep-sample-d13-r13\",\"tool_variant\":\"rstim-interpreted\",\"workload\":\"sample\",\"tier\":\"gating\",\"measurement_index\":2,\"warmup\":false,\"qubits\":25,\"measurements\":48,\"detectors\":0,\"observables\":0,\"repeat_depth\":1,\"repeat_count\":13,\"shots\":20000,\"wall_time_ns\":1000,\"peak_memory_bytes\":100}\n",
        "{\"case_label\":\"rep-sample-d13-r13\",\"tool_variant\":\"rstim-interpreted\",\"workload\":\"sample\",\"tier\":\"gating\",\"measurement_index\":3,\"warmup\":false,\"qubits\":25,\"measurements\":48,\"detectors\":0,\"observables\":0,\"repeat_depth\":1,\"repeat_count\":13,\"shots\":20000,\"wall_time_ns\":50,\"peak_memory_bytes\":100}\n"
    );

    let summary = summarize_jsonl_str(DUPLICATE_RAW_JSONL).expect("summary");
    let case = summary
        .cases
        .iter()
        .find(|case| case.case_label == "rep-sample-d13-r13")
        .expect("sample case");
    let interpreted = case
        .variants
        .iter()
        .find(|variant| variant.tool_variant == "rstim-interpreted")
        .expect("interpreted variant");

    assert!(summary.issues.iter().any(|issue| {
        issue.case_label == "rep-sample-d13-r13"
            && issue.message.contains("duplicate measurement slot")
    }));
    assert_eq!(interpreted.sample_count, 3);
    assert_eq!(interpreted.median_wall_time_ns, 30);
}
