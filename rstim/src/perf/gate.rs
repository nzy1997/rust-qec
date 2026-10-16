use std::collections::{BTreeMap, BTreeSet};

use super::{
    PerfCaseTier, PerfComparisonSummary, PerfSummary, PerfVariantSummary, benchmark_cases,
    comparison_variant_labels, expected_variant_labels,
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PerfGateConfig {
    pub sampler_ratio_threshold: f64,
    pub analyzer_ratio_threshold: f64,
}

impl Default for PerfGateConfig {
    fn default() -> Self {
        Self {
            sampler_ratio_threshold: 1.10,
            analyzer_ratio_threshold: 1.10,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PerfGateStatus {
    Pass,
    InfrastructureFailure,
    ContractFailure,
    RegressionFailure,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PerfGateVerdict {
    pub status: PerfGateStatus,
    pub messages: Vec<String>,
}

impl PerfGateVerdict {
    pub fn summary_markdown(&self) -> String {
        if self.status == PerfGateStatus::Pass {
            return "PASS".to_string();
        }

        let mut out = format!("{:?}\n", self.status);
        for message in &self.messages {
            out.push_str(&format!("- {message}\n"));
        }
        out
    }
}

pub fn evaluate_summary(summary: &PerfSummary, config: PerfGateConfig) -> PerfGateVerdict {
    let gating_cases = benchmark_cases()
        .into_iter()
        .filter(|case| case.tier == PerfCaseTier::Gating)
        .collect::<Vec<_>>();
    let gating_case_labels = gating_cases
        .iter()
        .map(|case| case.label)
        .collect::<BTreeSet<_>>();
    let gating_issue_messages = summary
        .issues
        .iter()
        .filter(|issue| gating_case_labels.contains(issue.case_label.as_str()))
        .map(|issue| format!("{:?}: {}", issue.kind, issue.message))
        .collect::<Vec<_>>();

    if !gating_issue_messages.is_empty() {
        return PerfGateVerdict {
            status: PerfGateStatus::ContractFailure,
            messages: gating_issue_messages,
        };
    }

    let mut contract_messages = Vec::new();
    let mut regression_messages = Vec::new();
    let mut summary_cases = BTreeMap::new();

    for case in &summary.cases {
        if summary_cases.insert(case.case_label.as_str(), case).is_some()
            && gating_case_labels.contains(case.case_label.as_str())
        {
            contract_messages.push(format!(
                "duplicate benchmark case summary for {}",
                case.case_label
            ));
        }
    }

    for case_def in gating_cases {
        let Some(case) = summary_cases.get(case_def.label).copied() else {
            contract_messages.push(format!(
                "missing benchmark case summary for {}",
                case_def.label
            ));
            continue;
        };

        let variant_lookup = case
            .variants
            .iter()
            .map(|variant| (variant.tool_variant.as_str(), variant))
            .collect::<BTreeMap<_, _>>();

        for expected_variant in expected_variant_labels(case_def) {
            if !variant_lookup.contains_key(expected_variant) {
                contract_messages.push(format!(
                    "{} missing required variant {}",
                    case_def.label, expected_variant
                ));
            }
        }

        let unexpected_compiled_variant = fallback_compiled_variant_label(case_def.workload);
        if case_def.requires_fallback && variant_lookup.contains_key(unexpected_compiled_variant) {
            contract_messages.push(format!(
                "{} unexpectedly produced {} on a fallback case",
                case_def.label, unexpected_compiled_variant
            ));
        }

        for comparison_kind in case_def.comparisons {
            let (lhs_variant, rhs_variant) = comparison_variant_labels(*comparison_kind);
            let Some(lhs) = variant_lookup.get(lhs_variant).copied() else {
                contract_messages.push(format!(
                    "{} missing comparison variant {} for {}",
                    case_def.label,
                    lhs_variant,
                    comparison_kind.as_str()
                ));
                continue;
            };
            let Some(rhs) = variant_lookup.get(rhs_variant).copied() else {
                contract_messages.push(format!(
                    "{} missing comparison variant {} for {}",
                    case_def.label,
                    rhs_variant,
                    comparison_kind.as_str()
                ));
                continue;
            };

            let comparison = recompute_comparison(*comparison_kind, lhs, rhs);
            let threshold = comparison_threshold(&comparison, config);
            if comparison.ratio > threshold {
                regression_messages.push(format!(
                    "{} {} ratio {:.6} exceeds threshold {:.2}",
                    case_def.label, comparison.kind, comparison.ratio, threshold
                ));
            }
        }
    }

    if !contract_messages.is_empty() {
        return PerfGateVerdict {
            status: PerfGateStatus::ContractFailure,
            messages: contract_messages,
        };
    }
    if !regression_messages.is_empty() {
        return PerfGateVerdict {
            status: PerfGateStatus::RegressionFailure,
            messages: regression_messages,
        };
    }

    PerfGateVerdict {
        status: PerfGateStatus::Pass,
        messages: Vec::new(),
    }
}

fn recompute_comparison(
    kind: super::PerfComparisonKind,
    lhs: &PerfVariantSummary,
    rhs: &PerfVariantSummary,
) -> PerfComparisonSummary {
    let ratio = if rhs.median_wall_time_ns == 0 {
        f64::INFINITY
    } else {
        lhs.median_wall_time_ns as f64 / rhs.median_wall_time_ns as f64
    };
    PerfComparisonSummary {
        kind: kind.as_str().to_string(),
        lhs_variant: lhs.tool_variant.clone(),
        rhs_variant: rhs.tool_variant.clone(),
        ratio,
    }
}

fn comparison_threshold(comparison: &PerfComparisonSummary, config: PerfGateConfig) -> f64 {
    match comparison.kind.as_str() {
        "sampler_compiled_vs_interpreted" => config.sampler_ratio_threshold,
        "analyzer_compiled_vs_flattened" => config.analyzer_ratio_threshold,
        _ => f64::INFINITY,
    }
}

fn fallback_compiled_variant_label(workload: super::PerfWorkload) -> &'static str {
    match workload {
        super::PerfWorkload::AnalyzeErrors => "rstim-analyzer-compiled",
        _ => "rstim-compiled",
    }
}
