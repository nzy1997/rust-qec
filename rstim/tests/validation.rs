use rstim::executor::Executor;
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
