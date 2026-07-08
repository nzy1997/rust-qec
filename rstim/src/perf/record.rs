use serde::{Deserialize, Serialize};
use serde_json::json;

/// Legacy compatibility record for older perf emitters.
///
/// New code should prefer `PerfMeasurementRecord`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PerfRecord {
    pub case_label: String,
    pub tool_variant: String,
    pub workload: String,
    pub qubits: usize,
    pub measurements: usize,
    pub detectors: usize,
    pub observables: usize,
    pub repeat_depth: usize,
    pub repeat_count: usize,
    pub shots: Option<usize>,
    pub wall_time_ns: u128,
    pub peak_memory_bytes: Option<u64>,
}

impl PerfRecord {
    pub fn to_json_line(&self) -> String {
        let mut value = json!({
            "case_label": self.case_label,
            "tool_variant": self.tool_variant,
            "workload": self.workload,
            "qubits": self.qubits,
            "measurements": self.measurements,
            "detectors": self.detectors,
            "observables": self.observables,
            "repeat_depth": self.repeat_depth,
            "repeat_count": self.repeat_count,
            "shots": self.shots,
            "wall_time_ns": self.wall_time_ns,
            "peak_memory_bytes": self.peak_memory_bytes,
        });
        if let Some(tier) = legacy_case_tier(&self.case_label) {
            value["tier"] = json!(tier);
        }
        let mut line = serde_json::to_string(&value).expect("serialize perf record");
        line.push('\n');
        line
    }
}

fn legacy_case_tier(case_label: &str) -> Option<&'static str> {
    match case_label {
        "rep-sample-d13-r13"
        | "surface-detect-d13-r13"
        | "repeat-analyze-large"
        | "loss-protection-sample" => Some("gating"),
        "repeat-analyze-stress-report" => Some("report_only"),
        _ => None,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PerfMeasurementRecord {
    pub case_label: String,
    pub tool_variant: String,
    pub workload: String,
    pub tier: String,
    pub measurement_index: usize,
    pub warmup: bool,
    pub qubits: usize,
    pub measurements: usize,
    pub detectors: usize,
    pub observables: usize,
    pub repeat_depth: usize,
    pub repeat_count: usize,
    pub shots: Option<usize>,
    pub wall_time_ns: u128,
    pub peak_memory_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample_output_mode: Option<PerfSampleOutputMode>,
    #[serde(default = "default_perf_record_status")]
    pub status: PerfRecordStatus,
    #[serde(default)]
    pub failure_reason: Option<String>,
    #[serde(default)]
    pub stderr: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PerfSampleOutputMode {
    Full,
    MeasurementsOnly,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PerfRecordStatus {
    Completed,
    ToolFailed,
    TimedOut,
    MissingVariant,
}

impl PerfRecordStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            PerfRecordStatus::Completed => "completed",
            PerfRecordStatus::ToolFailed => "tool_failed",
            PerfRecordStatus::TimedOut => "timed_out",
            PerfRecordStatus::MissingVariant => "missing_variant",
        }
    }
}

fn default_perf_record_status() -> PerfRecordStatus {
    PerfRecordStatus::Completed
}

impl PerfMeasurementRecord {
    pub fn to_json_line(&self) -> String {
        let mut line = serde_json::to_string(self).expect("serialize perf measurement record");
        line.push('\n');
        line
    }

    pub fn from_json_line(line: &str) -> Result<Self, String> {
        serde_json::from_str(line)
            .map_err(|e| format!("failed to parse perf measurement record: {e}"))
    }
}
