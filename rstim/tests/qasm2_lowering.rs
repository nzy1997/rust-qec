#![cfg(feature = "qasm")]

use rstim::ir::{StimInstr, StimTarget};
use rstim::qasm::{Angle, Backend, ClassicalBinding, ErrorCode, QasmLimits, from_qasm};

fn program(body: &str) -> String {
    format!("OPENQASM 2.0;\ninclude \"qelib1.inc\";\n{body}")
}

#[test]
fn classical_overwrites_and_feedback_refer_to_the_correct_measurement_versions() {
    let circuit = from_qasm(&program("qreg q[3]; creg c[1]; creg d[1]; measure q[0] -> c[0]; measure q[2] -> d[0]; if(c==1) x q[1]; measure q[1] -> c[0]; if(c==0) z q[2];")).unwrap();
    let plan = circuit
        .lower(Backend::Stabilizer, &QasmLimits::default())
        .unwrap();
    let feedback: Vec<_> = plan.instructions.iter().filter(|i| matches!(i,StimInstr::Op{targets,..} if targets.iter().any(|t|matches!(t,StimTarget::Rec(_))))).collect();
    assert_eq!(feedback.len(), 2);
    assert_eq!(feedback[0].name(), Some("CX"));
    assert_eq!(
        feedback[0].targets().unwrap(),
        [StimTarget::Rec(-2), StimTarget::Qubit(1)]
    );
    assert_eq!(feedback[1].name(), Some("CZ"));
    assert_eq!(
        feedback[1].targets().unwrap(),
        [StimTarget::Rec(-1), StimTarget::Qubit(2)]
    );
    assert_eq!(
        plan.classical_outputs,
        vec![
            ClassicalBinding::Measurement(2),
            ClassicalBinding::Measurement(1)
        ]
    );
    assert_eq!(
        plan.project_classical(&[true, false, false]).unwrap(),
        [false, false]
    );
    assert_eq!(
        plan.project_classical(&[false, true, true]).unwrap(),
        [true, true]
    );
    assert_eq!(
        plan.project_classical(&[true]).unwrap_err().code,
        ErrorCode::Type
    );
}

#[test]
fn declarations_preserve_idle_quantum_width_and_unwritten_classical_zeros() {
    let circuit = from_qasm("OPENQASM 2.0; qreg q[5]; creg c[3];").unwrap();
    let plan = circuit
        .lower(Backend::Stabilizer, &QasmLimits::default())
        .unwrap();
    assert_eq!(plan.num_qubits, 5);
    assert_eq!(rstim::stats::num_qubits(&plan.instructions), 5);
    assert_eq!(plan.project_classical(&[]).unwrap(), [false; 3]);
    assert_eq!(circuit.num_classical_bits(), 3);
    assert!(plan.measurement_events.is_empty());
}

#[test]
fn initialized_classical_zero_folds_conditions_without_a_fake_measurement() {
    let circuit = from_qasm(&program(
        "qreg q[2]; creg c[1]; if(c==0) x q[0]; if(c==1) y q[1];",
    ))
    .unwrap();
    let plan = circuit
        .lower(Backend::Stabilizer, &QasmLimits::default())
        .unwrap();
    let text = rstim::ir::circuit_to_string(&plan.instructions);
    assert!(text.contains("X 0\n"));
    assert!(!text.contains("Y 1"));
    assert!(!text.contains("rec["));
    assert!(plan.measurement_events.is_empty());
    assert_eq!(plan.project_classical(&[]).unwrap(), [false]);
}

#[test]
fn near_clifford_is_explicit_and_reports_origin_for_stabilizer_rejection() {
    let circuit = from_qasm(&program("qreg q[1]; t q[0];")).unwrap();
    let err = circuit
        .lower(Backend::Stabilizer, &QasmLimits::default())
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::BackendUnsupported);
    assert!(err.message.contains("near-clifford"));
    assert!(err.origin.is_some());
    let near = circuit
        .lower(Backend::NearClifford, &QasmLimits::default())
        .unwrap();
    assert!(near.instructions.iter().any(|i| i.name() == Some("T")));
    let report = circuit.analyze(&QasmLimits::default()).unwrap();
    assert!(!report.stabilizer);
    assert!(report.near_clifford);
}

#[test]
fn exact_pi_angles_use_cliffords_without_snapping_nearby_decimal_angles() {
    let circuit = from_qasm(&program(
        "qreg q[1]; rx(pi/2) q[0]; ry(-pi/2) q[0]; rz(pi) q[0];",
    ))
    .unwrap();
    let plan = circuit
        .lower(Backend::Stabilizer, &QasmLimits::default())
        .unwrap();
    assert_eq!(
        plan.instructions
            .iter()
            .filter_map(StimInstr::name)
            .collect::<Vec<_>>(),
        ["I", "SQRT_X", "SQRT_Y_DAG", "Z"]
    );
    let decimal = from_qasm(&program("qreg q[1]; rx(1.5707963267948965) q[0];")).unwrap();
    assert_eq!(
        decimal
            .lower(Backend::Stabilizer, &QasmLimits::default())
            .unwrap_err()
            .code,
        ErrorCode::BackendUnsupported
    );
    let plan = decimal
        .lower(Backend::NearClifford, &QasmLimits::default())
        .unwrap();
    assert!(plan.instructions.iter().any(|i| i.name() == Some("R_X")));
}

#[test]
fn rotations_convert_radians_to_half_turns_and_never_become_resets() {
    let circuit = from_qasm(&program(
        "qreg q[1]; rx(0.3) q[0]; ry(0.4) q[0]; rz(0.5) q[0]; reset q[0];",
    ))
    .unwrap();
    let plan = circuit
        .lower(Backend::NearClifford, &QasmLimits::default())
        .unwrap();
    let rotations: Vec<_> = plan
        .instructions
        .iter()
        .filter(|i| i.name().is_some_and(|n| n.starts_with("R_")))
        .collect();
    assert_eq!(rotations.len(), 3);
    for (op, value) in rotations.into_iter().zip([0.3, 0.4, 0.5]) {
        assert!((op.args().unwrap()[0] - value / std::f64::consts::PI).abs() < 1e-15);
    }
    assert_eq!(
        plan.instructions
            .iter()
            .filter(|i| i.name() == Some("R"))
            .count(),
        1
    );
    assert!(plan.measurement_events.is_empty());
}

#[test]
fn lowered_expansion_limit_is_checked_including_width_reservation() {
    let circuit = from_qasm(&program("qreg q[1]; U(pi/2,pi/2,pi/2) q[0];")).unwrap();
    let limits = QasmLimits {
        max_operations: 3,
        ..QasmLimits::default()
    };
    assert_eq!(
        circuit
            .lower(Backend::Stabilizer, &limits)
            .unwrap_err()
            .code,
        ErrorCode::Limit
    );
}

#[test]
fn invalid_angles_are_never_accepted_into_execution() {
    assert!(Angle::pi_ratio(1, 0).is_err());
    assert!(Angle::radians(f64::INFINITY).is_err());
    assert!(Angle::radians(f64::NAN).is_err());
    assert_eq!(
        Angle::pi_ratio(-3, 2)
            .unwrap()
            .clifford_quarters()
            .unwrap()
            .rem_euclid(4),
        1
    );
}
