use serde::{Deserialize, Serialize};

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
