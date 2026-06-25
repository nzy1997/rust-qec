use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::collections::btree_map::Entry;

use serde_json::{Number, Value};

use crate::bench::result::{
    BenchmarkResultRow, MetricMap, case_summary_additive_keys, stable_case_summary,
};

const ADDITIVE_METRICS: [&str; 5] = [
    "shots_used",
    "logical_errors",
    "compile_us",
    "total_decode_us",
    "wall_seconds",
];
const DERIVED_METRICS: [&str; 2] = ["logical_error_rate", "decode_us_per_shot"];

pub fn merge_result_rows(
    row_sets: Vec<Vec<BenchmarkResultRow>>,
) -> Result<Vec<BenchmarkResultRow>, String> {
    let mut rows_by_identity = BTreeMap::new();
    for row in row_sets.into_iter().flatten() {
        let identity = row.identity()?;
        match rows_by_identity.entry(identity) {
            Entry::Vacant(entry) => {
                entry.insert(row);
            }
            Entry::Occupied(mut entry) => {
                let identity = entry.key().clone();
                merge_row_into(&identity, entry.get_mut(), row)?;
            }
        }
    }

    let mut rows: Vec<BenchmarkResultRow> = rows_by_identity.into_values().collect();
    rows.sort_by(compare_rows);
    Ok(rows)
}

fn compare_rows(a: &BenchmarkResultRow, b: &BenchmarkResultRow) -> Ordering {
    let a_distance = a
        .params
        .get("distance")
        .and_then(|value| value.as_i64())
        .unwrap_or_default();
    let b_distance = b
        .params
        .get("distance")
        .and_then(|value| value.as_i64())
        .unwrap_or_default();
    let a_p = a
        .params
        .get("p")
        .and_then(|value| value.as_f64().or_else(|| value.as_i64().map(|n| n as f64)))
        .unwrap_or_default();
    let b_p = b
        .params
        .get("p")
        .and_then(|value| value.as_f64().or_else(|| value.as_i64().map(|n| n as f64)))
        .unwrap_or_default();
    a.runner
        .cmp(&b.runner)
        .then(a_distance.cmp(&b_distance))
        .then(a_p.partial_cmp(&b_p).unwrap_or(Ordering::Equal))
}

fn merge_row_into(
    identity: &str,
    base: &mut BenchmarkResultRow,
    incoming: BenchmarkResultRow,
) -> Result<(), String> {
    ensure_same(identity, "benchmark", base.benchmark == incoming.benchmark)?;
    ensure_same(identity, "runner", base.runner == incoming.runner)?;
    ensure_same(identity, "language", base.language == incoming.language)?;
    ensure_same(identity, "status", base.status == incoming.status)?;
    ensure_same(
        identity,
        "failure_kind",
        base.failure_kind == incoming.failure_kind,
    )?;
    ensure_same(identity, "params", base.params == incoming.params)?;
    ensure_same(identity, "error", base.error == incoming.error)?;
    ensure_same(identity, "artifacts", base.artifacts == incoming.artifacts)?;
    ensure_same(
        identity,
        "case_summary",
        stable_case_summary(&base.case_summary) == stable_case_summary(&incoming.case_summary),
    )?;

    merge_case_summary(identity, &mut base.case_summary, incoming.case_summary)?;
    merge_metrics(identity, &mut base.metrics, incoming.metrics)?;
    recompute_derived_metrics(&mut base.metrics);
    Ok(())
}

fn ensure_same(identity: &str, field: &str, same: bool) -> Result<(), String> {
    if same {
        Ok(())
    } else {
        Err(format!(
            "cannot merge benchmark rows with identity {identity}: conflicting {field}"
        ))
    }
}

fn merge_case_summary(
    identity: &str,
    base: &mut BTreeMap<String, Value>,
    incoming: BTreeMap<String, Value>,
) -> Result<(), String> {
    for key in case_summary_additive_keys() {
        let Some(incoming_value) = incoming.get(*key) else {
            continue;
        };
        match base.entry((*key).to_string()) {
            Entry::Vacant(entry) => {
                entry.insert(incoming_value.clone());
            }
            Entry::Occupied(mut entry) => {
                let merged = sum_json_numbers(
                    entry.get(),
                    incoming_value,
                    identity,
                    &format!("case_summary.{key}"),
                )?;
                entry.insert(merged);
            }
        }
    }
    Ok(())
}

fn sum_json_numbers(
    left: &Value,
    right: &Value,
    identity: &str,
    field: &str,
) -> Result<Value, String> {
    if let (Some(left), Some(right)) = (left.as_u64(), right.as_u64()) {
        return left
            .checked_add(right)
            .map(Value::from)
            .ok_or_else(|| conflict(identity, field));
    }
    if let (Some(left), Some(right)) = (left.as_i64(), right.as_i64()) {
        return left
            .checked_add(right)
            .map(Value::from)
            .ok_or_else(|| conflict(identity, field));
    }
    match (left.as_f64(), right.as_f64()) {
        (Some(left), Some(right)) => Number::from_f64(left + right)
            .map(Value::Number)
            .ok_or_else(|| conflict(identity, field)),
        _ => Err(conflict(identity, field)),
    }
}

fn merge_metrics(identity: &str, base: &mut MetricMap, incoming: MetricMap) -> Result<(), String> {
    for (key, value) in incoming {
        if ADDITIVE_METRICS.contains(&key.as_str()) {
            *base.entry(key).or_insert(0.0) += value;
        } else if DERIVED_METRICS.contains(&key.as_str()) {
            continue;
        } else {
            match base.entry(key) {
                Entry::Vacant(entry) => {
                    entry.insert(value);
                }
                Entry::Occupied(entry) if *entry.get() == value => {}
                Entry::Occupied(entry) => return Err(conflict(identity, entry.key())),
            }
        }
    }
    Ok(())
}

fn recompute_derived_metrics(metrics: &mut MetricMap) {
    let shots_used = metrics.get("shots_used").copied();
    let logical_errors = metrics.get("logical_errors").copied();
    let total_decode_us = metrics.get("total_decode_us").copied();

    if let (Some(logical_errors), Some(shots_used)) = (logical_errors, shots_used) {
        if shots_used != 0.0 {
            metrics.insert("logical_error_rate".into(), logical_errors / shots_used);
        } else {
            metrics.remove("logical_error_rate");
        }
    } else {
        metrics.remove("logical_error_rate");
    }

    if let (Some(total_decode_us), Some(shots_used)) = (total_decode_us, shots_used) {
        if shots_used != 0.0 {
            metrics.insert("decode_us_per_shot".into(), total_decode_us / shots_used);
        } else {
            metrics.remove("decode_us_per_shot");
        }
    } else {
        metrics.remove("decode_us_per_shot");
    }
}

fn conflict(identity: &str, field: &str) -> String {
    format!("cannot merge benchmark rows with identity {identity}: conflicting {field}")
}
