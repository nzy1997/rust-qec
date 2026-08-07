mod cases;
mod gate;
mod record;
mod report;
mod runner;
mod summary;

pub use cases::{
    PerfAtomLossVariant, PerfBenchmarkCase, PerfCaseTier, PerfCircuitSource, PerfComparisonKind, PerfNoiseMetadata,
    PerfVariant, PerfWorkload, benchmark_case_variants, benchmark_cases, benchmark_variants,
    comparison_variant_labels, effective_repeat_count, expected_variant_labels,
};
pub use gate::{PerfGateConfig, PerfGateStatus, PerfGateVerdict, evaluate_summary};
pub use record::{PerfMeasurementRecord, PerfRecord, PerfRecordStatus, PerfSampleOutputMode};
pub use report::render_markdown_report;
pub use runner::{
    PerfRunOptions, benchmark_case_by_label, run_benchmark_case_to_writer,
    run_benchmark_suite_to_writer, run_case_measurements,
};
pub use summary::{
    PerfCaseSummary, PerfComparisonSummary, PerfReportOnlyComparisonSummary, PerfSummary,
    PerfSummaryIssue, PerfSummaryIssueKind, PerfSummaryOptions, PerfVariantSummary,
    summarize_jsonl_str, summarize_jsonl_str_with_options,
};
