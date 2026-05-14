use rand::rngs::StdRng;
use rand::SeedableRng;
use rstim::sample_trace::{
    DetectorEvent, MeasurementComponent, MeasurementEvent, NoiseEvent, SampleTrace,
};
use rstim::{executor::Executor, parser::parse_lines};
use std::collections::BTreeSet;

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

#[test]
fn traced_execution_covers_measurement_alias_families() {
    let instrs = parse_lines(
        "LOSS(1) 0\n\
         MZ 0\n\
         RX 1\n\
         MX 1\n\
         RY 2\n\
         MY 2\n\
         LOSS(1) 3\n\
         MRZ 3\n\
         LOSS(1) 4\n\
         MRX 4\n\
         LOSS(1) 5\n\
         MRY 5\n\
         LOSS(1) 6\n\
         MZL 6\n\
         LOSS(1) 7\n\
         MXL 7\n\
         LOSS(1) 8\n\
         MYL 8\n\
         LOSS(1) 9\n\
         MRZL 9\n\
         LOSS(1) 10\n\
         MRXL 10\n\
         LOSS(1) 11\n\
         MRYL 11\n",
    )
    .unwrap();
    let mut ex = Executor::from_instrs(instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(1);

    let (out, trace) = ex.run_with_trace(&mut rng).unwrap();

    assert_eq!(out.measurements.len(), 18);

    let event = |name: &str, component: MeasurementComponent| {
        trace
            .measurement_events
            .iter()
            .find(|event| event.instr_name == name && event.component == component)
            .unwrap_or_else(|| panic!("missing measurement event for {name} {component:?}"))
    };

    let mz = event("MZ", MeasurementComponent::Value);
    assert!(mz.bit);
    assert!(mz.loss_cause);

    let mx = event("MX", MeasurementComponent::Value);
    assert!(!mx.bit);
    assert!(!mx.loss_cause);

    let my = event("MY", MeasurementComponent::Value);
    assert!(!my.bit);
    assert!(!my.loss_cause);

    for gate in ["MRZ", "MRX", "MRY"] {
        let value = event(gate, MeasurementComponent::Value);
        assert!(value.bit, "{gate} should report a one on a lost qubit");
        assert!(value.loss_cause, "{gate} should mark the lost-qubit cause");
    }

    for gate in ["MZL", "MXL", "MYL", "MRZL", "MRXL", "MRYL"] {
        let loss_flag = event(gate, MeasurementComponent::LossFlag);
        let value = event(gate, MeasurementComponent::Value);
        assert!(loss_flag.bit, "{gate} should emit a true loss flag");
        assert!(!loss_flag.loss_cause, "{gate} loss flag is not itself loss-caused");
        assert!(value.bit, "{gate} should emit a one-valued measurement bit");
        assert!(value.loss_cause, "{gate} should mark the measurement as loss-caused");
    }
}

#[test]
fn traced_execution_records_noise_channel_branch_labels() {
    let instrs = parse_lines(
        "X_ERROR(1) 0\n\
         Y_ERROR(1) 1\n\
         Z_ERROR(1) 2\n\
         DEPOLARIZE2(1) 3 4\n\
         PAULI_CHANNEL_1(1,0,0) 5\n\
         PAULI_CHANNEL_1(0,1,0) 6\n\
         PAULI_CHANNEL_1(0,0,1) 7\n\
         PAULI_CHANNEL_2(1,0,0,0,0,0,0,0,0,0,0,0,0,0,0) 8 9\n\
         CORRELATED_ERROR(1) X10 Y11\n\
         ELSE_CORRELATED_ERROR(1) Z12\n\
         CORRELATED_ERROR(0) X13\n\
         ELSE_CORRELATED_ERROR(1) Z14\n",
    )
    .unwrap();
    let mut ex = Executor::from_instrs(instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(7);

    let (_out, trace) = ex.run_with_trace(&mut rng).unwrap();

    assert_eq!(trace.noise_events.len(), 10);

    let labels_for = |name: &str| {
        trace
            .noise_events
            .iter()
            .filter(|event| event.instr_name == name)
            .map(|event| event.branch_label.as_deref().unwrap())
            .collect::<Vec<_>>()
    };

    assert_eq!(labels_for("X_ERROR"), vec!["X"]);
    assert_eq!(labels_for("Y_ERROR"), vec!["Y"]);
    assert_eq!(labels_for("Z_ERROR"), vec!["Z"]);
    assert_eq!(labels_for("PAULI_CHANNEL_1"), vec!["X", "Y", "Z"]);
    assert_eq!(labels_for("PAULI_CHANNEL_2"), vec!["IX"]);
    assert_eq!(labels_for("CORRELATED_ERROR"), vec!["XY"]);
    assert_eq!(labels_for("ELSE_CORRELATED_ERROR"), vec!["Z"]);

    let depolarize2 = trace
        .noise_events
        .iter()
        .find(|event| event.instr_name == "DEPOLARIZE2")
        .unwrap();
    let label = depolarize2.branch_label.as_deref().unwrap();
    assert_eq!(label.len(), 2);
    assert_ne!(label, "II");
}

#[test]
fn traced_execution_covers_y_and_z_gate_paths() {
    let instrs = parse_lines("Y 0\nZ 1\nM 0 1\n").unwrap();
    let mut ex = Executor::from_instrs(instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(0);

    let (out, trace) = ex.run_with_trace(&mut rng).unwrap();

    assert_eq!(out.measurements, vec![true, false]);
    assert_eq!(trace.measurement_events.len(), 2);
    assert_eq!(trace.measurement_events[0].target_qubit, 0);
    assert!(trace.measurement_events[0].bit);
    assert!(!trace.measurement_events[0].loss_cause);
    assert_eq!(trace.measurement_events[1].target_qubit, 1);
    assert!(!trace.measurement_events[1].bit);
    assert!(!trace.measurement_events[1].loss_cause);
}

#[test]
fn traced_execution_covers_all_depolarize1_trace_branches() {
    let instrs = parse_lines("DEPOLARIZE1(1) 0\nM 0\n").unwrap();
    let mut seen = BTreeSet::new();

    for seed in 0..64 {
        let mut ex = Executor::from_instrs(instrs.clone()).unwrap();
        let mut rng = StdRng::seed_from_u64(seed);
        let (_out, trace) = ex.run_with_trace(&mut rng).unwrap();
        seen.insert(trace.noise_events[0].branch_label.clone().unwrap());
        if seen.len() == 3 {
            break;
        }
    }

    assert_eq!(
        seen,
        BTreeSet::from(["X".to_string(), "Y".to_string(), "Z".to_string()])
    );
}
