use rstim::compiled::{choose_analyzer_path, compile_circuit, CompiledPathDecision};
use rstim::parser::parse_lines;
use rstim::perf::{
    benchmark_case_variants, benchmark_cases, benchmark_variants, effective_repeat_count,
    PerfBenchmarkCase, PerfCaseTier, PerfCircuitSource, PerfMeasurementRecord, PerfVariant,
    PerfWorkload,
};
use serde_json::Value;

#[test]
fn benchmark_cases_cover_sampling_detect_and_repeat_analysis() {
    let cases = benchmark_cases();

    assert!(
        cases
            .iter()
            .any(|case| case.workload == PerfWorkload::Sample),
        "expected at least one sample benchmark case"
    );
    assert!(
        cases
            .iter()
            .any(|case| case.workload == PerfWorkload::Detect),
        "expected at least one detect benchmark case"
    );
    assert!(
        cases
            .iter()
            .any(|case| case.workload == PerfWorkload::AnalyzeErrors),
        "expected at least one analyze_errors benchmark case"
    );
    assert!(
        cases.iter().any(|case| case.label.contains("repeat")),
        "expected at least one repeat-focused benchmark case"
    );
}

#[test]
fn benchmark_cases_define_gating_and_report_only_contracts() {
    let cases = benchmark_cases();

    assert!(
        cases.iter().any(|case| case.tier == PerfCaseTier::Gating),
        "expected at least one gating case"
    );
    assert!(
        cases
            .iter()
            .any(|case| case.tier == PerfCaseTier::ReportOnly),
        "expected at least one report-only case"
    );

    let loss_case = cases
        .iter()
        .find(|case| case.label == "loss-protection-sample")
        .expect("loss protection case");
    assert_eq!(loss_case.tier, PerfCaseTier::Gating);
    assert!(loss_case.requires_fallback);
    assert!(!loss_case.requires_compiled);
}

#[test]
fn perf_measurement_record_json_line_contains_round_metadata() {
    let record = PerfMeasurementRecord {
        case_label: "rep-sample-d13-r13".to_string(),
        tool_variant: PerfVariant::RstimCompiled.label().to_string(),
        workload: PerfWorkload::Sample.as_str().to_string(),
        tier: PerfCaseTier::Gating.as_str().to_string(),
        measurement_index: 3,
        warmup: false,
        qubits: 25,
        measurements: 48,
        detectors: 0,
        observables: 0,
        repeat_depth: 1,
        repeat_count: 13,
        shots: Some(20_000),
        wall_time_ns: 456_789,
        peak_memory_bytes: Some(8_192),
    };

    let line = record.to_json_line();
    let json: Value = serde_json::from_str(line.trim_end()).unwrap();

    assert_eq!(json["tier"], "gating");
    assert_eq!(json["measurement_index"], 3);
    assert_eq!(json["warmup"], false);
    assert_eq!(json["peak_memory_bytes"], 8_192);
}

#[test]
fn benchmark_variants_cover_stim_and_both_rstim_backends() {
    let variants = benchmark_variants();

    assert!(variants.contains(&PerfVariant::StimCli));
    assert!(variants.contains(&PerfVariant::RstimInterpreted));
    assert!(variants.contains(&PerfVariant::RstimCompiled));
    assert!(variants.contains(&PerfVariant::RstimAnalyzerFlattened));
    assert!(variants.contains(&PerfVariant::RstimAnalyzerCompiled));
}

#[test]
fn perf_workload_and_variant_labels_are_stable() {
    assert_eq!(PerfWorkload::Sample.as_str(), "sample");
    assert_eq!(PerfWorkload::Detect.as_str(), "detect");
    assert_eq!(PerfWorkload::AnalyzeErrors.as_str(), "analyze_errors");

    assert_eq!(PerfVariant::StimCli.label(), "stim-cli");
    assert_eq!(PerfVariant::RstimInterpreted.label(), "rstim-interpreted");
    assert_eq!(PerfVariant::RstimCompiled.label(), "rstim-compiled");
    assert_eq!(
        PerfVariant::RstimAnalyzerFlattened.label(),
        "rstim-analyzer-flattened"
    );
    assert_eq!(
        PerfVariant::RstimAnalyzerCompiled.label(),
        "rstim-analyzer-compiled"
    );
}

#[test]
fn benchmark_cases_include_a_compiled_analyzer_compatible_repeat_case() {
    let cases = benchmark_cases();

    let compatible = cases
        .iter()
        .filter(|case| case.workload == PerfWorkload::AnalyzeErrors)
        .any(|case| match case.source {
            rstim::perf::PerfCircuitSource::Inline { text } => {
                let instrs = parse_lines(text).unwrap();
                let compiled = compile_circuit(&instrs).unwrap();
                choose_analyzer_path(&compiled) == CompiledPathDecision::FastPath
            }
            rstim::perf::PerfCircuitSource::Generator { .. } => false,
        });

    assert!(
        compatible,
        "expected at least one analyze_errors benchmark case to support the compiled analyzer path"
    );
}

#[test]
fn benchmark_case_variants_add_compiled_backends_for_supported_paths() {
    let sample_instrs = parse_lines("X_ERROR(0.001) 0\nM 0\n").unwrap();
    let sample_case = PerfBenchmarkCase {
        label: "inline-sample",
        workload: PerfWorkload::Sample,
        source: PerfCircuitSource::Inline {
            text: "X_ERROR(0.001) 0\nM 0\n",
        },
        shots: Some(32),
        tier: PerfCaseTier::Gating,
        requires_compiled: true,
        requires_fallback: false,
        comparisons: &[],
    };
    let analyze_instrs =
        parse_lines("REPEAT 8 {\n  X_ERROR(0.001) 0\n  MR 0\n  DETECTOR rec[-1]\n}\n").unwrap();
    let analyze_case = PerfBenchmarkCase {
        label: "inline-analyze",
        workload: PerfWorkload::AnalyzeErrors,
        source: PerfCircuitSource::Inline {
            text: "REPEAT 8 {\n  X_ERROR(0.001) 0\n  MR 0\n  DETECTOR rec[-1]\n}\n",
        },
        shots: None,
        tier: PerfCaseTier::Gating,
        requires_compiled: true,
        requires_fallback: false,
        comparisons: &[],
    };

    assert_eq!(
        benchmark_case_variants(sample_case, &sample_instrs).unwrap(),
        vec![
            PerfVariant::StimCli,
            PerfVariant::RstimInterpreted,
            PerfVariant::RstimCompiled,
        ]
    );
    assert_eq!(
        benchmark_case_variants(analyze_case, &analyze_instrs).unwrap(),
        vec![
            PerfVariant::StimCli,
            PerfVariant::RstimAnalyzerFlattened,
            PerfVariant::RstimAnalyzerCompiled,
        ]
    );
}

#[test]
fn loss_protection_case_skips_compiled_sampler_variant() {
    let case = benchmark_cases()
        .into_iter()
        .find(|case| case.label == "loss-protection-sample")
        .expect("loss protection case");
    let text = match case.source {
        rstim::perf::PerfCircuitSource::Inline { text } => text,
        _ => panic!("loss protection case should be inline"),
    };
    let instrs = parse_lines(text).unwrap();

    let variants = benchmark_case_variants(case, &instrs).unwrap();

    assert!(variants.contains(&PerfVariant::StimCli));
    assert!(variants.contains(&PerfVariant::RstimInterpreted));
    assert!(!variants.contains(&PerfVariant::RstimCompiled));
}

#[test]
fn effective_repeat_count_scales_nested_repeat_regions() {
    let instrs = parse_lines(
        "H 0\nREPEAT 2 {\n  REPEAT 3 {\n    M 0\n  }\n  REPEAT 5 {\n    M 1\n  }\n}\nREPEAT 7 {\n  M 2\n}\n",
    )
    .unwrap();

    assert_eq!(effective_repeat_count(&instrs), 25);
}
