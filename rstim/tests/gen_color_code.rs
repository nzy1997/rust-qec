use rstim::codegen::color_code::memory_xyz;
use rstim::ir::{StimInstr, StimTarget};
use rstim::stats;

#[test]
fn color_code_d3_r2() {
    // d=3 (minimum), r=2 (minimum)
    let instrs = memory_xyz(3, 2, 0.0);
    assert!(stats::num_qubits(&instrs) > 0);
    assert!(stats::num_measurements(&instrs) > 0);
    assert!(stats::num_observables(&instrs) >= 1);
}

#[test]
fn color_code_d3_r3() {
    let instrs = memory_xyz(3, 3, 0.0);
    assert!(stats::num_qubits(&instrs) > 0);
    assert!(stats::num_observables(&instrs) >= 1);
}

#[test]
fn color_code_roundtrip() {
    use rstim::ir::circuit_to_string;
    use rstim::parser::parse_lines;
    let instrs = memory_xyz(3, 2, 0.0);
    let s = circuit_to_string(&instrs);
    let reparsed = parse_lines(&s).unwrap();
    assert_eq!(instrs, reparsed);
}

#[test]
fn color_code_with_noise() {
    let instrs = memory_xyz(3, 2, 0.001);
    let has_noise = instrs.iter().any(|i| {
        matches!(i, StimInstr::Op { name, .. } if name == "DEPOLARIZE1" || name == "DEPOLARIZE2")
    });
    assert!(has_noise);
}

#[test]
fn color_code_has_no_self_targeting_two_qubit_pairs() {
    let instrs = memory_xyz(3, 2, 0.001);

    for instr in &instrs {
        let StimInstr::Op { name, targets, .. } = instr else {
            continue;
        };
        if name != "CX" && name != "DEPOLARIZE2" {
            continue;
        }
        assert_eq!(targets.len() % 2, 0, "{instr:?}");
        for pair in targets.chunks_exact(2) {
            let [StimTarget::Qubit(a), StimTarget::Qubit(b)] = pair else {
                panic!("unexpected non-qubit pair in {instr:?}");
            };
            assert_ne!(a, b, "self-targeting {name} pair in {instr:?}");
        }
    }
}
