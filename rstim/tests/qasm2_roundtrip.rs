#![cfg(feature = "qasm")]

#[allow(dead_code)]
#[path = "support/qasm2_reference.rs"]
mod reference;

use proptest::prelude::*;
use reference::{Pauli, Primitive as P, assert_instrument, assert_unitary, h, x};
use rstim::ir::{StimInstr, StimTarget};
use rstim::qasm::{
    Backend, ExportOptions, OperationKind, QasmCircuit, QasmLimits, export_stim, from_qasm, to_qasm,
};
use std::f64::consts::{FRAC_PI_2, FRAC_PI_4, PI};

fn target(q: usize) -> String {
    if q < 2 {
        format!("alpha[{q}]")
    } else {
        "omega[0]".to_owned()
    }
}
fn source(body: &str) -> String {
    format!(
        "OPENQASM 2.0; include \"qelib1.inc\"; gate bend(t,p,l) a {{ U(t,p,l) a; }} \
         qreg alpha[2]; qreg omega[1]; creg unused[2]; creg c[1]; creg d[1]; {body}"
    )
}

// Unitaries may expand into a different gate sequence. The ordered channel and
// classical effects must retain their exact positions relative to one another.
fn effects(circuit: &QasmCircuit) -> Vec<OperationKind> {
    circuit
        .operations()
        .iter()
        .filter_map(|operation| match &operation.kind {
            OperationKind::Gate { .. } => None,
            effect => Some(effect.clone()),
        })
        .collect()
}
fn restored(source: &str) -> (QasmCircuit, QasmCircuit) {
    let checked = from_qasm(source).expect("generated source must parse");
    let output = to_qasm(&checked, &ExportOptions::default(), &QasmLimits::default())
        .expect("checked source must export");
    assert!(output.omitted_annotations.is_empty());
    let again = from_qasm(&output.text).expect("generated export must parse");
    assert_eq!(checked.quantum_registers(), again.quantum_registers());
    assert_eq!(checked.classical_registers(), again.classical_registers());
    assert_eq!(effects(&checked), effects(&again));
    (checked, again)
}
fn unitary_roundtrip(source: &str, expected: &[P]) {
    let (checked, again) = restored(source);
    for program in [checked, again] {
        let plan = program
            .lower(Backend::NearClifford, &QasmLimits::default())
            .unwrap();
        assert_eq!(plan.num_qubits, 3);
        assert_unitary(expected, &plan);
    }
}
fn instrument_roundtrip(source: &str, bits: usize, expected: &[P]) {
    let (checked, again) = restored(source);
    for program in [checked, again] {
        let plan = program
            .lower(Backend::NearClifford, &QasmLimits::default())
            .unwrap();
        assert_eq!(plan.num_qubits, 3);
        assert_instrument(expected, bits, &plan);
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]
    #[test]
    fn qasm_unitary_roundtrips_match_independent_three_qubit_operators(
        operations in prop::collection::vec((0u8..8,0usize..3,-12i32..13,-12i32..13,-12i32..13),0..21)
    ) {
        let mut text=String::new();
        let mut expected=Vec::new();
        for (kind,q,a,b,c) in operations {
            let operand=target(q);
            match kind {
                0=>{text.push_str(&format!("h {operand};"));expected.push(h(q));},
                1=>{text.push_str(&format!("x {operand};"));expected.push(x(q));},
                2=>{let other=(q+1)%3;text.push_str(&format!("cx {operand},{};",target(other)));expected.push(P::CX(q,other));},
                3=>{text.push_str(&format!("t {operand};"));expected.push(P::U(q,0.,0.,FRAC_PI_4));},
                4=>{text.push_str(&format!("barrier {operand};"));},
                _=>{
                    let name=if kind==5 {"U"} else {"bend"};
                    text.push_str(&format!("{name}({a}*pi/7,{b}*pi/11,{c}*pi/13) {operand};"));
                    expected.push(P::U(q,a as f64*PI/7.,b as f64*PI/11.,c as f64*PI/13.));
                },
            }
        }
        unitary_roundtrip(&source(&text),&expected);
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]
    #[test]
    fn qasm_dynamic_roundtrips_preserve_output_labelled_quantum_maps(
        operations in prop::collection::vec((0u8..12,0usize..3,any::<bool>()),0..13)
    ) {
        let mut text=String::new();
        let mut expected=Vec::new();
        let (mut measurements,mut resets)=(0,0);
        for (kind,q,value) in operations {
            let operand=target(q);
            let (register,bit)=if value {("c",2)} else {("d",3)};
            match kind {
                0=>{text.push_str(&format!("h {operand};"));expected.push(h(q));},
                1=>{text.push_str(&format!("x {operand};"));expected.push(x(q));},
                2=>{let other=(q+1)%3;text.push_str(&format!("cx {operand},{};",target(other)));expected.push(P::CX(q,other));},
                3|4 if measurements<4=>{measurements+=1;text.push_str(&format!("measure {operand} -> {register}[0];"));expected.push(P::Measure(q,bit));},
                5 if resets<2=>{resets+=1;text.push_str(&format!("reset {operand};"));expected.push(P::Reset(q));},
                6=>{text.push_str(&format!("barrier {operand},{};",target((q+1)%3)));},
                7=>{text.push_str(&format!("t {operand};"));expected.push(P::U(q,0.,0.,FRAC_PI_4));},
                8..=11=>{
                    let (name,pauli)=match kind%3 {0=>("x",Pauli::X),1=>("y",Pauli::Y),_=>("z",Pauli::Z)};
                    // The condition and register choices are independent of the
                    // qubit, exercising writes and reads across all registers.
                    let condition=kind%2==0;
                    text.push_str(&format!("if({register}=={}) {name} {operand};",u8::from(condition)));
                    expected.push(P::If(bit,condition,pauli,q));
                },
                _=>{text.push_str(&format!("h {operand};"));expected.push(h(q));},
            }
        }
        instrument_roundtrip(&source(&text),4,&expected);
    }
}

fn native(name: &str, arguments: Vec<f64>, qubits: &[usize]) -> StimInstr {
    StimInstr::new(
        name,
        arguments,
        qubits
            .iter()
            .map(|q| StimTarget::Qubit(*q as u32))
            .collect(),
    )
}
proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]
    #[test]
    fn native_roundtrips_match_independent_instruments_and_record_layouts(
        operations in prop::collection::vec((0u8..16,0usize..3,-6i32..7,any::<bool>()),0..13)
    ) {
        // A high identity reserves the unused declared width for short cases.
        let mut instructions=vec![native("I",vec![],&[2])];
        let mut expected=Vec::new();
        let (mut measurements,mut resets)=(0usize,0);
        for (kind,q,angle,value) in operations {
            match kind {
                0..=5=>{
                    let (name,p)=match kind {
                        0=>("I",P::U(q,0.,0.,0.)),1=>("H",h(q)),2=>("X",x(q)),
                        3=>("Y",P::U(q,PI,FRAC_PI_2,FRAC_PI_2)),
                        4=>("Z",P::U(q,0.,0.,PI)),_=>("S",P::U(q,0.,0.,FRAC_PI_2)),
                    };
                    instructions.push(native(name,vec![],&[q]));expected.push(p);
                },
                6..=8=>{
                    let half_turns=angle as f64/4.;
                    let theta=half_turns*PI;
                    let (name,p)=match kind {6=>("R_X",P::U(q,theta,-FRAC_PI_2,FRAC_PI_2)),7=>("R_Y",P::U(q,theta,0.,0.)),_=>("R_Z",P::U(q,0.,0.,theta))};
                    instructions.push(native(name,vec![half_turns],&[q]));expected.push(p);
                },
                9=>{let other=(q+1)%3;instructions.push(native("CX",vec![],&[q,other]));expected.push(P::CX(q,other));},
                10 if measurements<4=>{instructions.push(native("M",vec![],&[q]));expected.push(P::Measure(q,measurements));measurements+=1;},
                11 if resets<2=>{resets+=1;instructions.push(native("R",vec![],&[q]));expected.push(P::Reset(q));},
                12..=14 if measurements>0=>{
                    let (name,pauli)=match kind {12=>("CX",Pauli::X),13=>("CY",Pauli::Y),_=>("CZ",Pauli::Z)};
                    let lookback=if value {1}else{measurements};
                    instructions.push(StimInstr::new(name,vec![],vec![StimTarget::Rec(-(lookback as i32)),StimTarget::Qubit(q as u32)]));
                    expected.push(P::If(measurements-lookback,true,pauli,q));
                },
                _=>instructions.push(native("TICK",vec![],&[])),
            }
        }
        let output=export_stim(&instructions,&ExportOptions::default(),&QasmLimits::default()).unwrap();
        assert!(output.omitted_annotations.is_empty());
        assert_eq!(output.classical_registers.len(),measurements);
        for (index,register) in output.classical_registers.iter().enumerate() {
            assert_eq!(register.name,format!("m{index}"));assert_eq!(register.offset,index);assert_eq!(register.size,1);
        }
        let program=from_qasm(&output.text).unwrap();
        let plan=program.lower(Backend::NearClifford,&QasmLimits::default()).unwrap();
        assert_eq!(plan.num_qubits,3);
        assert_instrument(&expected,measurements,&plan);
        let (_,again)=restored(&output.text);
        let second=again.lower(Backend::NearClifford,&QasmLimits::default()).unwrap();
        assert_instrument(&expected,measurements,&second);
    }
}

#[test]
fn overwritten_outputs_unused_declarations_namespace_and_large_angles_survive_roundtrip() {
    let source = "OPENQASM 2.0; qreg h[2]; qreg rx[1]; creg x[1]; creg spare[2]; U(pi/2,0,pi) h[0]; U(0,0,9007199254740993*pi) h[1]; measure h[0] -> x[0]; if(x==0) U(pi,pi/2,pi/2) h[1]; measure h[1] -> x[0]; barrier h[1],rx[0];";
    let expected = [
        h(0),
        P::U(1, 0., 0., PI),
        P::Measure(0, 0),
        P::If(0, false, Pauli::Y, 1),
        P::Measure(1, 0),
    ];
    instrument_roundtrip(source, 3, &expected);
    let (first, second) = restored(source);
    assert_eq!(first.quantum_registers()[1].name, "rx");
    assert_eq!(second.classical_registers()[1].size, 2);
    assert_eq!(
        second
            .operations()
            .iter()
            .filter(|op| matches!(op.kind, OperationKind::Measure { bit: 0, .. }))
            .count(),
        2
    );
}

#[test]
fn minimum_exact_angle_coefficients_survive_export_and_stabilizer_classification() {
    for (gate, expected) in [
        ("rx((-9223372036854775807-1)*pi)", P::U(0, 0., 0., 0.)),
        ("ry((-9223372036854775807-1)*pi)", P::U(0, 0., 0., 0.)),
        ("rz((-9223372036854775807-1)*pi)", P::U(0, 0., 0., 0.)),
        (
            "U((-9223372036854775807-1)*pi,(-9223372036854775807-1)*pi,pi)",
            P::U(0, 0., 0., PI),
        ),
    ] {
        let input = format!("OPENQASM 2.0; include \"qelib1.inc\"; qreg q[1]; {gate} q[0];");
        let checked = from_qasm(&input).unwrap();
        let output = to_qasm(&checked, &ExportOptions::default(), &QasmLimits::default()).unwrap();
        assert!(
            output.text.contains("(-9223372036854775807-1)"),
            "{}",
            output.text
        );
        let restored = from_qasm(&output.text).unwrap();
        let plan = restored
            .lower(Backend::Stabilizer, &QasmLimits::default())
            .unwrap();
        assert_unitary(&[expected], &plan);
    }
}

#[test]
fn unrepresentable_exact_denominators_are_reported_without_float_substitution() {
    for (first, second, denominator) in [
        (2147483648u64, 4294967296u64, 1u64 << 63),
        (4294967295, 4294967297, u64::MAX),
    ] {
        for name in ["rx", "ry", "rz"] {
            let text = format!(
                "OPENQASM 2.0; include \"qelib1.inc\"; qreg q[1]; {name}(pi/{first}/{second}) q[0];"
            );
            let program = from_qasm(&text).unwrap();
            let OperationKind::Gate { gate, .. } = &program.operations()[0].kind else {
                panic!()
            };
            let angle = match gate {
                rstim::qasm::Gate::Rx(a) | rstim::qasm::Gate::Ry(a) | rstim::qasm::Gate::Rz(a) => a,
                _ => panic!(),
            };
            assert_eq!(
                *angle,
                rstim::qasm::Angle::pi_ratio(1, denominator).unwrap()
            );
            let error =
                to_qasm(&program, &ExportOptions::default(), &QasmLimits::default()).unwrap_err();
            assert_eq!(error.code, rstim::qasm::ErrorCode::Unrepresentable);
            assert!(error.message.contains("denominator"));
            assert!(error.origin.is_some());
        }
    }
    let program =
        from_qasm("OPENQASM 2.0; qreg q[1]; U(pi/4294967295/4294967297,0,0) q[0];").unwrap();
    assert_eq!(
        to_qasm(&program, &ExportOptions::default(), &QasmLimits::default())
            .unwrap_err()
            .code,
        rstim::qasm::ErrorCode::Unrepresentable
    );
}

#[test]
fn many_singleton_registers_reimport_with_correct_nearby_and_oldest_feedback() {
    const RECORDS: usize = 8192;
    let mut instructions = vec![native("I", vec![], &[2])];
    for index in 0..RECORDS {
        instructions.push(native("M", vec![], &[0]));
        instructions.push(StimInstr::new(
            "CX",
            vec![],
            vec![StimTarget::Rec(-1), StimTarget::Qubit(1)],
        ));
        instructions.push(StimInstr::new(
            "CZ",
            vec![],
            vec![StimTarget::Rec(-((index + 1) as i32)), StimTarget::Qubit(2)],
        ));
    }
    let output = export_stim(
        &instructions,
        &ExportOptions::default(),
        &QasmLimits::default(),
    )
    .unwrap();
    assert_eq!(output.classical_registers.len(), RECORDS);
    let program = from_qasm(&output.text).unwrap();
    assert_eq!(program.classical_registers(), output.classical_registers);
    let plan = program
        .lower(Backend::Stabilizer, &QasmLimits::default())
        .unwrap();
    assert_eq!(plan.measurement_events.len(), RECORDS);
    for (index, group) in program.operations()[1..].chunks_exact(3).enumerate() {
        assert!(matches!(group[0].kind,OperationKind::Measure{qubit:0,bit}if bit==index));
        assert!(
            matches!(group[1].kind,OperationKind::Conditional{bit,value:true,gate:rstim::qasm::Pauli::X,..}if bit==index)
        );
        assert!(matches!(
            group[2].kind,
            OperationKind::Conditional {
                bit: 0,
                value: true,
                gate: rstim::qasm::Pauli::Z,
                ..
            }
        ));
    }
    let output_again =
        to_qasm(&program, &ExportOptions::default(), &QasmLimits::default()).unwrap();
    assert_eq!(output.text, output_again.text);
}

#[test]
fn condition_register_lookup_never_accepts_quantum_or_gate_identifiers() {
    for name in ["q", "custom", "missing"] {
        let source = format!(
            "OPENQASM 2.0; gate custom a {{ U(pi,0,pi) a; }} qreg q[1]; if({name}==0) U(pi,0,pi) q[0];"
        );
        let error = from_qasm(&source).unwrap_err();
        assert_eq!(error.code, rstim::qasm::ErrorCode::Type);
        assert!(error.message.contains("declared classical register"));
        let origin = error.origin.unwrap();
        assert_eq!(origin.source, "<input>");
        assert_eq!(&source[origin.offset..origin.offset + origin.length], "if");
    }
}
