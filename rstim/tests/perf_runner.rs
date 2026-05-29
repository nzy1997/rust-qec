use rstim::perf::{
    PerfRunOptions, PerfVariant, benchmark_cases, run_case_measurements,
};

#[test]
fn runner_emits_one_warmup_and_five_measured_records_by_default() {
    let case = benchmark_cases()
        .into_iter()
        .find(|case| case.label == "loss-protection-sample")
        .expect("loss protection case");
    let circuit = "LOSS(1) 0\nMRL 0\nDETECTOR rec[-1]\n";

    let records = run_case_measurements(
        case,
        circuit,
        &[PerfVariant::RstimInterpreted],
        PerfRunOptions::default(),
    )
    .expect("runner records");

    assert_eq!(records.len(), 6);
    assert!(records[0].warmup);
    assert_eq!(records[0].measurement_index, 0);
    assert!(!records[1].warmup);
    assert_eq!(records[5].measurement_index, 5);
    assert!(records
        .iter()
        .all(|record| record.case_label == "loss-protection-sample"));
}
