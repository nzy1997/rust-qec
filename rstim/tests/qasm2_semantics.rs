#![cfg(feature = "qasm")]

#[path = "support/qasm2_reference.rs"]
mod reference;

use proptest::prelude::*;
use rand::{SeedableRng, rngs::StdRng};
use reference::{Pauli, Primitive as P, assert_instrument, assert_unitary, h, x};
use rstim::near_clifford::CompiledNearCliffordExecutor;
use rstim::qasm::{Backend, ClassicalBinding, ErrorCode, QasmLimits, from_qasm};
use std::f64::consts::{FRAC_PI_2, FRAC_PI_4, PI};

fn source(n: usize, registers: &str, body: &str) -> String {
    format!("OPENQASM 2.0; include \"qelib1.inc\"; qreg q[{n}]; {registers} {body}")
}
fn lower(n: usize, registers: &str, body: &str) -> rstim::qasm::LoweredQasmCircuit {
    from_qasm(&source(n, registers, body))
        .expect("checked QASM")
        .lower(Backend::NearClifford, &QasmLimits::default())
        .expect("supported QASM")
}

#[test]
fn primitive_u_matches_its_full_matrix_for_all_input_columns() {
    for (t, p, l) in [
        (0.37, -0.81, 1.25),
        (-2.7, 4.1, -0.03),
        (PI, PI, 2. * PI),
        (2. * PI, 0.2, -0.7),
        (4. * PI, -1., 0.),
    ] {
        let plan = lower(1, "", &format!("U({t:.17},{p:.17},{l:.17}) q[0];"));
        assert_unitary(&[P::U(0, t, p, l)], &plan);
    }
}

#[test]
fn standard_single_qubit_shortcuts_match_literal_header_matrices() {
    for (gate, t, p, l) in [
        ("id", 0., 0., 0.),
        ("h", FRAC_PI_2, 0., PI),
        ("x", PI, 0., PI),
        ("y", PI, FRAC_PI_2, FRAC_PI_2),
        ("z", 0., 0., PI),
        ("s", 0., 0., FRAC_PI_2),
        ("sdg", 0., 0., -FRAC_PI_2),
        ("t", 0., 0., FRAC_PI_4),
        ("tdg", 0., 0., -FRAC_PI_4),
        ("rx(0.37)", 0.37, -FRAC_PI_2, FRAC_PI_2),
        ("ry(0.37)", 0.37, 0., 0.),
        ("rz(0.37)", 0., 0., 0.37),
        ("u1(0.37)", 0., 0., 0.37),
        ("u2(0.37,-0.81)", FRAC_PI_2, 0.37, -0.81),
        ("u3(0.37,-0.81,1.25)", 0.37, -0.81, 1.25),
    ] {
        assert_unitary(&[P::U(0, t, p, l)], &lower(1, "", &format!("{gate} q[0];")));
    }
    assert_unitary(
        &[P::ControlledU(0, 1, PI, FRAC_PI_2, FRAC_PI_2)],
        &lower(2, "", "cy q[0],q[1];"),
    );
    assert_unitary(
        &[P::ControlledU(0, 1, 0., 0., PI)],
        &lower(2, "", "cz q[0],q[1];"),
    );
}

#[test]
fn standard_controlled_gates_preserve_relative_control_phases() {
    for (text, angle) in [
        ("pi", PI),
        ("2*pi", 2. * PI),
        ("4*pi", 4. * PI),
        ("-pi/3", -PI / 3.),
        ("0.231", 0.231),
    ] {
        let phase = lower(2, "", &format!("cu1({text}) q[0],q[1];"));
        assert_unitary(&[P::ControlledU(0, 1, 0., 0., angle)], &phase);
        let rotation = lower(2, "", &format!("crz({text}) q[0],q[1];"));
        assert_unitary(&[P::ControlledRz(0, 1, angle)], &rotation);
    }
    let u = lower(2, "", "cu3(0.37,-0.81,1.25) q[0],q[1];");
    // The pinned original header defines cu3 with an SU(2)-normalized
    // controlled block. Its missing control-phase correction is observable.
    assert_unitary(
        &[P::ControlledUPhase(
            0,
            1,
            0.37,
            -0.81,
            1.25,
            -(-0.81 + 1.25) / 2.,
        )],
        &u,
    );
    let ch = lower(2, "", "ch q[0],q[1];");
    assert_unitary(&[P::ControlledU(0, 1, FRAC_PI_2, 0., PI)], &ch);
    let ccx = lower(3, "", "ccx q[0],q[1],q[2];");
    assert_unitary(&[P::Toffoli(0, 1, 2)], &ccx);
}

#[test]
fn user_gate_names_cannot_spoof_standard_definitions() {
    let text = "OPENQASM 2.0; gate x a { U(0.37,0.22,-0.43) a; } qreg q[1]; x q[0];";
    let plan = from_qasm(text)
        .unwrap()
        .lower(Backend::NearClifford, &QasmLimits::default())
        .unwrap();
    assert_unitary(&[P::U(0, 0.37, 0.22, -0.43)], &plan);
    assert_eq!(
        from_qasm(text)
            .unwrap()
            .lower(Backend::Stabilizer, &QasmLimits::default())
            .unwrap_err()
            .code,
        ErrorCode::BackendUnsupported
    );
}

#[test]
fn exact_pi_expressions_are_clifford_but_nearby_decimals_are_not() {
    for body in [
        "rx(pi/2) q[0];",
        "ry(-3*pi/2) q[0];",
        "rz(4*pi) q[0];",
        "U(pi/2,0,pi) q[0];",
    ] {
        from_qasm(&source(1, "", body))
            .unwrap()
            .lower(Backend::Stabilizer, &QasmLimits::default())
            .unwrap();
    }
    let angle = FRAC_PI_2 + 1e-10;
    let body = format!("rx({angle:.17}) q[0];");
    let circuit = from_qasm(&source(1, "", &body)).unwrap();
    assert_eq!(
        circuit
            .lower(Backend::Stabilizer, &QasmLimits::default())
            .unwrap_err()
            .code,
        ErrorCode::BackendUnsupported
    );
    // Rx has primitive U(theta,-pi/2,pi/2), exactly rather than a snapped H/S recipe.
    assert_unitary(
        &[P::U(0, angle, -FRAC_PI_2, FRAC_PI_2)],
        &circuit
            .lower(Backend::NearClifford, &QasmLimits::default())
            .unwrap(),
    );
}

#[test]
fn measurements_collapse_in_place_and_keep_conditional_quantum_states() {
    let plan = lower(
        1,
        "creg c[2];",
        "h q[0]; measure q[0] -> c[0]; h q[0]; measure q[0] -> c[1];",
    );
    assert_instrument(&[h(0), P::Measure(0, 0), h(0), P::Measure(0, 1)], 2, &plan);
}

#[test]
fn feedback_reads_the_measurement_version_at_its_program_point() {
    let plan = lower(
        3,
        "creg c[1]; creg out[1];",
        "h q[0]; measure q[0] -> c[0]; if(c==1) x q[2]; h q[1]; measure q[1] -> c[0]; if(c==0) y q[2]; measure q[2] -> out[0];",
    );
    assert_instrument(
        &[
            h(0),
            P::Measure(0, 0),
            P::If(0, true, Pauli::X, 2),
            h(1),
            P::Measure(1, 0),
            P::If(0, false, Pauli::Y, 2),
            P::Measure(2, 1),
        ],
        2,
        &plan,
    );
    assert_eq!(plan.measurement_events, vec![0, 1, 2]);
    assert_eq!(
        plan.classical_outputs,
        vec![
            ClassicalBinding::Measurement(1),
            ClassicalBinding::Measurement(2)
        ]
    );
}

#[test]
fn initial_zero_bits_and_zero_equality_keep_their_literal_meaning() {
    let plan = lower(
        2,
        "creg c[1]; creg out[1];",
        "if(c==0) x q[1]; if(c==1) z q[1]; h q[0]; measure q[0] -> c[0]; if(c==0) z q[1]; measure q[1] -> out[0];",
    );
    assert_instrument(
        &[
            P::If(0, false, Pauli::X, 1),
            P::If(0, true, Pauli::Z, 1),
            h(0),
            P::Measure(0, 0),
            P::If(0, false, Pauli::Z, 1),
            P::Measure(1, 1),
        ],
        2,
        &plan,
    );
}

#[test]
fn bell_reset_discards_entanglement_without_a_visible_measurement() {
    let plan = lower(
        2,
        "creg c[1];",
        "h q[0]; cx q[0],q[1]; reset q[0]; measure q[0] -> c[0];",
    );
    assert_instrument(
        &[h(0), P::CX(0, 1), P::Reset(0), P::Measure(0, 0)],
        1,
        &plan,
    );
    assert_eq!(plan.measurement_events.len(), 1);
}

#[test]
fn register_layout_unused_qubits_and_overwritten_outputs_survive_execution() {
    let text = "OPENQASM 2.0; include \"qelib1.inc\"; qreg a[1]; qreg b[3]; creg unused[2]; creg result[2]; x a[0]; measure a[0] -> result[1]; reset a[0]; measure a[0] -> result[0];";
    let circuit = from_qasm(text).unwrap();
    let plan = circuit
        .lower(Backend::NearClifford, &QasmLimits::default())
        .unwrap();
    assert_eq!(plan.num_qubits, 4);
    assert_eq!(circuit.quantum_registers()[1].offset, 1);
    assert_eq!(circuit.classical_registers()[1].offset, 2);
    assert_eq!(
        plan.classical_outputs,
        vec![
            ClassicalBinding::Zero,
            ClassicalBinding::Zero,
            ClassicalBinding::Measurement(1),
            ClassicalBinding::Measurement(0)
        ]
    );
    let executor =
        CompiledNearCliffordExecutor::compile_with_limit(plan.instructions.clone(), 16).unwrap();
    let shots = executor.sample(32, &mut StdRng::seed_from_u64(71)).unwrap();
    for shot in shots {
        assert_eq!(shot.measurements, vec![true, false]);
        assert_eq!(
            plan.project_classical(&shot.measurements).unwrap(),
            vec![false, false, false, true]
        );
    }
    assert!(plan.project_classical(&[]).is_err());
}

#[test]
fn compiled_execution_applies_feedback_before_overwriting_its_control() {
    let plan = lower(
        2,
        "creg c[1]; creg d[1];",
        "h q[0]; measure q[0] -> c[0]; if(c==1) x q[0]; measure q[0] -> d[0]; if(d==0) x q[1]; measure q[1] -> c[0];",
    );
    let executor =
        CompiledNearCliffordExecutor::compile_with_limit(plan.instructions.clone(), 16).unwrap();
    for shot in executor
        .sample(64, &mut StdRng::seed_from_u64(117))
        .unwrap()
    {
        assert_eq!(&shot.measurements[1..], &[false, true]);
        assert_eq!(
            plan.project_classical(&shot.measurements).unwrap(),
            vec![true, false]
        );
    }
}

#[test]
fn overwriting_a_classical_bit_does_not_remove_the_earlier_projection() {
    let plan = lower(
        1,
        "creg c[1];",
        "h q[0]; measure q[0] -> c[0]; h q[0]; measure q[0] -> c[0];",
    );
    assert_instrument(&[h(0), P::Measure(0, 0), h(0), P::Measure(0, 0)], 1, &plan);
    assert_eq!(
        plan.classical_outputs,
        vec![ClassicalBinding::Measurement(1)]
    );
}

#[test]
fn custom_parameter_substitution_and_register_broadcasting_have_literal_semantics() {
    let plan = lower(
        2,
        "",
        "gate twist(a,b) t { U(a,b,-a) t; } twist(pi/7,-pi/3) q;",
    );
    assert_unitary(
        &[
            P::U(0, PI / 7., -PI / 3., -PI / 7.),
            P::U(1, PI / 7., -PI / 3., -PI / 7.),
        ],
        &plan,
    );
}

#[test]
fn empty_measurement_free_and_unwritten_outputs_have_defined_layouts() {
    let circuit = from_qasm("OPENQASM 2.0;").unwrap();
    let plan = circuit
        .lower(Backend::Stabilizer, &QasmLimits::default())
        .unwrap();
    assert_eq!(plan.num_qubits, 0);
    assert_eq!(plan.project_classical(&[]).unwrap(), Vec::<bool>::new());
    let plan = lower(2, "creg c[3];", "");
    assert_unitary(&[], &plan);
    assert_eq!(plan.project_classical(&[]).unwrap(), vec![false; 3]);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]
    #[test]
    fn generated_unitary_programs_match_independent_full_operators(ops in prop::collection::vec((0u8..6,0usize..2,-16i32..17,-16i32..17,-16i32..17),0..18)) {
        let mut body = String::new();
        let mut expected = Vec::new();
        for (kind,q,a,b,c) in ops {
            match kind {
                0 => { body.push_str(&format!("h q[{q}];")); expected.push(h(q)); }
                1 => { body.push_str(&format!("x q[{q}];")); expected.push(x(q)); }
                2 => { body.push_str(&format!("cx q[{q}],q[{}];",1-q)); expected.push(P::CX(q,1-q)); }
                3 => { body.push_str(&format!("t q[{q}];")); expected.push(P::U(q,0.,0.,FRAC_PI_4)); }
                _ => {
                    let (t,p,l) = (a as f64/7.,b as f64/11.,c as f64/13.);
                    body.push_str(&format!("U({t:.17},{p:.17},{l:.17}) q[{q}];"));
                    expected.push(P::U(q,t,p,l));
                }
            }
        }
        assert_unitary(&expected,&lower(2,"",&body));
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]
    #[test]
    fn generated_dynamic_programs_match_classically_labelled_quantum_maps(ops in prop::collection::vec((0u8..9,0usize..2,any::<bool>()),0..10)) {
        let mut body = String::new();
        let mut expected = Vec::new();
        for (kind,q,value) in ops {
            let reg = if q==0 {"c"} else {"d"};
            match kind {
                0 => { body.push_str(&format!("h q[{q}];")); expected.push(h(q)); }
                1 => { body.push_str(&format!("x q[{q}];")); expected.push(x(q)); }
                2 => { body.push_str(&format!("cx q[{q}],q[{}];",1-q)); expected.push(P::CX(q,1-q)); }
                3 => { body.push_str(&format!("measure q[{q}] -> {reg}[0];")); expected.push(P::Measure(q,q)); }
                4 => { body.push_str(&format!("reset q[{q}];")); expected.push(P::Reset(q)); }
                _ => {
                    let (name,pauli) = match kind%3 {0=>("x",Pauli::X),1=>("y",Pauli::Y),_=>("z",Pauli::Z)};
                    body.push_str(&format!("if({reg}=={}) {name} q[{}];",u8::from(value),1-q));
                    expected.push(P::If(q,value,pauli,1-q));
                }
            }
        }
        assert_instrument(&expected,3,&lower(2,"creg c[1]; creg d[1]; creg unused[1];",&body));
    }
}
