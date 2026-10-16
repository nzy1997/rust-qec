use crate::bench::result::BenchmarkResultRow;

pub fn merge_result_rows(row_sets: Vec<Vec<BenchmarkResultRow>>) -> Vec<BenchmarkResultRow> {
    let mut rows: Vec<BenchmarkResultRow> = row_sets.into_iter().flatten().collect();
    rows.sort_by(|a, b| {
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
            .then(a_p.partial_cmp(&b_p).unwrap_or(std::cmp::Ordering::Equal))
    });
    rows
}
