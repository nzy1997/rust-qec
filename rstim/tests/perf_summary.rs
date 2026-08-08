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

#[test]
fn summarize_rejects_unknown_benchmark_case_labels() {
    let err = summarize_jsonl_str(concat!(
        "{\"case_label\":\"unknown-case\",\"tool_variant\":\"stim-cli\",\"workload\":\"sample\",\"tier\":\"gating\",\"measurement_index\":0,\"warmup\":false,\"qubits\":1,\"measurements\":1,\"detectors\":0,\"observables\":0,\"repeat_depth\":1,\"repeat_count\":1,\"shots\":1,\"wall_time_ns\":10,\"peak_memory_bytes\":null}\n"
    ))
    .unwrap_err();

    assert!(err.contains("unknown benchmark case label in raw jsonl"));
    assert!(err.contains("unknown-case"));
}

#[test]
fn summarize_surfaces_metadata_mismatches_and_conflicting_case_metadata() {
    let summary = summarize_jsonl_str(concat!(
        "{\"case_label\":\"loss-protection-sample\",\"tool_variant\":\"stim-cli\",\"workload\":\"detect\",\"tier\":\"report_only\",\"measurement_index\":0,\"warmup\":false,\"qubits\":1,\"measurements\":1,\"detectors\":1,\"observables\":0,\"repeat_depth\":1,\"repeat_count\":0,\"shots\":128,\"wall_time_ns\":80,\"peak_memory_bytes\":128}\n",
        "{\"case_label\":\"loss-protection-sample\",\"tool_variant\":\"rstim-interpreted\",\"workload\":\"sample\",\"tier\":\"gating\",\"measurement_index\":0,\"warmup\":false,\"qubits\":2,\"measurements\":1,\"detectors\":1,\"observables\":0,\"repeat_depth\":1,\"repeat_count\":0,\"shots\":128,\"wall_time_ns\":70,\"peak_memory_bytes\":256}\n"
    ))
    .expect("summary");

    assert!(summary.issues.iter().any(|issue| {
        issue.case_label == "loss-protection-sample"
            && issue
                .message
                .contains("workload mismatch: expected sample but saw detect")
    }));
    assert!(summary.issues.iter().any(|issue| {
        issue.case_label == "loss-protection-sample"
            && issue
                .message
                .contains("tier mismatch: expected gating but saw report_only")
    }));
    assert!(summary.issues.iter().any(|issue| {
        issue.case_label == "loss-protection-sample"
            && issue
                .message
                .contains("conflicting record metadata within case")
    }));
}

#[test]
fn summarize_rejects_variants_with_only_warmup_measurements() {
    let err = summarize_jsonl_str(concat!(
        "{\"case_label\":\"loss-protection-sample\",\"tool_variant\":\"stim-cli\",\"workload\":\"sample\",\"tier\":\"gating\",\"measurement_index\":0,\"warmup\":true,\"qubits\":1,\"measurements\":1,\"detectors\":1,\"observables\":0,\"repeat_depth\":1,\"repeat_count\":0,\"shots\":128,\"wall_time_ns\":80,\"peak_memory_bytes\":128}\n"
    ))
    .unwrap_err();

    assert!(
        err.contains("missing measured records for case loss-protection-sample variant stim-cli")
    );
}

#[test]
fn summarize_reports_missing_comparison_lhs_variants() {
    let summary = summarize_jsonl_str(concat!(
        "{\"case_label\":\"rep-sample-d13-r13\",\"tool_variant\":\"stim-cli\",\"workload\":\"sample\",\"tier\":\"gating\",\"measurement_index\":0,\"warmup\":false,\"qubits\":25,\"measurements\":48,\"detectors\":0,\"observables\":0,\"repeat_depth\":1,\"repeat_count\":13,\"shots\":20000,\"wall_time_ns\":130,\"peak_memory_bytes\":1024}\n",
        "{\"case_label\":\"rep-sample-d13-r13\",\"tool_variant\":\"rstim-interpreted\",\"workload\":\"sample\",\"tier\":\"gating\",\"measurement_index\":0,\"warmup\":false,\"qubits\":25,\"measurements\":48,\"detectors\":0,\"observables\":0,\"repeat_depth\":1,\"repeat_count\":13,\"shots\":20000,\"wall_time_ns\":100,\"peak_memory_bytes\":4096}\n"
    ))
    .expect("summary");

    assert!(summary.issues.iter().any(|issue| {
        issue.case_label == "rep-sample-d13-r13"
            && issue.message.contains("missing `rstim-compiled`")
    }));
}

#[test]
fn summarize_reports_non_completed_comparison_variants_as_missing() {
    let lhs_failed = summarize_jsonl_str(concat!(
        "{\"case_label\":\"rep-sample-d13-r13\",\"tool_variant\":\"stim-cli\",\"workload\":\"sample\",\"tier\":\"gating\",\"measurement_index\":0,\"warmup\":false,\"qubits\":25,\"measurements\":48,\"detectors\":0,\"observables\":0,\"repeat_depth\":1,\"repeat_count\":13,\"shots\":20000,\"wall_time_ns\":130,\"peak_memory_bytes\":1024,\"status\":\"completed\",\"failure_reason\":null,\"stderr\":null}\n",
        "{\"case_label\":\"rep-sample-d13-r13\",\"tool_variant\":\"rstim-interpreted\",\"workload\":\"sample\",\"tier\":\"gating\",\"measurement_index\":0,\"warmup\":false,\"qubits\":25,\"measurements\":48,\"detectors\":0,\"observables\":0,\"repeat_depth\":1,\"repeat_count\":13,\"shots\":20000,\"wall_time_ns\":100,\"peak_memory_bytes\":4096,\"status\":\"completed\",\"failure_reason\":null,\"stderr\":null}\n",
        "{\"case_label\":\"rep-sample-d13-r13\",\"tool_variant\":\"rstim-compiled\",\"workload\":\"sample\",\"tier\":\"gating\",\"measurement_index\":0,\"warmup\":false,\"qubits\":25,\"measurements\":48,\"detectors\":0,\"observables\":0,\"repeat_depth\":1,\"repeat_count\":13,\"shots\":20000,\"wall_time_ns\":0,\"peak_memory_bytes\":null,\"status\":\"tool_failed\",\"failure_reason\":\"compiled failed\",\"stderr\":\"compiled failed\\n\"}\n"
    ))
    .expect("lhs failed summary");
    assert!(lhs_failed.issues.iter().any(|issue| {
        issue.case_label == "rep-sample-d13-r13"
            && issue.message.contains("missing `rstim-compiled`")
    }));
    assert!(
        lhs_failed
            .cases
            .iter()
            .find(|case| case.case_label == "rep-sample-d13-r13")
            .unwrap()
            .comparisons
            .is_empty()
    );

    let rhs_failed = summarize_jsonl_str(concat!(
        "{\"case_label\":\"rep-sample-d13-r13\",\"tool_variant\":\"stim-cli\",\"workload\":\"sample\",\"tier\":\"gating\",\"measurement_index\":0,\"warmup\":false,\"qubits\":25,\"measurements\":48,\"detectors\":0,\"observables\":0,\"repeat_depth\":1,\"repeat_count\":13,\"shots\":20000,\"wall_time_ns\":130,\"peak_memory_bytes\":1024,\"status\":\"completed\",\"failure_reason\":null,\"stderr\":null}\n",
        "{\"case_label\":\"rep-sample-d13-r13\",\"tool_variant\":\"rstim-interpreted\",\"workload\":\"sample\",\"tier\":\"gating\",\"measurement_index\":0,\"warmup\":false,\"qubits\":25,\"measurements\":48,\"detectors\":0,\"observables\":0,\"repeat_depth\":1,\"repeat_count\":13,\"shots\":20000,\"wall_time_ns\":0,\"peak_memory_bytes\":null,\"status\":\"tool_failed\",\"failure_reason\":\"interpreted failed\",\"stderr\":\"interpreted failed\\n\"}\n",
        "{\"case_label\":\"rep-sample-d13-r13\",\"tool_variant\":\"rstim-compiled\",\"workload\":\"sample\",\"tier\":\"gating\",\"measurement_index\":0,\"warmup\":false,\"qubits\":25,\"measurements\":48,\"detectors\":0,\"observables\":0,\"repeat_depth\":1,\"repeat_count\":13,\"shots\":20000,\"wall_time_ns\":80,\"peak_memory_bytes\":2048,\"status\":\"completed\",\"failure_reason\":null,\"stderr\":null}\n"
    ))
    .expect("rhs failed summary");
    assert!(rhs_failed.issues.iter().any(|issue| {
        issue.case_label == "rep-sample-d13-r13"
            && issue.message.contains("missing `rstim-interpreted`")
    }));
    assert!(
        rhs_failed
            .cases
            .iter()
            .find(|case| case.case_label == "rep-sample-d13-r13")
            .unwrap()
            .comparisons
            .is_empty()
    );
}

#[test]
fn summarize_uses_none_when_all_memory_samples_are_missing() {
    let summary = summarize_jsonl_str(concat!(
        "{\"case_label\":\"loss-protection-sample\",\"tool_variant\":\"stim-cli\",\"workload\":\"sample\",\"tier\":\"gating\",\"measurement_index\":0,\"warmup\":false,\"qubits\":1,\"measurements\":1,\"detectors\":1,\"observables\":0,\"repeat_depth\":1,\"repeat_count\":0,\"shots\":128,\"wall_time_ns\":80,\"peak_memory_bytes\":null}\n",
        "{\"case_label\":\"loss-protection-sample\",\"tool_variant\":\"rstim-interpreted\",\"workload\":\"sample\",\"tier\":\"gating\",\"measurement_index\":0,\"warmup\":false,\"qubits\":1,\"measurements\":1,\"detectors\":1,\"observables\":0,\"repeat_depth\":1,\"repeat_count\":0,\"shots\":128,\"wall_time_ns\":70,\"peak_memory_bytes\":null}\n"
    ))
    .expect("summary");

    let case = summary
        .cases
        .iter()
        .find(|case| case.case_label == "loss-protection-sample")
        .expect("loss protection case");
    assert!(
        case.variants
            .iter()
            .all(|variant| variant.median_peak_memory_bytes.is_none())
    );
}

#[test]
fn report_renders_optional_newline_and_empty_sections() {
    let summary = rstim::perf::PerfSummary {
        cases: Vec::new(),
        issues: Vec::new(),
    };

    let report = render_markdown_report(&summary, Some("PASS"));
    assert!(report.contains("## Gate Verdict"));
    assert!(report.contains("PASS\n\n"));
    assert!(report.contains("## Gating Cases\n\n_None._"));
    assert!(report.contains("## Report-Only Cases\n\n_None._"));
}

#[test]
fn legacy_summary_json_defaults_variant_status_to_completed() {
    let summary: rstim::perf::PerfSummary = serde_json::from_str(
        r#"{
            "cases": [
                {
                    "case_label": "loss-protection-sample",
                    "workload": "sample",
                    "tier": "gating",
                    "requires_compiled": false,
                    "requires_fallback": true,
                    "expected_variants": ["stim-cli", "rstim-interpreted"],
                    "present_variants": ["stim-cli"],
                    "variants": [
                        {
                            "tool_variant": "stim-cli",
                            "sample_count": 1,
                            "median_wall_time_ns": 80,
                            "median_peak_memory_bytes": null
                        }
                    ],
                    "comparisons": []
                }
            ],
            "issues": []
        }"#,
    )
    .expect("legacy summary json");

    let variant = &summary.cases[0].variants[0];
    assert_eq!(variant.status, "completed");
    assert!(variant.failure_reason.is_none());
    assert!(variant.stderr.is_none());
}

#[test]
fn selected_summary_keeps_failed_variant_and_omits_unrelated_missing_cases() {
    let raw = concat!(
        "{\"case_label\":\"loss-protection-sample\",\"tool_variant\":\"stim-cli\",\"workload\":\"sample\",\"tier\":\"gating\",\"measurement_index\":0,\"warmup\":false,\"qubits\":1,\"measurements\":1,\"detectors\":1,\"observables\":0,\"repeat_depth\":1,\"repeat_count\":0,\"shots\":128,\"wall_time_ns\":0,\"peak_memory_bytes\":null,\"status\":\"tool_failed\",\"failure_reason\":\"stim failed: boom\",\"stderr\":\"boom\\n\"}\n",
        "{\"case_label\":\"loss-protection-sample\",\"tool_variant\":\"rstim-interpreted\",\"workload\":\"sample\",\"tier\":\"gating\",\"measurement_index\":0,\"warmup\":false,\"qubits\":1,\"measurements\":1,\"detectors\":1,\"observables\":0,\"repeat_depth\":1,\"repeat_count\":0,\"shots\":128,\"wall_time_ns\":70,\"peak_memory_bytes\":256,\"status\":\"completed\",\"failure_reason\":null,\"stderr\":null}\n"
    );

    let summary = rstim::perf::summarize_jsonl_str_with_options(
        raw,
        rstim::perf::PerfSummaryOptions {
            case_label: Some("loss-protection-sample".to_string()),
        },
    )
    .unwrap();

    assert_eq!(summary.cases.len(), 1);
    assert_eq!(summary.cases[0].case_label, "loss-protection-sample");
    assert!(
        !summary
            .issues
            .iter()
            .any(|issue| issue.message.contains("missing benchmark case data"))
    );

    let stim = summary.cases[0]
        .variants
        .iter()
        .find(|variant| variant.tool_variant == "stim-cli")
        .unwrap();
    assert_eq!(stim.status, "tool_failed");
    assert_eq!(stim.sample_count, 0);
    assert_eq!(stim.failure_reason.as_deref(), Some("stim failed: boom"));

    let report = render_markdown_report(&summary, None);
    assert!(report.contains("stim-cli status: `tool_failed`"));
    assert!(report.contains("stim failed: boom"));
}

#[test]
fn selected_summary_ignores_unrelated_raw_issues() {
    let raw = concat!(
        "{\"case_label\":\"rep-sample-d13-r13\",\"tool_variant\":\"stim-cli\",\"workload\":\"sample\",\"tier\":\"gating\",\"measurement_index\":0,\"warmup\":false,\"qubits\":25,\"measurements\":48,\"detectors\":0,\"observables\":0,\"repeat_depth\":1,\"repeat_count\":13,\"shots\":20000,\"wall_time_ns\":130,\"peak_memory_bytes\":1024,\"status\":\"completed\",\"failure_reason\":null,\"stderr\":null}\n",
        "{\"case_label\":\"rep-sample-d13-r13\",\"tool_variant\":\"stim-cli\",\"workload\":\"sample\",\"tier\":\"gating\",\"measurement_index\":0,\"warmup\":false,\"qubits\":25,\"measurements\":48,\"detectors\":0,\"observables\":0,\"repeat_depth\":1,\"repeat_count\":13,\"shots\":20000,\"wall_time_ns\":999,\"peak_memory_bytes\":1024,\"status\":\"completed\",\"failure_reason\":null,\"stderr\":null}\n",
        "{\"case_label\":\"loss-protection-sample\",\"tool_variant\":\"rstim-interpreted\",\"workload\":\"sample\",\"tier\":\"gating\",\"measurement_index\":0,\"warmup\":false,\"qubits\":1,\"measurements\":1,\"detectors\":1,\"observables\":0,\"repeat_depth\":1,\"repeat_count\":0,\"shots\":128,\"wall_time_ns\":70,\"peak_memory_bytes\":256,\"status\":\"completed\",\"failure_reason\":null,\"stderr\":null}\n"
    );

    let summary = rstim::perf::summarize_jsonl_str_with_options(
        raw,
        rstim::perf::PerfSummaryOptions {
            case_label: Some("loss-protection-sample".to_string()),
        },
    )
    .unwrap();

    assert_eq!(summary.cases.len(), 1);
    assert_eq!(summary.cases[0].case_label, "loss-protection-sample");
    assert!(
        !summary
            .issues
            .iter()
            .any(|issue| issue.case_label == "rep-sample-d13-r13")
    );
}

#[test]
fn legacy_perf_record_omits_tier_for_unknown_case() {
    let record = rstim::perf::PerfRecord {
        case_label: "custom-case".to_string(),
        tool_variant: "stim-cli".to_string(),
        workload: "sample".to_string(),
        qubits: 1,
        measurements: 1,
        detectors: 0,
        observables: 0,
        repeat_depth: 1,
        repeat_count: 1,
        shots: Some(1),
        wall_time_ns: 10,
        peak_memory_bytes: None,
    };

    let line = record.to_json_line();
    assert!(!line.contains("\"tier\""));
}

#[test]
fn summarize_sample_fixture_reports_shot_rates_and_report_only_stim_ratio() {
    let raw = include_str!("fixtures/perf/stim_style_sample_raw.jsonl");
    let summary = summarize_jsonl_str(raw).expect("summary");
    let case = summary
        .cases
        .iter()
        .find(|case| case.case_label == "stim-style-surface-sample-d11-r100-b1024")
        .expect("public sample case");

    let stim = case
        .variants
        .iter()
        .find(|variant| variant.tool_variant == "stim-cli")
        .expect("stim variant");
    let compiled = case
        .variants
        .iter()
        .find(|variant| variant.tool_variant == "rstim-compiled")
        .expect("compiled variant");

    assert_eq!(stim.median_shots_per_second, Some(512_000.0));
    assert_eq!(compiled.median_shots_per_second, Some(256_000.0));
    let atom_loss = case
        .variants
        .iter()
        .find(|variant| variant.tool_variant == "rstim-interpreted-atom-loss")
        .expect("atom-loss variant");
    assert_eq!(
        atom_loss.median_shots_per_second,
        Some(1024.0 * 1_000_000_000.0 / 7_500_000.0)
    );

    let atom_loss_comparison = case
        .comparisons
        .iter()
        .find(|comparison| comparison.kind == "sampler_atom_loss_vs_interpreted")
        .expect("atom-loss comparison");
    assert_eq!(
        atom_loss_comparison.lhs_variant,
        "rstim-interpreted-atom-loss"
    );
    assert_eq!(atom_loss_comparison.rhs_variant, "rstim-interpreted");
    assert_eq!(atom_loss_comparison.ratio, 1.5);

    let comparison = case
        .rstim_compiled_vs_stim_cli_ratio
        .as_ref()
        .expect("report-only stim comparison");
    assert_eq!(comparison.kind, "rstim_compiled_vs_stim_cli");
    assert_eq!(comparison.lhs_variant, "rstim-compiled");
    assert_eq!(comparison.rhs_variant, "stim-cli");
    assert_eq!(comparison.status, "completed");
    assert_eq!(comparison.failure_reason, None);
    assert_eq!(comparison.ratio, Some(2.0));

    let summary_json = serde_json::to_value(&summary).unwrap();
    let public_case = summary_json["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["case_label"] == "stim-style-surface-sample-d11-r100-b1024")
        .unwrap();
    assert!(public_case.to_string().contains("median_shots_per_second"));
    assert!(
        public_case
            .to_string()
            .contains("rstim_compiled_vs_stim_cli_ratio")
    );

    let report = render_markdown_report(&summary, None);
    assert!(report.contains("stim-style-surface-sample-d11-r100-b1024"));
    assert!(report.contains("shots/s"));
    assert!(report.contains("report-only Stim comparison"));
    assert!(report.contains("2.000000"));
    assert!(report.contains("sampler_atom_loss_vs_interpreted"));
    assert!(report.contains("1.500000"));
    assert!(report.contains("p = 1 - 0.999^(1/3) ~= 0.0003334445062"));
    assert!(report.contains("probability of at least one error equal to `0.001`"));
    assert!(report.contains("loss masks and Pauli frames are propagated in 64-shot bitsets"));
}

#[test]
fn summarize_report_only_stim_comparison_surfaces_failed_variant_status() {
    let raw = concat!(
        "{\"case_label\":\"stim-style-surface-sample-d11-r100-b1024\",\"tool_variant\":\"stim-cli\",\"workload\":\"sample\",\"tier\":\"report_only\",\"measurement_index\":0,\"warmup\":false,\"qubits\":1,\"measurements\":1,\"detectors\":0,\"observables\":0,\"repeat_depth\":1,\"repeat_count\":100,\"shots\":1024,\"wall_time_ns\":0,\"peak_memory_bytes\":null,\"status\":\"tool_failed\",\"failure_reason\":\"stim failed: boom\",\"stderr\":\"boom\\n\"}\n",
        "{\"case_label\":\"stim-style-surface-sample-d11-r100-b1024\",\"tool_variant\":\"rstim-compiled\",\"workload\":\"sample\",\"tier\":\"report_only\",\"measurement_index\":0,\"warmup\":false,\"qubits\":1,\"measurements\":1,\"detectors\":0,\"observables\":0,\"repeat_depth\":1,\"repeat_count\":100,\"shots\":1024,\"wall_time_ns\":4000000,\"peak_memory_bytes\":1500,\"status\":\"completed\",\"failure_reason\":null,\"stderr\":null}\n"
    );

    let summary = summarize_jsonl_str(raw).expect("summary");
    let case = summary
        .cases
        .iter()
        .find(|case| case.case_label == "stim-style-surface-sample-d11-r100-b1024")
        .expect("public sample case");
    let comparison = case
        .rstim_compiled_vs_stim_cli_ratio
        .as_ref()
        .expect("report-only stim comparison");

    assert_eq!(comparison.ratio, None);
    assert_eq!(comparison.status, "tool_failed");
    assert_eq!(
        comparison.failure_reason.as_deref(),
        Some("stim failed: boom")
    );

    let report = render_markdown_report(&summary, None);
    assert!(report.contains("report-only Stim comparison unavailable"));
    assert!(report.contains("tool_failed"));
    assert!(report.contains("stim failed: boom"));
}

#[test]
fn summarize_report_only_stim_comparison_surfaces_missing_variant_status() {
    let raw = concat!(
        "{\"case_label\":\"stim-style-surface-sample-d11-r100-b1024\",\"tool_variant\":\"stim-cli\",\"workload\":\"sample\",\"tier\":\"report_only\",\"measurement_index\":0,\"warmup\":false,\"qubits\":1,\"measurements\":1,\"detectors\":0,\"observables\":0,\"repeat_depth\":1,\"repeat_count\":100,\"shots\":1024,\"wall_time_ns\":2000000,\"peak_memory_bytes\":1000,\"status\":\"completed\",\"failure_reason\":null,\"stderr\":null}\n",
        "{\"case_label\":\"stim-style-surface-sample-d11-r100-b1024\",\"tool_variant\":\"rstim-interpreted\",\"workload\":\"sample\",\"tier\":\"report_only\",\"measurement_index\":0,\"warmup\":false,\"qubits\":1,\"measurements\":1,\"detectors\":0,\"observables\":0,\"repeat_depth\":1,\"repeat_count\":100,\"shots\":1024,\"wall_time_ns\":5000000,\"peak_memory_bytes\":2000,\"status\":\"completed\",\"failure_reason\":null,\"stderr\":null}\n"
    );

    let summary = summarize_jsonl_str(raw).expect("summary");
    let case = summary
        .cases
        .iter()
        .find(|case| case.case_label == "stim-style-surface-sample-d11-r100-b1024")
        .expect("public sample case");
    let comparison = case
        .rstim_compiled_vs_stim_cli_ratio
        .as_ref()
        .expect("report-only stim comparison");

    assert_eq!(comparison.ratio, None);
    assert_eq!(comparison.status, "missing_variant");
    assert_eq!(
        comparison.failure_reason.as_deref(),
        Some("missing variant rstim-compiled")
    );
}

#[test]
fn summarize_report_only_stim_comparison_surfaces_missing_stim_cli_status() {
    let raw = concat!(
        "{\"case_label\":\"stim-style-surface-sample-d11-r100-b1024\",\"tool_variant\":\"rstim-compiled\",\"workload\":\"sample\",\"tier\":\"report_only\",\"measurement_index\":0,\"warmup\":false,\"qubits\":1,\"measurements\":1,\"detectors\":0,\"observables\":0,\"repeat_depth\":1,\"repeat_count\":100,\"shots\":1024,\"wall_time_ns\":4000000,\"peak_memory_bytes\":1500,\"status\":\"completed\",\"failure_reason\":null,\"stderr\":null}\n",
        "{\"case_label\":\"stim-style-surface-sample-d11-r100-b1024\",\"tool_variant\":\"rstim-interpreted\",\"workload\":\"sample\",\"tier\":\"report_only\",\"measurement_index\":0,\"warmup\":false,\"qubits\":1,\"measurements\":1,\"detectors\":0,\"observables\":0,\"repeat_depth\":1,\"repeat_count\":100,\"shots\":1024,\"wall_time_ns\":5000000,\"peak_memory_bytes\":2000,\"status\":\"completed\",\"failure_reason\":null,\"stderr\":null}\n"
    );

    let summary = summarize_jsonl_str(raw).expect("summary");
    let case = summary
        .cases
        .iter()
        .find(|case| case.case_label == "stim-style-surface-sample-d11-r100-b1024")
        .expect("public sample case");
    let comparison = case
        .rstim_compiled_vs_stim_cli_ratio
        .as_ref()
        .expect("report-only stim comparison");

    assert_eq!(comparison.ratio, None);
    assert_eq!(comparison.status, "missing_variant");
    assert_eq!(
        comparison.failure_reason.as_deref(),
        Some("missing variant stim-cli")
    );
}

#[test]
fn summarize_report_only_stim_comparison_handles_zero_duration_stim_cli() {
    let raw = concat!(
        "{\"case_label\":\"stim-style-surface-sample-d11-r100-b1024\",\"tool_variant\":\"stim-cli\",\"workload\":\"sample\",\"tier\":\"report_only\",\"measurement_index\":0,\"warmup\":false,\"qubits\":1,\"measurements\":1,\"detectors\":0,\"observables\":0,\"repeat_depth\":1,\"repeat_count\":100,\"shots\":1024,\"wall_time_ns\":0,\"peak_memory_bytes\":1000,\"status\":\"completed\",\"failure_reason\":null,\"stderr\":null}\n",
        "{\"case_label\":\"stim-style-surface-sample-d11-r100-b1024\",\"tool_variant\":\"rstim-compiled\",\"workload\":\"sample\",\"tier\":\"report_only\",\"measurement_index\":0,\"warmup\":false,\"qubits\":1,\"measurements\":1,\"detectors\":0,\"observables\":0,\"repeat_depth\":1,\"repeat_count\":100,\"shots\":1024,\"wall_time_ns\":4000000,\"peak_memory_bytes\":1500,\"status\":\"completed\",\"failure_reason\":null,\"stderr\":null}\n"
    );

    let summary = summarize_jsonl_str(raw).expect("summary");
    let case = summary
        .cases
        .iter()
        .find(|case| case.case_label == "stim-style-surface-sample-d11-r100-b1024")
        .expect("public sample case");
    let stim = case
        .variants
        .iter()
        .find(|variant| variant.tool_variant == "stim-cli")
        .expect("stim variant");
    let comparison = case
        .rstim_compiled_vs_stim_cli_ratio
        .as_ref()
        .expect("report-only stim comparison");

    assert_eq!(stim.median_shots_per_second, Some(f64::INFINITY));
    assert_eq!(comparison.status, "completed");
    assert_eq!(comparison.ratio, Some(f64::INFINITY));
}

#[test]
fn summarize_rejects_missing_sample_shots_for_rate() {
    let err = summarize_jsonl_str(concat!(
        "{\"case_label\":\"stim-style-surface-sample-d11-r100-b1024\",\"tool_variant\":\"stim-cli\",\"workload\":\"sample\",\"tier\":\"report_only\",\"measurement_index\":0,\"warmup\":false,\"qubits\":1,\"measurements\":1,\"detectors\":0,\"observables\":0,\"repeat_depth\":1,\"repeat_count\":100,\"shots\":null,\"wall_time_ns\":2000000,\"peak_memory_bytes\":1000,\"status\":\"completed\",\"failure_reason\":null,\"stderr\":null}\n"
    ))
    .unwrap_err();

    assert!(err.contains("shots must be present for sample rate"));
    assert!(err.contains("stim-style-surface-sample-d11-r100-b1024"));
    assert!(err.contains("stim-cli"));
}

#[test]
fn summarize_rejects_zero_shot_sample_rate() {
    let raw = include_str!("fixtures/perf/stim_style_sample_zero_shots_raw.jsonl");
    let err = summarize_jsonl_str(raw).unwrap_err();
    assert!(err.contains("shots must be positive for sample rate"));
}
