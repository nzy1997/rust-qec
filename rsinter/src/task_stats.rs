use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ops::Add;

use crate::failure::{FailureKind, combine_failure_kind};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskStats {
    pub strong_id: String,
    pub decoder: String,
    pub metadata: serde_json::Value,
    pub shots: u64,
    pub errors: u64,
    pub discards: u64,
    pub seconds: f64,
    #[serde(default)]
    pub failure_kind: FailureKind,
    pub custom_counts: HashMap<String, u64>,
}

impl Add for TaskStats {
    type Output = TaskStats;
    fn add(self, rhs: TaskStats) -> TaskStats {
        let mut counts = self.custom_counts;
        for (k, v) in rhs.custom_counts {
            *counts.entry(k).or_insert(0) += v;
        }
        TaskStats {
            strong_id: self.strong_id,
            decoder: self.decoder,
            metadata: self.metadata,
            shots: self.shots + rhs.shots,
            errors: self.errors + rhs.errors,
            discards: self.discards + rhs.discards,
            seconds: self.seconds + rhs.seconds,
            failure_kind: combine_failure_kind(self.failure_kind, rhs.failure_kind),
            custom_counts: counts,
        }
    }
}
