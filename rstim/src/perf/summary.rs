use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::{
    PerfBenchmarkCase, PerfComparisonKind, PerfMeasurementRecord, PerfWorkload, benchmark_cases,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PerfSummaryIssueKind {
    DuplicateMeasurement,
    MetadataMismatch,
    MissingComparisonVariants,
    MissingBenchmarkCaseData,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PerfSummaryIssue {
    pub kind: PerfSummaryIssueKind,
    pub case_label: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PerfVariantSummary {
    pub tool_variant: String,
    pub sample_count: usize,
    pub median_wall_time_ns: u128,
    pub median_peak_memory_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PerfComparisonSummary {
    pub kind: String,
    pub lhs_variant: String,
    pub rhs_variant: String,
    pub ratio: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PerfCaseSummary {
    pub case_label: String,
    pub workload: String,
    pub tier: String,
    pub requires_compiled: bool,
    pub requires_fallback: bool,
    pub expected_variants: Vec<String>,
    pub present_variants: Vec<String>,
    pub variants: Vec<PerfVariantSummary>,
    pub comparisons: Vec<PerfComparisonSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PerfSummary {
    pub cases: Vec<PerfCaseSummary>,
    pub issues: Vec<PerfSummaryIssue>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CaseMetadata {
    workload: String,
    tier: String,
    qubits: usize,
    measurements: usize,
    detectors: usize,
    observables: usize,
    repeat_depth: usize,
    repeat_count: usize,
    shots: Option<usize>,
}

fn expected_variants(case: PerfBenchmarkCase) -> Vec<String> {
    let mut variants = vec!["stim-cli".to_string()];
    match case.workload {
        PerfWorkload::Sample | PerfWorkload::Detect => {
            variants.push("rstim-interpreted".to_string());
            if case.requires_compiled {
                variants.push("rstim-compiled".to_string());
            }
        }
        PerfWorkload::AnalyzeErrors => {
            variants.push("rstim-analyzer-flattened".to_string());
            if case.requires_compiled {
                variants.push("rstim-analyzer-compiled".to_string());
            }
        }
    }
    variants
}

fn median_u128(mut values: Vec<u128>) -> u128 {
    values.sort_unstable();
    values[values.len() / 2]
}

fn median_u64(mut values: Vec<u64>) -> u64 {
    values.sort_unstable();
    values[values.len() / 2]
}

fn case_metadata(record: &PerfMeasurementRecord) -> CaseMetadata {
    CaseMetadata {
        workload: record.workload.clone(),
        tier: record.tier.clone(),
        qubits: record.qubits,
        measurements: record.measurements,
        detectors: record.detectors,
        observables: record.observables,
        repeat_depth: record.repeat_depth,
        repeat_count: record.repeat_count,
        shots: record.shots,
    }
}

fn comparison_variants(kind: PerfComparisonKind) -> (&'static str, &'static str) {
    match kind {
        PerfComparisonKind::SamplerCompiledVsInterpreted => {
            ("rstim-compiled", "rstim-interpreted")
        }
        PerfComparisonKind::AnalyzerCompiledVsFlattened => {
            ("rstim-analyzer-compiled", "rstim-analyzer-flattened")
        }
    }
}

fn push_issue(
    issues: &mut Vec<PerfSummaryIssue>,
    kind: PerfSummaryIssueKind,
    case_label: &str,
    message: impl Into<String>,
) {
    issues.push(PerfSummaryIssue {
        kind,
        case_label: case_label.to_string(),
        message: message.into(),
    });
}

fn push_metadata_issue(
    issues: &mut Vec<PerfSummaryIssue>,
    case_label: &str,
    message: impl Into<String>,
) {
    push_issue(
        issues,
        PerfSummaryIssueKind::MetadataMismatch,
        case_label,
        message,
    );
}

pub fn summarize_jsonl_str(raw: &str) -> Result<PerfSummary, String> {
    let case_defs = benchmark_cases()
        .into_iter()
        .map(|case| (case.label.to_string(), case))
        .collect::<BTreeMap<_, _>>();

    let mut issues = Vec::new();
    let mut seen_measurements = BTreeSet::new();
    let mut grouped: BTreeMap<String, BTreeMap<String, Vec<PerfMeasurementRecord>>> =
        BTreeMap::new();
    let mut case_meta: BTreeMap<String, CaseMetadata> = BTreeMap::new();

    for line in raw.lines().filter(|line| !line.trim().is_empty()) {
        let record = PerfMeasurementRecord::from_json_line(line)?;
        let Some(case_def) = case_defs.get(&record.case_label) else {
            return Err(format!(
                "unknown benchmark case label in raw jsonl: {}",
                record.case_label
            ));
        };

        let slot = (
            record.case_label.clone(),
            record.tool_variant.clone(),
            record.measurement_index,
            record.warmup,
        );
        if !seen_measurements.insert(slot) {
            push_issue(
                &mut issues,
                PerfSummaryIssueKind::DuplicateMeasurement,
                &record.case_label,
                format!(
                    "duplicate measurement slot for case={} variant={} index={} warmup={}",
                    record.case_label,
                    record.tool_variant,
                    record.measurement_index,
                    record.warmup
                ),
            );
            continue;
        }

        if record.workload != case_def.workload.as_str() {
            push_metadata_issue(
                &mut issues,
                &record.case_label,
                format!(
                    "workload mismatch: expected {} but saw {}",
                    case_def.workload.as_str(),
                    record.workload
                ),
            );
        }
        if record.tier != case_def.tier.as_str() {
            push_metadata_issue(
                &mut issues,
                &record.case_label,
                format!(
                    "tier mismatch: expected {} but saw {}",
                    case_def.tier.as_str(),
                    record.tier
                ),
            );
        }

        let metadata = case_metadata(&record);
        if let Some(existing) = case_meta.get(&record.case_label) {
            if existing != &metadata {
                push_metadata_issue(
                    &mut issues,
                    &record.case_label,
                    "conflicting record metadata within case",
                );
            }
        } else {
            case_meta.insert(record.case_label.clone(), metadata);
        }

        grouped
            .entry(record.case_label.clone())
            .or_default()
            .entry(record.tool_variant.clone())
            .or_default()
            .push(record);
    }

    let mut cases = Vec::new();
    for case in benchmark_cases() {
        let Some(variant_records) = grouped.remove(case.label) else {
            push_issue(
                &mut issues,
                PerfSummaryIssueKind::MissingBenchmarkCaseData,
                case.label,
                format!("missing benchmark case data for {}", case.label),
            );
            continue;
        };

        let mut present_variants = variant_records.keys().cloned().collect::<Vec<_>>();
        present_variants.sort();

        let mut variants = Vec::new();
        let mut variant_lookup = BTreeMap::new();
        for (tool_variant, records) in variant_records {
            let measured = records
                .iter()
                .filter(|record| !record.warmup)
                .collect::<Vec<_>>();
            if measured.is_empty() {
                return Err(format!(
                    "missing measured records for case {} variant {}",
                    case.label, tool_variant
                ));
            }

            let median_wall_time_ns =
                median_u128(measured.iter().map(|record| record.wall_time_ns).collect());
            let memory_samples = measured
                .iter()
                .filter_map(|record| record.peak_memory_bytes)
                .collect::<Vec<_>>();
            let median_peak_memory_bytes = if memory_samples.is_empty() {
                None
            } else {
                Some(median_u64(memory_samples))
            };

            let summary = PerfVariantSummary {
                tool_variant: tool_variant.clone(),
                sample_count: measured.len(),
                median_wall_time_ns,
                median_peak_memory_bytes,
            };
            variant_lookup.insert(tool_variant, summary.clone());
            variants.push(summary);
        }
        variants.sort_by(|a, b| a.tool_variant.cmp(&b.tool_variant));

        let mut comparisons = Vec::new();
        for kind in case.comparisons {
            let (lhs_variant, rhs_variant) = comparison_variants(*kind);
            let Some(lhs) = variant_lookup.get(lhs_variant) else {
                push_issue(
                    &mut issues,
                    PerfSummaryIssueKind::MissingComparisonVariants,
                    case.label,
                    format!(
                        "missing comparison variants for {}: expected `{}` and `{}`, missing `{}`",
                        kind.as_str(),
                        lhs_variant,
                        rhs_variant,
                        lhs_variant
                    ),
                );
                continue;
            };
            let Some(rhs) = variant_lookup.get(rhs_variant) else {
                push_issue(
                    &mut issues,
                    PerfSummaryIssueKind::MissingComparisonVariants,
                    case.label,
                    format!(
                        "missing comparison variants for {}: expected `{}` and `{}`, missing `{}`",
                        kind.as_str(),
                        lhs_variant,
                        rhs_variant,
                        rhs_variant
                    ),
                );
                continue;
            };
            let ratio = if rhs.median_wall_time_ns == 0 {
                f64::INFINITY
            } else {
                lhs.median_wall_time_ns as f64 / rhs.median_wall_time_ns as f64
            };
            comparisons.push(PerfComparisonSummary {
                kind: kind.as_str().to_string(),
                lhs_variant: lhs_variant.to_string(),
                rhs_variant: rhs_variant.to_string(),
                ratio,
            });
        }

        cases.push(PerfCaseSummary {
            case_label: case.label.to_string(),
            workload: case.workload.as_str().to_string(),
            tier: case.tier.as_str().to_string(),
            requires_compiled: case.requires_compiled,
            requires_fallback: case.requires_fallback,
            expected_variants: expected_variants(case),
            present_variants,
            variants,
            comparisons,
        });
    }

    Ok(PerfSummary { cases, issues })
}
