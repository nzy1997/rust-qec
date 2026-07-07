use rstim::codegen::{repetition_code_memory, rotated_memory_x, NoiseParams};
use rstim::compiled::{choose_analyzer_path, compile_circuit, CompiledPathDecision};
use rstim::parser::parse_lines;
use rstim::perf::{
    benchmark_case_variants, benchmark_cases, benchmark_variants, effective_repeat_count,
    PerfBenchmarkCase, PerfCaseTier, PerfCircuitSource, PerfComparisonKind, PerfMeasurementRecord,
    PerfRecord, PerfRecordStatus, PerfVariant, PerfWorkload,
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

    let expectations = [
        ("rep-sample-d13-r13", PerfCaseTier::Gating, true, false),
        ("surface-detect-d13-r13", PerfCaseTier::Gating, true, false),
        ("repeat-analyze-large", PerfCaseTier::Gating, true, false),
        ("loss-protection-sample", PerfCaseTier::Gating, false, true),
        (
            "repeat-analyze-stress-report",
            PerfCaseTier::ReportOnly,
            true,
            false,
        ),
        (
            "stim-style-surface-sample-d11-r100-b1024",
            PerfCaseTier::ReportOnly,
            true,
            false,
        ),
    ];

    for (label, tier, requires_compiled, requires_fallback) in expectations {
        let case = cases
            .iter()
            .find(|case| case.label == label)
            .unwrap_or_else(|| panic!("missing benchmark case {label}"));
        assert_eq!(case.tier, tier, "unexpected tier for {label}");
        assert_eq!(
            case.requires_compiled, requires_compiled,
            "unexpected requires_compiled for {label}"
        );
        assert_eq!(
            case.requires_fallback, requires_fallback,
            "unexpected requires_fallback for {label}"
        );
    }
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
        status: PerfRecordStatus::Completed,
        failure_reason: None,
        stderr: None,
    };

    let line = record.to_json_line();
    assert!(line.ends_with('\n'));
    let json: Value = serde_json::from_str(line.trim_end()).unwrap();
    let parsed = PerfMeasurementRecord::from_json_line(line.trim_end()).unwrap();

    assert_eq!(json["case_label"], "rep-sample-d13-r13");
    assert_eq!(json["tool_variant"], "rstim-compiled");
    assert_eq!(json["workload"], "sample");
    assert_eq!(json["tier"], "gating");
    assert_eq!(json["measurement_index"], 3);
    assert_eq!(json["warmup"], false);
    assert_eq!(json["qubits"], 25);
    assert_eq!(json["measurements"], 48);
    assert_eq!(json["detectors"], 0);
    assert_eq!(json["observables"], 0);
    assert_eq!(json["repeat_depth"], 1);
    assert_eq!(json["repeat_count"], 13);
    assert_eq!(json["shots"], 20_000);
    assert_eq!(json["wall_time_ns"], 456_789);
    assert_eq!(json["peak_memory_bytes"], 8_192);
    assert_eq!(parsed, record);
}

#[test]
fn perf_measurement_record_json_line_contains_status_and_failure_context() {
    let record = PerfMeasurementRecord {
        case_label: "loss-protection-sample".to_string(),
        tool_variant: PerfVariant::StimCli.label().to_string(),
        workload: PerfWorkload::Sample.as_str().to_string(),
        tier: PerfCaseTier::Gating.as_str().to_string(),
        measurement_index: 0,
        warmup: false,
        qubits: 1,
        measurements: 1,
        detectors: 1,
        observables: 0,
        repeat_depth: 1,
        repeat_count: 0,
        shots: Some(128),
        wall_time_ns: 0,
        peak_memory_bytes: None,
        status: PerfRecordStatus::ToolFailed,
        failure_reason: Some("stim failed: boom".to_string()),
        stderr: Some("boom\n".to_string()),
    };

    let line = record.to_json_line();
    let json: serde_json::Value = serde_json::from_str(line.trim_end()).unwrap();
    let parsed = PerfMeasurementRecord::from_json_line(line.trim_end()).unwrap();

    assert_eq!(json["status"], "tool_failed");
    assert_eq!(json["failure_reason"], "stim failed: boom");
    assert_eq!(json["stderr"], "boom\n");
    assert_eq!(parsed, record);
}

#[test]
fn perf_record_status_labels_are_stable() {
    assert_eq!(PerfRecordStatus::Completed.as_str(), "completed");
    assert_eq!(PerfRecordStatus::ToolFailed.as_str(), "tool_failed");
    assert_eq!(PerfRecordStatus::TimedOut.as_str(), "timed_out");
    assert_eq!(PerfRecordStatus::MissingVariant.as_str(), "missing_variant");
}

#[test]
fn perf_measurement_record_deserializes_legacy_rows_as_completed() {
    let parsed = PerfMeasurementRecord::from_json_line(
        "{\"case_label\":\"loss-protection-sample\",\"tool_variant\":\"stim-cli\",\"workload\":\"sample\",\"tier\":\"gating\",\"measurement_index\":0,\"warmup\":false,\"qubits\":1,\"measurements\":1,\"detectors\":1,\"observables\":0,\"repeat_depth\":1,\"repeat_count\":0,\"shots\":128,\"wall_time_ns\":80,\"peak_memory_bytes\":128}"
    )
    .unwrap();

    assert_eq!(parsed.status, PerfRecordStatus::Completed);
    assert_eq!(parsed.failure_reason, None);
    assert_eq!(parsed.stderr, None);
}

#[test]
fn legacy_perf_record_json_line_infers_tier_for_known_benchmark_case() {
    let record = PerfRecord {
        case_label: "repeat-analyze-stress-report".to_string(),
        tool_variant: PerfVariant::RstimAnalyzerCompiled.label().to_string(),
        workload: PerfWorkload::AnalyzeErrors.as_str().to_string(),
        qubits: 1,
        measurements: 1,
        detectors: 1,
        observables: 0,
        repeat_depth: 1,
        repeat_count: 8192,
        shots: None,
        wall_time_ns: 123,
        peak_memory_bytes: Some(456),
    };

    let line = record.to_json_line();
    let json: Value = serde_json::from_str(line.trim_end()).unwrap();

    assert!(line.ends_with('\n'));
    assert_eq!(json["case_label"], "repeat-analyze-stress-report");
    assert_eq!(json["tier"], "report_only");
}

#[test]
fn benchmark_case_variants_and_comparisons_match_declared_contracts() {
    let expectations = [
        (
            "rep-sample-d13-r13",
            vec![
                PerfVariant::StimCli,
                PerfVariant::RstimInterpreted,
                PerfVariant::RstimCompiled,
            ],
            vec![PerfComparisonKind::SamplerCompiledVsInterpreted],
        ),
        (
            "surface-detect-d13-r13",
            vec![
                PerfVariant::StimCli,
                PerfVariant::RstimInterpreted,
                PerfVariant::RstimCompiled,
            ],
            vec![PerfComparisonKind::SamplerCompiledVsInterpreted],
        ),
        (
            "repeat-analyze-large",
            vec![
                PerfVariant::StimCli,
                PerfVariant::RstimAnalyzerFlattened,
                PerfVariant::RstimAnalyzerCompiled,
            ],
            vec![PerfComparisonKind::AnalyzerCompiledVsFlattened],
        ),
        (
            "loss-protection-sample",
            vec![PerfVariant::StimCli, PerfVariant::RstimInterpreted],
            Vec::new(),
        ),
        (
            "repeat-analyze-stress-report",
            vec![
                PerfVariant::StimCli,
                PerfVariant::RstimAnalyzerFlattened,
                PerfVariant::RstimAnalyzerCompiled,
            ],
            vec![PerfComparisonKind::AnalyzerCompiledVsFlattened],
        ),
        (
            "stim-style-surface-sample-d11-r100-b1024",
            vec![
                PerfVariant::StimCli,
                PerfVariant::RstimInterpreted,
                PerfVariant::RstimCompiled,
            ],
            vec![PerfComparisonKind::SamplerCompiledVsInterpreted],
        ),
    ];

    for (label, expected_variants, expected_comparisons) in expectations {
        let case = benchmark_cases()
            .into_iter()
            .find(|case| case.label == label)
            .unwrap_or_else(|| panic!("missing benchmark case {label}"));
        let instrs = match case.source {
            PerfCircuitSource::Generator {
                code,
                task,
                distance,
                rounds,
                noise,
            } => match (code, task) {
                ("repetition_code", "memory") => repetition_code_memory(distance, rounds, noise),
                ("surface_code", "rotated_memory_x") => rotated_memory_x(distance, rounds, noise),
                _ => panic!("unsupported generator source for {label}"),
            },
            PerfCircuitSource::Fixture {
                canonical_input_path,
                ..
            } => parse_lines(
                &std::fs::read_to_string(std::path::Path::new("..").join(canonical_input_path))
                    .unwrap(),
            )
            .unwrap(),
            PerfCircuitSource::Inline { text } => parse_lines(text).unwrap(),
        };

        let variants = benchmark_case_variants(case, &instrs).unwrap();
        assert_eq!(
            variants, expected_variants,
            "unexpected variants for {label}"
        );
        assert_eq!(
            case.comparisons,
            expected_comparisons.as_slice(),
            "unexpected comparisons for {label}"
        );
    }
}

#[test]
fn benchmark_cases_include_stim_style_surface_sample_contract() {
    let case = benchmark_cases()
        .into_iter()
        .find(|case| case.label == "stim-style-surface-sample-d11-r100-b1024")
        .expect("stim-style-surface sample perf case");

    assert_eq!(case.workload, PerfWorkload::Sample);
    assert_eq!(case.shots, Some(1024));
    assert_eq!(case.tier, PerfCaseTier::ReportOnly);
    assert!(case.requires_compiled);
    assert!(!case.requires_fallback);
    assert_eq!(
        case.comparisons,
        [PerfComparisonKind::SamplerCompiledVsInterpreted].as_slice()
    );

    let (case_id, canonical_input_path, noise) = match case.source {
        PerfCircuitSource::Fixture {
            case_id,
            canonical_input_path,
            noise,
        } => (case_id, canonical_input_path, noise),
        PerfCircuitSource::Generator { .. } => {
            panic!(
                "Stim-style surface sample must use checked Stim fixture, not regenerated rstim source"
            )
        }
        PerfCircuitSource::Inline { .. } => {
            panic!("Stim-style surface sample must point at checked Stim fixture")
        }
    };

    assert_eq!(case_id, "stim_surface_d11_r100");
    assert_eq!(
        canonical_input_path,
        "benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim"
    );
    assert_eq!(noise.after_clifford_depolarization, 0.001);
    assert_eq!(noise.after_reset_flip_probability, 0.001);
    assert_eq!(noise.before_measure_flip_probability, 0.001);
    assert_eq!(noise.before_round_data_depolarization, 0.0);

    let uniform = NoiseParams::uniform(0.001);
    assert_ne!(
        noise.before_round_data_depolarization, uniform.before_round_data_depolarization,
        "uniform noise would enable before_round_data_depolarization"
    );

    let fixture_text =
        std::fs::read_to_string(std::path::Path::new("..").join(canonical_input_path))
            .expect("checked Stim fixture");
    let instrs = parse_lines(&fixture_text).expect("fixture parses");
    assert_eq!(
        benchmark_case_variants(case, &instrs).unwrap(),
        vec![
            PerfVariant::StimCli,
            PerfVariant::RstimInterpreted,
            PerfVariant::RstimCompiled,
        ]
    );
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
            rstim::perf::PerfCircuitSource::Fixture { .. } => false,
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
