use rstim::perf::{
    PerfRunOptions, PerfVariant, benchmark_cases, run_case_measurements,
};

const LOSS_PROTECTION_CIRCUIT: &str = "LOSS(1) 0\nMRL 0\nDETECTOR rec[-1]\n";

fn loss_protection_case() -> rstim::perf::PerfBenchmarkCase {
    benchmark_cases()
        .into_iter()
        .find(|case| case.label == "loss-protection-sample")
        .expect("loss protection case")
}

#[test]
fn runner_emits_one_warmup_and_five_measured_records_by_default() {
    let records = run_case_measurements(
        loss_protection_case(),
        LOSS_PROTECTION_CIRCUIT,
        &[PerfVariant::RstimInterpreted],
        PerfRunOptions::default(),
    )
    .expect("runner records");

    let warmup_count = records.iter().filter(|record| record.warmup).count();
    let measured_count = records.iter().filter(|record| !record.warmup).count();

    assert_eq!(records.len(), 6);
    assert_eq!(warmup_count, 1);
    assert_eq!(measured_count, 5);
    assert!(records[0].warmup);
    assert_eq!(records[0].measurement_index, 0);
    assert!(!records[1].warmup);
    assert_eq!(records[5].measurement_index, 5);
    assert!(records
        .iter()
        .all(|record| record.case_label == "loss-protection-sample"));
}

#[test]
fn runner_rejects_zero_measured_rounds() {
    let result = run_case_measurements(
        loss_protection_case(),
        LOSS_PROTECTION_CIRCUIT,
        &[PerfVariant::RstimInterpreted],
        PerfRunOptions {
            warmup_rounds: 1,
            measured_rounds: 0,
        },
    );

    assert!(result.is_err());
    assert!(result.unwrap_err().contains("measured_rounds"));
}
