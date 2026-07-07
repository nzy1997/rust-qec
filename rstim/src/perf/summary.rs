use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::{
    PerfMeasurementRecord, PerfRecordStatus, PerfWorkload, benchmark_cases,
    comparison_variant_labels, expected_variant_labels,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PerfVariantSummary {
    pub tool_variant: String,
    pub sample_count: usize,
    pub median_wall_time_ns: u128,
    #[serde(default)]
    pub median_shots_per_second: Option<f64>,
    pub median_peak_memory_bytes: Option<u64>,
    #[serde(default = "default_perf_variant_summary_status")]
    pub status: String,
    pub failure_reason: Option<String>,
    pub stderr: Option<String>,
}

fn default_perf_variant_summary_status() -> String {
    PerfRecordStatus::Completed.as_str().to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PerfComparisonSummary {
    pub kind: String,
    pub lhs_variant: String,
    pub rhs_variant: String,
    pub ratio: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PerfReportOnlyComparisonSummary {
    pub kind: String,
    pub lhs_variant: String,
    pub rhs_variant: String,
    pub ratio: Option<f64>,
    pub status: String,
    pub failure_reason: Option<String>,
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
    #[serde(default)]
    pub rstim_compiled_vs_stim_cli_ratio: Option<PerfReportOnlyComparisonSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PerfSummary {
    pub cases: Vec<PerfCaseSummary>,
    pub issues: Vec<PerfSummaryIssue>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PerfSummaryOptions {
    pub case_label: Option<String>,
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

fn median_u128(mut values: Vec<u128>) -> u128 {
    values.sort_unstable();
    values[values.len() / 2]
}

fn median_u64(mut values: Vec<u64>) -> u64 {
    values.sort_unstable();
    values[values.len() / 2]
}

fn sample_rate_for_variant(
    case_label: &str,
    tool_variant: &str,
    measured: &[&PerfMeasurementRecord],
    median_wall_time_ns: u128,
) -> Result<Option<f64>, String> {
    if measured.first().map(|record| record.workload.as_str())
        != Some(PerfWorkload::Sample.as_str())
    {
        return Ok(None);
    }
    let shots = measured[0].shots.ok_or_else(|| {
        format!(
            "shots must be present for sample rate for case {case_label} variant {tool_variant}"
        )
    })?;
    if shots == 0 {
        return Err(format!(
            "shots must be positive for sample rate for case {case_label} variant {tool_variant}"
        ));
    }
    if median_wall_time_ns == 0 {
        return Ok(Some(f64::INFINITY));
    }
    Ok(Some(
        shots as f64 * 1_000_000_000.0 / median_wall_time_ns as f64,
    ))
}

fn unavailable_stim_comparison(
    lhs_variant: &str,
    rhs_variant: &str,
    status: &str,
    failure_reason: Option<String>,
) -> PerfReportOnlyComparisonSummary {
    PerfReportOnlyComparisonSummary {
        kind: "rstim_compiled_vs_stim_cli".to_string(),
        lhs_variant: lhs_variant.to_string(),
        rhs_variant: rhs_variant.to_string(),
        ratio: None,
        status: status.to_string(),
        failure_reason,
    }
}

fn report_only_stim_comparison(
    case: super::PerfBenchmarkCase,
    variant_lookup: &BTreeMap<String, PerfVariantSummary>,
) -> Option<PerfReportOnlyComparisonSummary> {
    if case.workload != PerfWorkload::Sample || !case.requires_compiled {
        return None;
    }

    let lhs_variant = "rstim-compiled";
    let rhs_variant = "stim-cli";
    let Some(lhs) = variant_lookup.get(lhs_variant) else {
        return Some(unavailable_stim_comparison(
            lhs_variant,
            rhs_variant,
            PerfRecordStatus::MissingVariant.as_str(),
            Some(format!("missing variant {lhs_variant}")),
        ));
    };
    let Some(rhs) = variant_lookup.get(rhs_variant) else {
        return Some(unavailable_stim_comparison(
            lhs_variant,
            rhs_variant,
            PerfRecordStatus::MissingVariant.as_str(),
            Some(format!("missing variant {rhs_variant}")),
        ));
    };

    for variant in [lhs, rhs] {
        if variant.status != PerfRecordStatus::Completed.as_str() || variant.sample_count == 0 {
            return Some(unavailable_stim_comparison(
                lhs_variant,
                rhs_variant,
                &variant.status,
                variant.failure_reason.clone(),
            ));
        }
    }

    let ratio = if rhs.median_wall_time_ns == 0 {
        f64::INFINITY
    } else {
        lhs.median_wall_time_ns as f64 / rhs.median_wall_time_ns as f64
    };
    Some(PerfReportOnlyComparisonSummary {
        kind: "rstim_compiled_vs_stim_cli".to_string(),
        lhs_variant: lhs_variant.to_string(),
        rhs_variant: rhs_variant.to_string(),
        ratio: Some(ratio),
        status: PerfRecordStatus::Completed.as_str().to_string(),
        failure_reason: None,
    })
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
    summarize_jsonl_str_with_options(raw, PerfSummaryOptions::default())
}

pub fn summarize_jsonl_str_with_options(
    raw: &str,
    options: PerfSummaryOptions,
) -> Result<PerfSummary, String> {
    let selected_label = options.case_label.as_deref();
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
        if let Some(label) = selected_label {
            if record.case_label != label {
                continue;
            }
        }
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
                    record.case_label, record.tool_variant, record.measurement_index, record.warmup
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

    let cases_to_summarize = match selected_label {
        Some(label) => {
            let case = case_defs
                .get(label)
                .copied()
                .ok_or_else(|| format!("unknown benchmark case: {label}"))?;
            vec![case]
        }
        None => benchmark_cases(),
    };

    let mut cases = Vec::new();
    for case in cases_to_summarize {
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
                .filter(|record| !record.warmup && record.status == PerfRecordStatus::Completed)
                .collect::<Vec<_>>();
            let summary = if measured.is_empty() {
                if let Some(failed) = records
                    .iter()
                    .find(|record| !record.warmup && record.status != PerfRecordStatus::Completed)
                {
                    PerfVariantSummary {
                        tool_variant: tool_variant.clone(),
                        sample_count: 0,
                        median_wall_time_ns: 0,
                        median_shots_per_second: None,
                        median_peak_memory_bytes: None,
                        status: failed.status.as_str().to_string(),
                        failure_reason: failed.failure_reason.clone(),
                        stderr: failed.stderr.clone(),
                    }
                } else {
                    return Err(format!(
                        "missing measured records for case {} variant {}",
                        case.label, tool_variant
                    ));
                }
            } else {
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
                let median_shots_per_second = sample_rate_for_variant(
                    case.label,
                    &tool_variant,
                    &measured,
                    median_wall_time_ns,
                )?;

                PerfVariantSummary {
                    tool_variant: tool_variant.clone(),
                    sample_count: measured.len(),
                    median_wall_time_ns,
                    median_shots_per_second,
                    median_peak_memory_bytes,
                    status: PerfRecordStatus::Completed.as_str().to_string(),
                    failure_reason: None,
                    stderr: None,
                }
            };
            variant_lookup.insert(tool_variant, summary.clone());
            variants.push(summary);
        }
        variants.sort_by(|a, b| a.tool_variant.cmp(&b.tool_variant));

        let mut comparisons = Vec::new();
        for kind in case.comparisons {
            let (lhs_variant, rhs_variant) = comparison_variant_labels(*kind);
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
            if lhs.status != PerfRecordStatus::Completed.as_str() || lhs.sample_count == 0 {
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
            }
            if rhs.status != PerfRecordStatus::Completed.as_str() || rhs.sample_count == 0 {
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
            }
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
        let rstim_compiled_vs_stim_cli_ratio = report_only_stim_comparison(case, &variant_lookup);

        cases.push(PerfCaseSummary {
            case_label: case.label.to_string(),
            workload: case.workload.as_str().to_string(),
            tier: case.tier.as_str().to_string(),
            requires_compiled: case.requires_compiled,
            requires_fallback: case.requires_fallback,
            expected_variants: expected_variant_labels(case)
                .into_iter()
                .map(ToString::to_string)
                .collect(),
            present_variants,
            variants,
            comparisons,
            rstim_compiled_vs_stim_cli_ratio,
        });
    }

    Ok(PerfSummary { cases, issues })
}
