use rsinter::csv_io::{read_csv, write_csv};
use rsinter::failure::FailureKind;
use rsinter::task_stats::TaskStats;
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
        failure_kind: FailureKind::Ok,
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
    assert_eq!(recovered[0].failure_kind, FailureKind::Ok);
    assert_eq!(recovered[0].strong_id, "abc123");
}

#[test]
fn csv_reads_legacy_rows_without_failure_kind() {
    let input = concat!(
        "shots,errors,discards,seconds,decoder,strong_id,json_metadata,custom_counts\n",
        "100,0,0,1.0000,vacuous,clean,\"{\"\"d\"\":3}\",\"{}\"\n",
        "100,4,0,1.0000,vacuous,logical,\"{\"\"d\"\":3}\",\"{}\"\n"
    );

    let recovered = read_csv(input.as_bytes()).unwrap();

    assert_eq!(recovered[0].failure_kind, FailureKind::Ok);
    assert_eq!(recovered[1].failure_kind, FailureKind::LogicalFailure);
}

#[test]
fn task_stats_addition() {
    let a = sample_stats();
    let b = TaskStats {
        shots: 500,
        errors: 2,
        seconds: 0.5,
        ..sample_stats()
    };
    let c = a + b;
    assert_eq!(c.shots, 1500);
    assert_eq!(c.errors, 7);
    assert!((c.seconds - 1.73).abs() < 0.01);
}

#[test]
fn task_stats_addition_keeps_strongest_failure_kind() {
    let a = TaskStats {
        failure_kind: FailureKind::LogicalFailure,
        ..sample_stats()
    };
    let b = TaskStats {
        shots: 500,
        errors: 0,
        seconds: 0.5,
        failure_kind: FailureKind::Timeout,
        ..sample_stats()
    };

    let c = a + b;

    assert_eq!(c.failure_kind, FailureKind::Timeout);
    assert_eq!(c.shots, 1500);
}
