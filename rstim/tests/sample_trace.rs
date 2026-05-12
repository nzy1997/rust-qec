use rand::rngs::StdRng;
use rand::SeedableRng;
use rstim::sample_trace::{
    DetectorEvent, MeasurementComponent, MeasurementEvent, NoiseEvent, SampleTrace,
};
use rstim::{executor::Executor, parser::parse_lines};

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

#[test]
fn traced_execution_records_loss_as_branch_l() {
    let instrs = parse_lines("LOSS(1) 0\nM 0\n").unwrap();
    let mut ex = Executor::from_instrs(instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(1);

    let (_out, trace) = ex.run_with_trace(&mut rng).unwrap();

    assert_eq!(trace.noise_events.len(), 1);
    assert_eq!(trace.noise_events[0].instr_name, "LOSS");
    assert_eq!(trace.noise_events[0].op_path, vec![0]);
    assert_eq!(trace.noise_events[0].repeat_iterations, Vec::<u64>::new());
    assert_eq!(trace.noise_events[0].target_slots, vec![0]);
    assert_eq!(trace.noise_events[0].target_qubits, vec![0]);
    assert_eq!(trace.noise_events[0].branch_label.as_deref(), Some("L"));
}

#[test]
fn traced_execution_marks_loss_caused_measurement() {
    let instrs = parse_lines("LOSS(1) 0\nM 0\n").unwrap();
    let mut ex = Executor::from_instrs(instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(1);

    let (out, trace) = ex.run_with_trace(&mut rng).unwrap();

    assert_eq!(out.measurements, vec![true]);
    assert_eq!(trace.measurement_events.len(), 1);
    assert_eq!(trace.measurement_events[0].instr_name, "M");
    assert_eq!(trace.measurement_events[0].op_path, vec![1]);
    assert_eq!(trace.measurement_events[0].measurement_index, 1);
    assert_eq!(
        trace.measurement_events[0].component,
        MeasurementComponent::Value
    );
    assert!(trace.measurement_events[0].bit);
    assert!(trace.measurement_events[0].loss_cause);
}

#[test]
fn traced_execution_records_resolved_depolarize1_branch() {
    let instrs = parse_lines("DEPOLARIZE1(1) 0\nM 0\n").unwrap();
    let mut ex = Executor::from_instrs(instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(2);

    let (_out, trace) = ex.run_with_trace(&mut rng).unwrap();

    assert_eq!(trace.noise_events.len(), 1);
    let label = trace.noise_events[0].branch_label.as_deref().unwrap();
    assert!(matches!(label, "X" | "Y" | "Z"));
}

#[test]
fn traced_execution_orders_loss_visible_measurement_components_loss_flag_then_value() {
    let instrs = parse_lines("LOSS(1) 0\nMRL 0\n").unwrap();
    let mut ex = Executor::from_instrs(instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(1);

    let (out, trace) = ex.run_with_trace(&mut rng).unwrap();

    assert_eq!(out.measurements, vec![true, true]);
    assert_eq!(trace.measurement_events.len(), 2);

    assert_eq!(trace.measurement_events[0].measurement_index, 1);
    assert_eq!(
        trace.measurement_events[0].component,
        MeasurementComponent::LossFlag
    );
    assert!(trace.measurement_events[0].bit);
    assert!(!trace.measurement_events[0].loss_cause);

    assert_eq!(trace.measurement_events[1].measurement_index, 2);
    assert_eq!(
        trace.measurement_events[1].component,
        MeasurementComponent::Value
    );
    assert!(trace.measurement_events[1].bit);
    assert!(trace.measurement_events[1].loss_cause);
}

#[test]
fn traced_execution_tracks_repeat_iterations_in_process() {
    let instrs = parse_lines("REPEAT 2 {\n  LOSS(1) 0\n  M 0\n  DETECTOR rec[-1]\n}\n").unwrap();
    let mut ex = Executor::from_instrs(instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(1);

    let (_out, trace) = ex.run_with_trace(&mut rng).unwrap();

    assert_eq!(trace.noise_events.len(), 2);
    assert_eq!(trace.noise_events[0].op_path, vec![0, 0]);
    assert_eq!(trace.noise_events[0].repeat_iterations, vec![0]);
    assert_eq!(trace.noise_events[1].op_path, vec![0, 0]);
    assert_eq!(trace.noise_events[1].repeat_iterations, vec![1]);

    assert_eq!(trace.measurement_events.len(), 2);
    assert_eq!(trace.measurement_events[0].measurement_index, 1);
    assert_eq!(trace.measurement_events[0].repeat_iterations, vec![0]);
    assert_eq!(trace.measurement_events[1].measurement_index, 2);
    assert_eq!(trace.measurement_events[1].repeat_iterations, vec![1]);

    assert_eq!(trace.detector_events.len(), 2);
    assert_eq!(trace.detector_events[0].op_path, vec![0, 2]);
    assert_eq!(trace.detector_events[0].repeat_iterations, vec![0]);
    assert!(trace.detector_events[0].flipped);
    assert_eq!(trace.detector_events[1].op_path, vec![0, 2]);
    assert_eq!(trace.detector_events[1].repeat_iterations, vec![1]);
    assert!(trace.detector_events[1].flipped);
}
