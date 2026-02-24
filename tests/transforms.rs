use rstim::parser::parse_lines;
use rstim::transforms;

#[test]
fn flattened_no_repeat() {
    let instrs = parse_lines("H 0\nM 0").unwrap();
    let flat = transforms::flattened(&instrs);
    assert_eq!(flat.len(), 2);
}

#[test]
fn flattened_simple_repeat() {
    let instrs = parse_lines("REPEAT 3 {\n  H 0\n  M 0\n}").unwrap();
    let flat = transforms::flattened(&instrs);
    assert_eq!(flat.len(), 6);
}

#[test]
fn flattened_nested_repeat() {
    let instrs = parse_lines("REPEAT 2 {\n  REPEAT 3 {\n    H 0\n  }\n}").unwrap();
    let flat = transforms::flattened(&instrs);
    assert_eq!(flat.len(), 6);
}

#[test]
fn flattened_mixed() {
    let instrs = parse_lines("X 0\nREPEAT 2 {\n  H 0\n}\nM 0").unwrap();
    let flat = transforms::flattened(&instrs);
    assert_eq!(flat.len(), 4);
}

#[test]
fn without_noise_removes_errors() {
    let instrs = parse_lines("H 0\nDEPOLARIZE1(0.01) 0\nX_ERROR(0.1) 0\nM 0").unwrap();
    let clean = transforms::without_noise(&instrs);
    assert_eq!(clean.len(), 2);
    assert_eq!(clean[0].name().unwrap(), "H");
    assert_eq!(clean[1].name().unwrap(), "M");
}

#[test]
fn without_noise_preserves_repeat() {
    let instrs = parse_lines("REPEAT 3 {\n  H 0\n  DEPOLARIZE1(0.01) 0\n  M 0\n}").unwrap();
    let clean = transforms::without_noise(&instrs);
    assert_eq!(clean.len(), 1);
    if let rstim::ir::StimInstr::Repeat { body, .. } = &clean[0] {
        assert_eq!(body.len(), 2);
    } else {
        panic!("expected Repeat");
    }
}

#[test]
fn without_noise_all_noise_types() {
    let instrs = parse_lines(
        "X_ERROR(0.1) 0\nY_ERROR(0.1) 0\nZ_ERROR(0.1) 0\n\
         DEPOLARIZE1(0.01) 0\nDEPOLARIZE2(0.01) 0 1\n\
         PAULI_CHANNEL_1(0.1,0,0) 0\nPAULI_CHANNEL_2(0.1,0,0,0,0,0,0,0,0,0,0,0,0,0,0) 0 1\n\
         CORRELATED_ERROR(0.1) X0\nELSE_CORRELATED_ERROR(0.1) Z0\n\
         HERALDED_ERASE(0.1) 0\nHERALDED_PAULI_CHANNEL_1(0.1,0,0,0) 0\n\
         I_ERROR(0.1) 0\nII_ERROR(0.1) 0 1\n\
         H 0"
    ).unwrap();
    let clean = transforms::without_noise(&instrs);
    assert_eq!(clean.len(), 1);
    assert_eq!(clean[0].name().unwrap(), "H");
}

#[test]
fn without_tags_removes_tags() {
    let instrs = parse_lines("H[my_tag] 0\nCX 0 1\nM[readout] 0").unwrap();
    let clean = transforms::without_tags(&instrs);
    for instr in &clean {
        if let rstim::ir::StimInstr::Op { tag, .. } = instr {
            assert!(tag.is_none());
        }
    }
}

#[test]
fn without_tags_preserves_repeat() {
    let instrs = parse_lines("REPEAT 2 {\n  H[tag] 0\n}").unwrap();
    let clean = transforms::without_tags(&instrs);
    if let rstim::ir::StimInstr::Repeat { body, .. } = &clean[0] {
        if let rstim::ir::StimInstr::Op { tag, .. } = &body[0] {
            assert!(tag.is_none());
        }
    }
}

#[test]
fn inverse_single_qubit_gates() {
    let instrs = parse_lines("S 0\nH 1").unwrap();
    let inv = transforms::inverse(&instrs).unwrap();
    assert_eq!(inv.len(), 2);
    assert_eq!(inv[0].name().unwrap(), "H");
    assert_eq!(inv[1].name().unwrap(), "S_DAG");
}

#[test]
fn inverse_two_qubit_gates() {
    let instrs = parse_lines("CX 0 1\nCZ 2 3").unwrap();
    let inv = transforms::inverse(&instrs).unwrap();
    assert_eq!(inv.len(), 2);
    assert_eq!(inv[0].name().unwrap(), "CZ");
    assert_eq!(inv[1].name().unwrap(), "CX");
}

#[test]
fn inverse_self_inverse_gates() {
    let instrs = parse_lines("H 0\nX 0\nY 0\nZ 0\nCX 0 1\nCZ 0 1\nSWAP 0 1").unwrap();
    let inv = transforms::inverse(&instrs).unwrap();
    assert_eq!(inv.len(), 7);
    assert_eq!(inv[0].name().unwrap(), "SWAP");
    assert_eq!(inv[6].name().unwrap(), "H");
}

#[test]
fn inverse_s_and_sqrt_gates() {
    let instrs = parse_lines("S 0\nSQRT_X 0\nSQRT_Y 0").unwrap();
    let inv = transforms::inverse(&instrs).unwrap();
    assert_eq!(inv[0].name().unwrap(), "SQRT_Y_DAG");
    assert_eq!(inv[1].name().unwrap(), "SQRT_X_DAG");
    assert_eq!(inv[2].name().unwrap(), "S_DAG");
}

#[test]
fn inverse_dag_gates() {
    let instrs = parse_lines("S_DAG 0\nSQRT_X_DAG 0\nSQRT_Y_DAG 0\nISWAP_DAG 0 1").unwrap();
    let inv = transforms::inverse(&instrs).unwrap();
    assert_eq!(inv[0].name().unwrap(), "ISWAP");
    assert_eq!(inv[1].name().unwrap(), "SQRT_Y");
    assert_eq!(inv[2].name().unwrap(), "SQRT_X");
    assert_eq!(inv[3].name().unwrap(), "S");
}

#[test]
fn inverse_fails_on_measurement() {
    let instrs = parse_lines("M 0").unwrap();
    assert!(transforms::inverse(&instrs).is_err());
}

#[test]
fn inverse_fails_on_noise() {
    let instrs = parse_lines("X_ERROR(0.1) 0").unwrap();
    assert!(transforms::inverse(&instrs).is_err());
}

#[test]
fn inverse_repeat_block() {
    let instrs = parse_lines("REPEAT 3 {\n  S 0\n  H 0\n}").unwrap();
    let inv = transforms::inverse(&instrs).unwrap();
    assert_eq!(inv.len(), 1);
    if let rstim::ir::StimInstr::Repeat { count, body } = &inv[0] {
        assert_eq!(*count, 3);
        assert_eq!(body[0].name().unwrap(), "H");
        assert_eq!(body[1].name().unwrap(), "S_DAG");
    } else {
        panic!("expected Repeat");
    }
}
