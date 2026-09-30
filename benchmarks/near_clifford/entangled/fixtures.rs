//! Synthetic entangled workloads and untimed independent dense-oracle checks.
use crate::oracle::DenseOracle;
use rand::{RngCore, SeedableRng, rngs::StdRng};
use rstim::near_clifford::{ActiveState, CliffordGate, MeasurementBasis, NearCliffordExecutor};
use serde_json::{Value, json};

#[derive(Clone, Copy)]
enum Op {
    H(usize),
    S(usize),
    T(usize),
    Tdag(usize),
    Cx(usize, usize),
    Cz(usize, usize),
    Measure(usize, char, bool),
    Feedback(usize),
}

fn parameters(name: &str) -> Result<(&str, usize, usize, usize), String> {
    let p = name.split('_').collect::<Vec<_>>();
    if p.len() != 4 || !["brick", "parity", "rounds"].contains(&p[0]) {
        return Err("expected FAMILY_RANK_WIDTH_DEPTH; FAMILY is brick, parity or rounds".into());
    }
    let parse = |i: usize| {
        p[i].parse::<usize>()
            .map_err(|_| "parameters must be positive integers".to_string())
    };
    let (rank, width, depth) = (parse(1)?, parse(2)?, parse(3)?);
    let data = width.saturating_sub(usize::from(p[0] != "brick"));
    if rank < 2
        || rank > 16
        || data < rank
        || width > 193
        || !(1..=8).contains(&depth)
        || (p[0] == "rounds" && rank + depth > 16)
    {
        return Err(
            "require 2<=rank<=16, rank<=data width, width<=193, 1<=depth<=8; rounds rank+depth<=16"
                .into(),
        );
    }
    Ok((p[0], rank, width, depth))
}

fn operations(name: &str) -> Result<(usize, Vec<Op>), String> {
    let (family, rank, width, depth) = parameters(name)?;
    let data = width - usize::from(family != "brick");
    let mut ops = (0..data).map(Op::H).collect::<Vec<_>>();
    for i in 0..rank {
        let q = i * (data - 1) / (rank - 1);
        ops.push(if i % 2 == 0 { Op::T(q) } else { Op::Tdag(q) });
    }
    // CZ of non-basis product inputs creates genuine entanglement; subsequent
    // noncommuting local rotations and CX layers mix measurement coordinates.
    for layer in 0..depth {
        for q in 0..data - 1 {
            ops.push(Op::Cz(q, q + 1));
        }
        for q in 0..data {
            if (q + layer) % 3 == 0 {
                ops.push(Op::S(q));
            }
            if (q + layer) % 2 == 0 {
                ops.push(Op::H(q));
            }
        }
        for q in (layer % 2..data - 1).step_by(2) {
            ops.push(if layer % 2 == 0 {
                Op::Cx(q, q + 1)
            } else {
                Op::Cx(q + 1, q)
            });
        }
        if family == "parity" {
            let ancilla = width - 1;
            for q in (0..data).step_by(2) {
                ops.push(Op::Cx(q, ancilla));
            }
            ops.push(Op::S(ancilla));
        }
        if family == "rounds" {
            let ancilla = width - 1;
            ops.push(Op::Cx(layer % data, ancilla));
            ops.push(Op::Cx((layer + data / 2) % data, ancilla));
            ops.push(Op::Measure(ancilla, 'Z', true));
            ops.push(Op::Feedback((layer + 1) % data));
            // Reinject sparse non-Clifford work after the syndrome projection.
            ops.push(Op::H((layer + 2) % data));
            ops.push(Op::T((layer + 2) % data));
        }
    }
    if family == "parity" {
        ops.push(Op::Measure(width - 1, 'X', false));
    }
    // Different bases and reverse order avoid an independent uniform suffix.
    for q in (0..data).rev() {
        ops.push(Op::Measure(q, ['X', 'Y', 'Z'][q % 3], false));
    }
    Ok((width, ops))
}

pub fn circuit(name: &str) -> Result<String, String> {
    let (_, ops) = operations(name)?;
    let mut text = String::new();
    for op in ops {
        let line = match op {
            Op::H(q) => format!("H {q}"),
            Op::S(q) => format!("S {q}"),
            Op::T(q) => format!("T {q}"),
            Op::Tdag(q) => format!("T_DAG {q}"),
            Op::Cx(a, b) => format!("CX {a} {b}"),
            Op::Cz(a, b) => format!("CZ {a} {b}"),
            Op::Feedback(q) => format!("CX rec[-1] {q}"),
            Op::Measure(q, b, reset) => format!(
                "{}{} {q}",
                if reset { "MR" } else { "M" },
                if b == 'Z' {
                    ""
                } else if b == 'X' {
                    "X"
                } else {
                    "Y"
                }
            ),
        };
        text.push_str(&line);
        text.push('\n');
    }
    Ok(text)
}

fn purity(oracle: &DenseOracle, width: usize, q: usize) -> f64 {
    let mask = 1 << (width - q - 1);
    let (mut a, mut d, mut re, mut im) = (0.0, 0.0, 0.0, 0.0);
    for (i, x) in oracle.amplitudes().iter().enumerate() {
        if i & mask != 0 {
            continue;
        }
        let y = oracle.amplitudes()[i | mask];
        a += x.norm_sqr();
        d += y.norm_sqr();
        re += x.re * y.re + x.im * y.im;
        im += x.im * y.re - x.re * y.im;
    }
    a * a + d * d + 2.0 * (re * re + im * im)
}

fn check_terminal_branch(n: usize, ops: &[Op], outcomes: &[bool]) -> Result<(), String> {
    let mut dense = DenseOracle::new(n);
    let mut index = 0;
    for &op in ops {
        match op {
            Op::H(q) => dense.h(q),
            Op::S(q) => dense.s(q),
            Op::T(q) => dense.t(q),
            Op::Tdag(q) => dense.t_dag(q),
            Op::Cx(a, b) => dense.cx(a, b),
            Op::Cz(a, b) => dense.cz(a, b),
            Op::Measure(q, b, false) => {
                let outcome = *outcomes.get(index).ok_or("compiled record is too short")?;
                let probability = dense.measurement_probability(q, b, outcome);
                if !probability.is_finite() || probability < 1e-12 {
                    return Err(
                        "compiled terminal output violates independent dense support".into(),
                    );
                }
                dense.collapse(q, b, outcome);
                index += 1;
            }
            _ => return Err("terminal support check received a mid-circuit operation".into()),
        }
    }
    if index != outcomes.len() {
        return Err("compiled record is too long".into());
    }
    Ok(())
}

pub fn validate(name: &str) -> Result<Value, String> {
    let (family, rank, width, depth) = parameters(name)?;
    // Wider circuits cannot be checked with a full 2^width dense state.
    // Record explicitly when a reduced family witness is used instead.
    let reference_name = if width <= 16 {
        name.to_string()
    } else {
        format!(
            "{family}_{}_{}_{}",
            rank.min(8),
            if family == "brick" { 8 } else { 9 },
            depth
        )
    };
    let (n, ops) = operations(&reference_name)?;
    let text = circuit(&reference_name)?;
    let executor = NearCliffordExecutor::compile_text(&text)?;
    let (mut comparisons, mut minimum_purity, mut peak_rank) = (0usize, 1.0f64, 0usize);
    for seed in [739, 20261001, 20261002] {
        let mut state = ActiveState::new(n, 16);
        let mut dense = DenseOracle::new(n);
        let mut rng = StdRng::seed_from_u64(seed);
        let mut record = Vec::new();
        for &op in &ops {
            match op {
                Op::H(q) => {
                    state.apply_clifford(CliffordGate::H(q))?;
                    dense.h(q);
                }
                Op::S(q) => {
                    state.apply_clifford(CliffordGate::S(q))?;
                    dense.s(q);
                }
                Op::T(q) => {
                    state.t(q)?;
                    dense.t(q);
                }
                Op::Tdag(q) => {
                    state.t_dag(q)?;
                    dense.t_dag(q);
                }
                Op::Cx(a, b) => {
                    state.apply_clifford(CliffordGate::CX(a, b))?;
                    dense.cx(a, b);
                }
                Op::Cz(a, b) => {
                    state.apply_clifford(CliffordGate::CZ(a, b))?;
                    dense.cz(a, b);
                }
                Op::Feedback(q) => {
                    if *record.last().ok_or("feedback has no measurement")? {
                        state.apply_clifford(CliffordGate::X(q))?;
                        dense.x(q);
                    }
                }
                Op::Measure(q, b, reset) => {
                    for (basis, letter) in [
                        (MeasurementBasis::X, 'X'),
                        (MeasurementBasis::Y, 'Y'),
                        (MeasurementBasis::Z, 'Z'),
                    ] {
                        let expected = dense.measurement_probability(q, letter, false);
                        let (zero, one) = state.measurement_probabilities(q, basis)?;
                        if !((zero - expected).abs() <= 1e-10
                            && (one - (1.0 - expected)).abs() <= 1e-10)
                        {
                            return Err(format!(
                                "dense probability mismatch {reference_name}: seed={seed}, q={q}, basis={letter}"
                            ));
                        }
                        comparisons += 1;
                    }
                    let basis = match b {
                        'X' => MeasurementBasis::X,
                        'Y' => MeasurementBasis::Y,
                        _ => MeasurementBasis::Z,
                    };
                    let outcome = if reset {
                        state.measure_reset(q, basis, &mut rng)?
                    } else {
                        state.measure(q, basis, &mut rng)?
                    };
                    if !reset {
                        state.retire_fixed_axes();
                    }
                    dense.collapse(q, b, outcome);
                    if reset && outcome {
                        if b == 'Z' {
                            dense.x(q);
                        } else {
                            dense.z(q);
                        }
                    }
                    record.push(outcome);
                }
            }
            peak_rank = peak_rank.max(state.active_rank());
            if matches!(op, Op::Cz(_, _)) {
                minimum_purity = minimum_purity.min(purity(&dense, n, 0));
            }
        }
        let continuation = rng.next_u64();
        let mut compiled_rng = StdRng::seed_from_u64(seed);
        let compiled = executor.run(&mut compiled_rng)?;
        let compiled_continuation = compiled_rng.next_u64();
        if family != "rounds" {
            check_terminal_branch(n, &ops, &compiled.measurements)?;
        }
        // Terminal planning may reorder commuting measurements before restoring
        // record order, so per-seed manual equality is only an unplanned contract.
        if family == "rounds"
            && (compiled.measurements != record || compiled_continuation != continuation)
        {
            return Err(format!(
                "compiled/manual outputs or RNG differ: {reference_name}, seed={seed}, compiled={:?}, manual={record:?}, next={compiled_continuation}/{continuation}",
                compiled.measurements
            ));
        }
    }
    if minimum_purity >= 0.99 {
        return Err(format!(
            "witness did not exhibit entanglement: {reference_name}"
        ));
    }
    Ok(
        json!({"reference_fixture":reference_name,"full_width_oracle":width<=16,
        "born_probability_comparisons":comparisons,"minimum_one_qubit_purity":minimum_purity,
        "reference_peak_rank":peak_rank,"trajectories":3,"manual_executor_seed_equality":family=="rounds","terminal_support_checks":if family=="rounds" {0} else {3},"status":"pass"}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_invalid_parameters_instead_of_building_a_different_workload() {
        for n in [
            "unknown_8_8_3",
            "brick_1_8_3",
            "brick_8_7_3",
            "parity_8_8_3",
            "rounds_12_13_8",
            "brick_8_8_0",
            "brick_8_194_3",
            "brick_8_8_3_extra",
        ] {
            assert!(circuit(n).is_err(), "{n}");
        }
    }
    #[test]
    fn independent_oracle_checks_entanglement_and_conditional_measurements_in_each_family() {
        for n in [
            "brick_8_8_3",
            "brick_16_16_3",
            "parity_8_9_3",
            "rounds_4_5_2",
            "rounds_8_9_8",
        ] {
            let v = validate(n).unwrap();
            assert_eq!(v["status"], "pass");
            assert!(v["minimum_one_qubit_purity"].as_f64().unwrap() < 0.99);
        }
    }
}
