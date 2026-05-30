use super::{PerfCaseTier, PerfSummary};

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
    if !summary.issues.is_empty() {
        return PerfGateVerdict {
            status: PerfGateStatus::ContractFailure,
            messages: summary
                .issues
                .iter()
                .map(|issue| format!("{:?}: {}", issue.kind, issue.message))
                .collect(),
        };
    }

    let mut contract_messages = Vec::new();
    let mut regression_messages = Vec::new();

    for case in &summary.cases {
        if case.tier != PerfCaseTier::Gating.as_str() {
            continue;
        }

        for expected_variant in &case.expected_variants {
            if !case.present_variants.iter().any(|present| present == expected_variant) {
                contract_messages.push(format!(
                    "{} missing required variant {}",
                    case.case_label, expected_variant
                ));
            }
        }

        if case.requires_fallback
            && case
                .present_variants
                .iter()
                .any(|variant| variant == "rstim-compiled")
        {
            contract_messages.push(format!(
                "{} unexpectedly produced rstim-compiled on a fallback case",
                case.case_label
            ));
        }

        for comparison in &case.comparisons {
            let threshold = match comparison.kind.as_str() {
                "sampler_compiled_vs_interpreted" => config.sampler_ratio_threshold,
                "analyzer_compiled_vs_flattened" => config.analyzer_ratio_threshold,
                _ => continue,
            };

            if comparison.ratio > threshold {
                regression_messages.push(format!(
                    "{} {} ratio {:.6} exceeds threshold {:.2}",
                    case.case_label, comparison.kind, comparison.ratio, threshold
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
