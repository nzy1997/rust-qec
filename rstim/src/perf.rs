mod cases;
mod record;
mod report;
mod runner;
mod summary;

pub use cases::{
    benchmark_case_variants, benchmark_cases, benchmark_variants, effective_repeat_count,
    PerfBenchmarkCase, PerfCaseTier, PerfCircuitSource, PerfComparisonKind, PerfVariant,
    PerfWorkload,
};
pub use record::{PerfMeasurementRecord, PerfRecord};
pub use report::render_markdown_report;
pub use runner::{PerfRunOptions, run_benchmark_suite_to_writer, run_case_measurements};
pub use summary::{
    PerfCaseSummary, PerfComparisonSummary, PerfSummary, PerfSummaryIssue,
    PerfSummaryIssueKind, PerfVariantSummary, summarize_jsonl_str,
};
