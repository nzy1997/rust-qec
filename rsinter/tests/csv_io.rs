use rsinter::task_stats::TaskStats;
use rsinter::csv_io::{write_csv, read_csv};
use std::collections::HashMap;

fn sample_stats() -> TaskStats {
    TaskStats {
        strong_id: "abc123".into(),
        decoder: "vacuous".into(),
        metadata: serde_json::json!({"d": 3}),
        shots: 1000,
        errors: 5,
        discards: 0,
        seconds: 1.23,
        custom_counts: HashMap::new(),
    }
}

#[test]
fn csv_roundtrip() {
    let stats = vec![sample_stats()];
    let mut buf = Vec::new();
    write_csv(&stats, &mut buf).unwrap();
    let recovered = read_csv(&buf).unwrap();
    assert_eq!(recovered.len(), 1);
    assert_eq!(recovered[0].shots, 1000);
    assert_eq!(recovered[0].errors, 5);
    assert_eq!(recovered[0].strong_id, "abc123");
}

#[test]
fn task_stats_addition() {
    let a = sample_stats();
    let b = TaskStats { shots: 500, errors: 2, seconds: 0.5, ..sample_stats() };
    let c = a + b;
    assert_eq!(c.shots, 1500);
    assert_eq!(c.errors, 7);
    assert!((c.seconds - 1.73).abs() < 0.01);
}
