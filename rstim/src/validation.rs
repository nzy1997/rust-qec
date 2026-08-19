use crate::ir::{StimInstr, StimTarget};

/// Parse circuit text and validate its instruction semantics without executing it.
pub fn parse_and_validate(text: &str) -> Result<Vec<StimInstr>, String> {
    let instrs = crate::parser::parse_lines(text)?;
    validate_circuit(&instrs)?;
    Ok(instrs)
}

/// Validate instruction names, arguments, target shapes, and record lookbacks.
pub fn validate_circuit(instrs: &[StimInstr]) -> Result<(), String> {
    let mut state = ValidationState::default();
    validate_block(instrs, &mut state)
}

#[derive(Clone, Copy, Default)]
struct ValidationState {
    measurements: usize,
    correlated_chain_open: bool,
}

fn validate_block(instrs: &[StimInstr], state: &mut ValidationState) -> Result<(), String> {
    for instr in instrs {
        match instr {
            StimInstr::Op {
                name,
                args,
                targets,
                ..
            } => validate_operation(name, args, targets, state).map_err(|message| {
                if message.starts_with("unsupported instruction") {
                    message
                } else {
                    format!("{name}: {message}")
                }
            })?,
            StimInstr::Repeat { count, body } => {
                let before = state.measurements;
                let mut body_state = ValidationState {
                    measurements: before,
                    correlated_chain_open: false,
                };
                validate_block(body, &mut body_state)?;
                let per_iteration = body_state
                    .measurements
                    .checked_sub(before)
                    .ok_or_else(|| "measurement count underflow".to_string())?;
                let repeat_count = usize::try_from(*count)
                    .map_err(|_| "repeat count does not fit this platform".to_string())?;
                let repeated = per_iteration
                    .checked_mul(repeat_count)
                    .ok_or_else(|| "measurement count overflow".to_string())?;
                state.measurements = before
                    .checked_add(repeated)
                    .ok_or_else(|| "measurement count overflow".to_string())?;
                state.correlated_chain_open = false;
            }
        }
    }
    Ok(())
}

fn validate_operation(
    name: &str,
    args: &[f64],
    targets: &[StimTarget],
    state: &mut ValidationState,
) -> Result<(), String> {
    let mut measurements = 0;
    let continues_correlated_chain = matches!(name, "ELSE_CORRELATED_ERROR");
    match name {
        "I" | "H" | "H_XY" | "H_YZ" | "S" | "SQRT_Z" | "S_DAG" | "SQRT_Z_DAG" | "SQRT_X"
        | "SQRT_X_DAG" | "SQRT_Y" | "SQRT_Y_DAG" | "X" | "Y" | "Z" | "C_XYZ" | "C_ZYX"
        | "C_NXYZ" | "C_NZYX" | "C_XNYZ" | "C_XYNZ" | "C_ZNYX" | "C_ZYNX" | "H_NXY" | "H_NXZ"
        | "H_NYZ" => {
            expect_arg_count(name, args, 0)?;
            expect_qubit_targets(name, targets, false)?;
        }
        "I_ERROR" => {
            expect_probability_args(name, args, 1)?;
            expect_qubit_targets(name, targets, false)?;
        }
        "II_ERROR" => {
            expect_probability_args(name, args, 1)?;
            expect_qubit_pairs(name, targets, false)?;
        }
        "CX" | "CNOT" | "ZCX" | "CY" | "ZCY" | "CZ" | "ZCZ" => {
            expect_arg_count(name, args, 0)?;
            validate_controlled_pairs(name, targets, state.measurements)?;
        }
        "XCX" | "XCY" | "XCZ" | "YCX" | "YCY" | "YCZ" | "SWAP" | "ISWAP" | "ISWAP_DAG"
        | "CXSWAP" | "SWAPCX" | "CZSWAP" => {
            expect_arg_count(name, args, 0)?;
            expect_qubit_pairs(name, targets, false)?;
        }
        "M" | "MZ" | "MX" | "MY" | "MR" | "MRZ" | "MRX" | "MRY" => {
            expect_optional_probability(name, args)?;
            measurements = validate_measurement_targets(targets)?;
        }
        "ML" | "MZL" | "MXL" | "MYL" | "MRL" | "MRZL" | "MRXL" | "MRYL" => {
            expect_optional_probability(name, args)?;
            measurements = validate_measurement_targets(targets)?
                .checked_mul(2)
                .ok_or_else(|| "measurement count overflow".to_string())?;
        }
        "MPAD" => {
            expect_optional_probability(name, args)?;
            for target in targets {
                match target {
                    StimTarget::Qubit(0 | 1) => {}
                    _ => return Err("targets must be literal bits 0 or 1".to_string()),
                }
            }
            measurements = targets.len();
        }
        "R" | "RZ" | "RX" | "RY" => {
            expect_arg_count(name, args, 0)?;
            expect_qubit_targets(name, targets, false)?;
        }
        "LOSS" | "X_ERROR" | "Y_ERROR" | "Z_ERROR" | "DEPOLARIZE1" | "HERALDED_ERASE" => {
            expect_probability_args(name, args, 1)?;
            expect_qubit_targets(name, targets, false)?;
            if name == "HERALDED_ERASE" {
                measurements = targets.len();
            }
        }
        "DEPOLARIZE2" => {
            expect_probability_args(name, args, 1)?;
            expect_qubit_pairs(name, targets, false)?;
        }
        "QUBIT_COORDS" => {
            expect_finite_args(name, args)?;
            expect_qubit_targets(name, targets, false)?;
        }
        "SHIFT_COORDS" => {
            expect_finite_args(name, args)?;
            expect_no_targets(name, targets)?;
        }
        "TICK" => {
            expect_arg_count(name, args, 0)?;
            expect_no_targets(name, targets)?;
        }
        "DETECTOR" => {
            expect_finite_args(name, args)?;
            validate_record_targets(name, targets, state.measurements)?;
        }
        "OBSERVABLE_INCLUDE" => {
            expect_observable_index(args)?;
            validate_record_targets(name, targets, state.measurements)?;
        }
        "MXX" | "MYY" | "MZZ" => {
            expect_optional_probability(name, args)?;
            expect_qubit_pairs(name, targets, true)?;
            measurements = targets.len() / 2;
        }
        "MPP" => {
            expect_optional_probability(name, args)?;
            measurements = validate_pauli_products(name, targets)?;
        }
        "SPP" | "SPP_DAG" => {
            expect_arg_count(name, args, 0)?;
            validate_pauli_products(name, targets)?;
        }
        "PAULI_CHANNEL_1" => {
            expect_probability_distribution(name, args, 3)?;
            expect_qubit_targets(name, targets, false)?;
        }
        "PAULI_CHANNEL_2" => {
            expect_probability_distribution(name, args, 15)?;
            expect_qubit_pairs(name, targets, false)?;
        }
        "HERALDED_PAULI_CHANNEL_1" => {
            expect_probability_distribution(name, args, 4)?;
            expect_qubit_targets(name, targets, false)?;
            measurements = targets.len();
        }
        "CORRELATED_ERROR" | "E" | "ELSE_CORRELATED_ERROR" => {
            expect_probability_args(name, args, 1)?;
            expect_pauli_targets(name, targets)?;
            if name == "ELSE_CORRELATED_ERROR" && !state.correlated_chain_open {
                return Err(
                    "must immediately follow CORRELATED_ERROR, E, or ELSE_CORRELATED_ERROR"
                        .to_string(),
                );
            }
        }
        _ => return Err(format!("unsupported instruction {name}")),
    }

    state.measurements = state
        .measurements
        .checked_add(measurements)
        .ok_or_else(|| "measurement count overflow".to_string())?;
    state.correlated_chain_open = matches!(name, "CORRELATED_ERROR" | "E")
        || (continues_correlated_chain && state.correlated_chain_open);
    Ok(())
}

fn expect_arg_count(name: &str, args: &[f64], expected: usize) -> Result<(), String> {
    if args.len() != expected {
        return Err(format!(
            "expected {expected} argument(s), got {}",
            args.len()
        ));
    }
    expect_finite_args(name, args)
}

fn expect_finite_args(_name: &str, args: &[f64]) -> Result<(), String> {
    if args.iter().any(|value| !value.is_finite()) {
        return Err("arguments must be finite".to_string());
    }
    Ok(())
}

fn expect_optional_probability(name: &str, args: &[f64]) -> Result<(), String> {
    if args.len() > 1 {
        return Err(format!(
            "expected zero or one probability argument, got {}",
            args.len()
        ));
    }
    if let Some(probability) = args.first() {
        expect_probability(name, *probability)?;
    }
    Ok(())
}

fn expect_probability_args(name: &str, args: &[f64], expected: usize) -> Result<(), String> {
    expect_arg_count(name, args, expected)?;
    for probability in args {
        expect_probability(name, *probability)?;
    }
    Ok(())
}

fn expect_probability(_name: &str, probability: f64) -> Result<(), String> {
    if !probability.is_finite() || !(0.0..=1.0).contains(&probability) {
        return Err(format!(
            "probability must be finite and in [0, 1], got {probability}"
        ));
    }
    Ok(())
}

fn expect_probability_distribution(
    name: &str,
    args: &[f64],
    expected: usize,
) -> Result<(), String> {
    expect_probability_args(name, args, expected)?;
    if args.iter().sum::<f64>() > 1.0 + 1e-12 {
        return Err("probabilities must sum to at most 1".to_string());
    }
    Ok(())
}

fn expect_qubit_targets(
    _name: &str,
    targets: &[StimTarget],
    allow_inverted: bool,
) -> Result<(), String> {
    for target in targets {
        match target {
            StimTarget::Qubit(_) => {}
            StimTarget::QubitInv(_) if allow_inverted => {}
            StimTarget::QubitInv(_) => {
                return Err("inverted qubit target is not allowed".to_string());
            }
            _ => return Err("expected qubit target".to_string()),
        }
    }
    Ok(())
}

fn validate_measurement_targets(targets: &[StimTarget]) -> Result<usize, String> {
    let mut measurements = 0;
    for target in targets {
        match target {
            StimTarget::Qubit(_) | StimTarget::QubitInv(_) => measurements += 1,
            StimTarget::Sweep(_) => {}
            _ => return Err("expected qubit target".to_string()),
        }
    }
    Ok(measurements)
}

fn expect_qubit_pairs(
    name: &str,
    targets: &[StimTarget],
    allow_inverted: bool,
) -> Result<(), String> {
    if !targets.len().is_multiple_of(2) {
        return Err("expected an even number of targets".to_string());
    }
    expect_qubit_targets(name, targets, allow_inverted)
}

fn validate_controlled_pairs(
    _name: &str,
    targets: &[StimTarget],
    measurements: usize,
) -> Result<(), String> {
    if !targets.len().is_multiple_of(2) {
        return Err("expected an even number of targets".to_string());
    }
    for pair in targets.chunks_exact(2) {
        match (&pair[0], &pair[1]) {
            (StimTarget::Qubit(_), StimTarget::Qubit(_))
            | (StimTarget::Sweep(_), StimTarget::Qubit(_)) => {}
            (StimTarget::Rec(offset), StimTarget::Qubit(_)) => {
                validate_record_offset(*offset, measurements)?;
            }
            _ => {
                return Err(
                    "expected qubit pair or rec[]/sweep[] control followed by qubit".to_string(),
                );
            }
        }
    }
    Ok(())
}

fn validate_record_targets(
    _name: &str,
    targets: &[StimTarget],
    measurements: usize,
) -> Result<(), String> {
    for target in targets {
        match target {
            StimTarget::Rec(offset) => validate_record_offset(*offset, measurements)?,
            _ => return Err("expected rec[] target".to_string()),
        }
    }
    Ok(())
}

fn validate_record_offset(offset: i32, measurements: usize) -> Result<(), String> {
    let lookback = offset.unsigned_abs() as usize;
    if offset >= 0 || lookback == 0 || lookback > measurements {
        return Err(format!(
            "rec[{offset}] is out of range with {measurements} measurement(s) available"
        ));
    }
    Ok(())
}

fn expect_no_targets(_name: &str, targets: &[StimTarget]) -> Result<(), String> {
    if !targets.is_empty() {
        return Err(format!("expected no targets, got {}", targets.len()));
    }
    Ok(())
}

fn expect_observable_index(args: &[f64]) -> Result<(), String> {
    expect_arg_count("OBSERVABLE_INCLUDE", args, 1)?;
    let value = args[0];
    if value < 0.0 || value.fract() != 0.0 || value > u32::MAX as f64 {
        return Err(format!(
            "observable index must be an integer in [0, {}], got {value}",
            u32::MAX
        ));
    }
    Ok(())
}

fn validate_pauli_products(name: &str, targets: &[StimTarget]) -> Result<usize, String> {
    if targets.is_empty() {
        return Ok(0);
    }
    let mut products = 0;
    let mut after_combiner = false;
    for target in targets {
        match target {
            StimTarget::Pauli { inverted, .. } => {
                let starts_product = !after_combiner;
                if *inverted && !starts_product {
                    return Err(
                        "only the first Pauli target in a product may be inverted".to_string()
                    );
                }
                if starts_product {
                    products += 1;
                }
                after_combiner = false;
            }
            StimTarget::Combiner if products > 0 && !after_combiner => {
                after_combiner = true;
            }
            StimTarget::Combiner => return Err("misplaced Pauli combiner".to_string()),
            _ => return Err(format!("{name} targets must be Pauli targets")),
        }
    }
    if after_combiner {
        return Err("Pauli product cannot end with a combiner".to_string());
    }
    Ok(products)
}

fn expect_pauli_targets(_name: &str, targets: &[StimTarget]) -> Result<(), String> {
    for target in targets {
        match target {
            StimTarget::Pauli {
                inverted: false, ..
            } => {}
            StimTarget::Pauli { inverted: true, .. } => {
                return Err("inverted Pauli targets are not allowed".to_string());
            }
            _ => return Err("expected Pauli target".to_string()),
        }
    }
    Ok(())
}
