use rstim::sample_trace::{
    DetectorEvent, MeasurementComponent, MeasurementEvent, NoiseEvent, SampleTrace,
};

#[test]
fn sample_trace_holds_noise_measurement_and_detector_events() {
    let trace = SampleTrace {
        noise_events: vec![NoiseEvent {
            op_path: vec![1],
            repeat_iterations: vec![0_u64],
            instr_name: "DEPOLARIZE1".to_string(),
            target_slots: vec![0],
            target_qubits: vec![5],
            occurred: true,
            branch_label: Some("Y".to_string()),
        }],
        measurement_events: vec![MeasurementEvent {
            op_path: vec![2],
            repeat_iterations: vec![0_u64],
            target_slot: 0,
            target_qubit: 5,
            instr_name: "M".to_string(),
            measurement_index: 1,
            bit: true,
            loss_cause: false,
            component: MeasurementComponent::Value,
        }],
        detector_events: vec![DetectorEvent {
            op_path: vec![3],
            repeat_iterations: vec![0_u64],
            detector_index: 0,
            flipped: true,
        }],
    };

    assert_eq!(trace.noise_events[0].branch_label.as_deref(), Some("Y"));
    assert!(trace.measurement_events[0].bit);
    assert!(trace.detector_events[0].flipped);
}
