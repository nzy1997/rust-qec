use rstim::executor::Executor;
use rstim::ir::{PauliBasis, StimInstr, StimTarget};
use rstim::parser::parse_lines;
use rstim::validation::{parse_and_validate, validate_circuit};

#[test]
fn accepts_supported_structural_forms_without_executing() {
    let circuit = concat!(
        "QUBIT_COORDS(0,0) 0 1\n",
        "H 0\n",
        "CX 0 1\n",
        "CX sweep[2] 1\n",
        "M 0 1\n",
        "DETECTOR rec[-1] rec[-2]\n",
        "OBSERVABLE_INCLUDE(0) rec[-1]\n",
        "MPP X0*X1 Z0\n",
        "PAULI_CHANNEL_1(0.1,0.2,0.3) 0\n",
        "REPEAT 2 {\n",
        "  M 0\n",
        "  DETECTOR rec[-1]\n",
        "}\n",
    );
    assert!(parse_and_validate(circuit).is_ok());
}

#[test]
fn rejects_wrong_arity_target_types_and_probability_ranges() {
    let invalid = [
        ("CX 0\n", "even number"),
        ("M rec[-1]\n", "expected qubit target"),
        ("H !0\n", "inverted qubit"),
        ("TICK 0\n", "expected no targets"),
        ("M(0.1,0.2) 0\n", "zero or one"),
        ("X_ERROR(1.1) 0\n", "in [0, 1]"),
        ("PAULI_CHANNEL_1(0.5,0.5,0.5) 0\n", "sum to at most 1"),
        ("MPAD 2\n", "literal bits"),
    ];
    for (circuit, expected) in invalid {
        let error = parse_and_validate(circuit).unwrap_err();
        assert!(
            error.contains(expected),
            "expected {expected:?} in {error:?} for {circuit:?}"
        );
    }
}

#[test]
fn rejects_out_of_range_record_lookbacks() {
    let error = parse_and_validate("M 0\nDETECTOR rec[-2]\n").unwrap_err();
    assert!(error.contains("out of range"));
}

#[test]
fn executor_uses_the_same_preflight_validator() {
    let instrs = parse_lines("CX 0\n").unwrap();
    let validation_error = validate_circuit(&instrs).unwrap_err();
    let executor_error = Executor::from_instrs(instrs).err().unwrap();
    assert_eq!(executor_error, validation_error);
}

#[test]
fn rejects_additional_argument_target_and_chain_errors() {
    let invalid = [
        ("ELSE_CORRELATED_ERROR(0.1) X0\n", "must immediately follow"),
        ("H(1) 0\n", "expected 0 argument"),
        ("DEPOLARIZE2(0.1) 0\n", "even number"),
        ("CX 0 rec[-1]\n", "expected qubit pair"),
        ("DETECTOR 0\n", "expected rec[] target"),
        ("H\n", "expected at least one target"),
        ("OBSERVABLE_INCLUDE(-1)\n", "observable index"),
        ("MPP\n", "at least one Pauli product"),
        ("CORRELATED_ERROR(0.1)\n", "at least one Pauli target"),
        ("CORRELATED_ERROR(0.1) !X0\n", "inverted Pauli"),
    ];
    for (circuit, expected) in invalid {
        let error = parse_and_validate(circuit).unwrap_err();
        assert!(
            error.contains(expected),
            "expected {expected:?} in {error:?} for {circuit:?}"
        );
    }

    let direct_invalid = [
        (
            StimInstr::new("QUBIT_COORDS", vec![f64::NAN], vec![StimTarget::Qubit(0)]),
            "finite",
        ),
        (
            StimInstr::new(
                "MPP",
                vec![],
                vec![
                    StimTarget::pauli(0, PauliBasis::X, false),
                    StimTarget::Combiner,
                    StimTarget::pauli(1, PauliBasis::X, true),
                ],
            ),
            "only the first Pauli target",
        ),
        (
            StimInstr::new("MPP", vec![], vec![StimTarget::Combiner]),
            "misplaced Pauli combiner",
        ),
        (
            StimInstr::new(
                "MPP",
                vec![],
                vec![
                    StimTarget::pauli(0, PauliBasis::Z, false),
                    StimTarget::Combiner,
                ],
            ),
            "cannot end with a combiner",
        ),
        (
            StimInstr::new("MPP", vec![], vec![StimTarget::Qubit(0)]),
            "must be Pauli targets",
        ),
        (
            StimInstr::new("CORRELATED_ERROR", vec![0.1], vec![StimTarget::Qubit(0)]),
            "expected Pauli target",
        ),
    ];
    for (instruction, expected) in direct_invalid {
        let error = validate_circuit(&[instruction]).unwrap_err();
        assert!(
            error.contains(expected),
            "expected {expected:?} in {error:?}"
        );
    }
}
