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
