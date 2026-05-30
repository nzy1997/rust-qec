use rstim::perf::{
    PerfCaseSummary, PerfComparisonSummary, PerfGateConfig, PerfGateStatus, PerfSummary,
    PerfVariantSummary, evaluate_summary,
};

fn sample_variant(tool_variant: &str, median_wall_time_ns: u128) -> PerfVariantSummary {
    PerfVariantSummary {
        tool_variant: tool_variant.to_string(),
        sample_count: 5,
        median_wall_time_ns,
        median_peak_memory_bytes: None,
    }
}

#[test]
fn gate_rejects_missing_required_variant() {
    let summary = PerfSummary {
        cases: vec![PerfCaseSummary {
            case_label: "rep-sample-d13-r13".to_string(),
            workload: "sample".to_string(),
            tier: "gating".to_string(),
            requires_compiled: true,
            requires_fallback: false,
            expected_variants: vec![
                "stim-cli".to_string(),
                "rstim-interpreted".to_string(),
                "rstim-compiled".to_string(),
            ],
            present_variants: vec!["stim-cli".to_string(), "rstim-interpreted".to_string()],
            variants: vec![sample_variant("rstim-interpreted", 100)],
            comparisons: vec![],
        }],
        issues: vec![],
    };

    let verdict = evaluate_summary(&summary, PerfGateConfig::default());
    assert_eq!(verdict.status, PerfGateStatus::ContractFailure);
    assert!(verdict.messages[0].contains("missing required variant rstim-compiled"));
}

#[test]
fn gate_rejects_sampler_regressions_above_threshold() {
    let summary = PerfSummary {
        cases: vec![PerfCaseSummary {
            case_label: "rep-sample-d13-r13".to_string(),
            workload: "sample".to_string(),
            tier: "gating".to_string(),
            requires_compiled: true,
            requires_fallback: false,
            expected_variants: vec![
                "stim-cli".to_string(),
                "rstim-interpreted".to_string(),
                "rstim-compiled".to_string(),
            ],
            present_variants: vec![
                "stim-cli".to_string(),
                "rstim-interpreted".to_string(),
                "rstim-compiled".to_string(),
            ],
            variants: vec![
                sample_variant("rstim-interpreted", 100),
                sample_variant("rstim-compiled", 111),
            ],
            comparisons: vec![PerfComparisonSummary {
                kind: "sampler_compiled_vs_interpreted".to_string(),
                lhs_variant: "rstim-compiled".to_string(),
                rhs_variant: "rstim-interpreted".to_string(),
                ratio: 1.11,
            }],
        }],
        issues: vec![],
    };

    let verdict = evaluate_summary(&summary, PerfGateConfig::default());
    assert_eq!(verdict.status, PerfGateStatus::RegressionFailure);
    assert!(verdict.messages[0].contains("exceeds threshold 1.10"));
}

#[test]
fn gate_accepts_gating_fallback_case_without_compiled_variant() {
    let summary = PerfSummary {
        cases: vec![PerfCaseSummary {
            case_label: "loss-protection-sample".to_string(),
            workload: "sample".to_string(),
            tier: "gating".to_string(),
            requires_compiled: false,
            requires_fallback: true,
            expected_variants: vec!["stim-cli".to_string(), "rstim-interpreted".to_string()],
            present_variants: vec!["stim-cli".to_string(), "rstim-interpreted".to_string()],
            variants: vec![sample_variant("rstim-interpreted", 50)],
            comparisons: vec![],
        }],
        issues: vec![],
    };

    let verdict = evaluate_summary(&summary, PerfGateConfig::default());
    assert_eq!(verdict.status, PerfGateStatus::Pass);
    assert!(verdict.messages.is_empty());
}
