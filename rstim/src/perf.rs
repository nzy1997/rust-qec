mod cases;
mod gate;
mod record;
mod report;
mod runner;
mod summary;

pub use cases::{
    benchmark_case_variants, benchmark_cases, benchmark_variants, comparison_variant_labels,
    effective_repeat_count, expected_variant_labels, PerfBenchmarkCase, PerfCaseTier,
    PerfCircuitSource, PerfComparisonKind, PerfNoiseMetadata, PerfVariant, PerfWorkload,
};
pub use gate::{evaluate_summary, PerfGateConfig, PerfGateStatus, PerfGateVerdict};
pub use record::{PerfMeasurementRecord, PerfRecord, PerfRecordStatus};
pub use report::render_markdown_report;
pub use runner::{
    benchmark_case_by_label, run_benchmark_case_to_writer, run_benchmark_suite_to_writer,
    run_case_measurements, PerfRunOptions,
};
pub use summary::{
    summarize_jsonl_str, summarize_jsonl_str_with_options, PerfCaseSummary, PerfComparisonSummary,
    PerfSummary, PerfSummaryIssue, PerfSummaryIssueKind, PerfSummaryOptions, PerfVariantSummary,
};
