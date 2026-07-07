mod cases;
mod gate;
mod record;
mod report;
mod runner;
mod summary;

pub use cases::{
    PerfBenchmarkCase, PerfCaseTier, PerfCircuitSource, PerfComparisonKind, PerfNoiseMetadata,
    PerfVariant, PerfWorkload, benchmark_case_variants, benchmark_cases, benchmark_variants,
    comparison_variant_labels, effective_repeat_count, expected_variant_labels,
};
pub use gate::{PerfGateConfig, PerfGateStatus, PerfGateVerdict, evaluate_summary};
pub use record::{PerfMeasurementRecord, PerfRecord};
pub use report::render_markdown_report;
pub use runner::{PerfRunOptions, run_benchmark_suite_to_writer, run_case_measurements};
pub use summary::{
    PerfCaseSummary, PerfComparisonSummary, PerfSummary, PerfSummaryIssue, PerfSummaryIssueKind,
    PerfVariantSummary, summarize_jsonl_str,
};
