use rstim::ir::{StimInstr, StimTarget};
use rstim::parser::parse_lines;
use rstim::sim::packed_inverse_tableau::{CanonicalTableauSnapshot, PackedInverseTableau};
use rstim::sim::tableau::StabilizerState;

fn apply_packed_circuit(num_qubits: usize, circuit: &str) -> (Vec<bool>, CanonicalTableauSnapshot) {
    let instrs = parse_lines(circuit).expect("test circuit parses");
    let mut tableau = PackedInverseTableau::identity(num_qubits);
    let mut measurements = Vec::new();
    apply_packed_instrs(&mut tableau, &instrs, &mut measurements);
    (measurements, tableau.canonical_snapshot())
}

fn apply_legacy_circuit(num_qubits: usize, circuit: &str) -> (Vec<bool>, CanonicalTableauSnapshot) {
    let instrs = parse_lines(circuit).expect("test circuit parses");
    let mut state = StabilizerState::new(num_qubits);
    let mut measurements = Vec::new();
    apply_legacy_instrs(&mut state, &instrs, &mut measurements);
    (measurements, state.canonical_snapshot())
}

fn apply_packed_instrs(
    tableau: &mut PackedInverseTableau,
    instrs: &[StimInstr],
    measurements: &mut Vec<bool>,
) {
    for instr in instrs {
        match instr {
            StimInstr::Repeat { count, body } => {
                for _ in 0..*count {
                    apply_packed_instrs(tableau, body, measurements);
                }
            }
            StimInstr::Op { name, targets, .. } => {
                apply_packed_op(tableau, name, targets, measurements)
            }
        }
    }
}

fn apply_packed_op(
    tableau: &mut PackedInverseTableau,
    name: &str,
    targets: &[StimTarget],
    measurements: &mut Vec<bool>,
) {
    match name {
        "H" => {
            for q in plain_qubits(targets) {
                tableau.h(q);
            }
        }
        "S" => {
            for q in plain_qubits(targets) {
                tableau.s(q);
            }
        }
        "S_DAG" => {
            for q in plain_qubits(targets) {
                tableau.s_dag(q);
            }
        }
        "X" => {
            for q in plain_qubits(targets) {
                tableau.x_gate(q);
            }
        }
        "Y" => {
            for q in plain_qubits(targets) {
                tableau.y_gate(q);
            }
        }
        "Z" => {
            for q in plain_qubits(targets) {
                tableau.z_gate(q);
            }
        }
        "CX" => {
            for (c, t) in plain_pairs(targets) {
                tableau.cx(c, t);
            }
        }
        "M" | "MZ" => {
            for (q, inv) in qubits_with_inversion(targets) {
                measurements.push(tableau.measure_z_biased(q, inv));
            }
        }
        "MX" => {
            for (q, inv) in qubits_with_inversion(targets) {
                measurements.push(tableau.measure_x_biased(q, inv));
            }
        }
        "MY" => {
            for (q, inv) in qubits_with_inversion(targets) {
                measurements.push(tableau.measure_y_biased(q, inv));
            }
        }
        "MR" | "MRZ" => {
            for (q, inv) in qubits_with_inversion(targets) {
                measurements.push(tableau.measure_reset_z_biased(q, inv));
            }
        }
        "MRX" => {
            for (q, inv) in qubits_with_inversion(targets) {
                measurements.push(tableau.measure_reset_x_biased(q, inv));
            }
        }
        "MRY" => {
            for (q, inv) in qubits_with_inversion(targets) {
                measurements.push(tableau.measure_reset_y_biased(q, inv));
            }
        }
        "R" | "RZ" => {
            for q in plain_qubits(targets) {
                tableau.reset_z_biased(q);
            }
        }
        "RX" => {
            for q in plain_qubits(targets) {
                tableau.reset_x_biased(q);
            }
        }
        "RY" => {
            for q in plain_qubits(targets) {
                tableau.reset_y_biased(q);
            }
        }
        other => panic!("unsupported packed test operation {other}"),
    }
}

fn apply_legacy_instrs(
    state: &mut StabilizerState,
    instrs: &[StimInstr],
    measurements: &mut Vec<bool>,
) {
    for instr in instrs {
        match instr {
            StimInstr::Repeat { count, body } => {
                for _ in 0..*count {
                    apply_legacy_instrs(state, body, measurements);
                }
            }
            StimInstr::Op { name, targets, .. } => {
                apply_legacy_op(state, name, targets, measurements)
            }
        }
    }
}

fn apply_legacy_op(
    state: &mut StabilizerState,
    name: &str,
    targets: &[StimTarget],
    measurements: &mut Vec<bool>,
) {
    match name {
        "H" => {
            for q in plain_qubits(targets) {
                state.h(q);
            }
        }
        "S" => {
            for q in plain_qubits(targets) {
                state.s(q);
            }
        }
        "S_DAG" => {
            for q in plain_qubits(targets) {
                state.s_dag(q);
            }
        }
        "X" => {
            for q in plain_qubits(targets) {
                state.x_gate(q);
            }
        }
        "Y" => {
            for q in plain_qubits(targets) {
                state.y_gate(q);
            }
        }
        "Z" => {
            for q in plain_qubits(targets) {
                state.z_gate(q);
            }
        }
        "CX" => {
            for (c, t) in plain_pairs(targets) {
                state.cx(c, t);
            }
        }
        "M" | "MZ" => {
            for (q, inv) in qubits_with_inversion(targets) {
                measurements.push(legacy_measure_z(state, q, inv));
            }
        }
        "MX" => {
            for (q, inv) in qubits_with_inversion(targets) {
                measurements.push(legacy_measure_x(state, q, inv));
            }
        }
        "MY" => {
            for (q, inv) in qubits_with_inversion(targets) {
                measurements.push(legacy_measure_y(state, q, inv));
            }
        }
        "MR" | "MRZ" => {
            for (q, inv) in qubits_with_inversion(targets) {
                measurements.push(legacy_measure_reset_z(state, q, inv));
            }
        }
        "MRX" => {
            for (q, inv) in qubits_with_inversion(targets) {
                measurements.push(legacy_measure_reset_x(state, q, inv));
            }
        }
        "MRY" => {
            for (q, inv) in qubits_with_inversion(targets) {
                measurements.push(legacy_measure_reset_y(state, q, inv));
            }
        }
        "R" | "RZ" => {
            for q in plain_qubits(targets) {
                state.reset_z_biased(q);
            }
        }
        "RX" => {
            for q in plain_qubits(targets) {
                state.reset_x_biased(q);
            }
        }
        "RY" => {
            for q in plain_qubits(targets) {
                state.reset_y_biased(q);
            }
        }
        other => panic!("unsupported legacy test operation {other}"),
    }
}

fn legacy_measure_z(state: &mut StabilizerState, q: usize, inv: bool) -> bool {
    (state.measure_z_biased(q) == 1) ^ inv
}

fn legacy_measure_x(state: &mut StabilizerState, q: usize, inv: bool) -> bool {
    state.h(q);
    let bit = legacy_measure_z(state, q, inv);
    state.h(q);
    bit
}

fn legacy_measure_y(state: &mut StabilizerState, q: usize, inv: bool) -> bool {
    state.s_dag(q);
    state.h(q);
    let bit = legacy_measure_z(state, q, inv);
    state.h(q);
    state.s(q);
    bit
}

fn legacy_measure_reset_z(state: &mut StabilizerState, q: usize, inv: bool) -> bool {
    let raw = state.measure_z_biased(q) == 1;
    if raw {
        state.x_gate(q);
    }
    raw ^ inv
}

fn legacy_measure_reset_x(state: &mut StabilizerState, q: usize, inv: bool) -> bool {
    state.h(q);
    let bit = legacy_measure_reset_z(state, q, inv);
    state.h(q);
    bit
}

fn legacy_measure_reset_y(state: &mut StabilizerState, q: usize, inv: bool) -> bool {
    state.s_dag(q);
    state.h(q);
    let bit = legacy_measure_reset_z(state, q, inv);
    state.h(q);
    state.s(q);
    bit
}

fn plain_qubits(targets: &[StimTarget]) -> Vec<usize> {
    targets
        .iter()
        .map(|target| match target {
            StimTarget::Qubit(q) => *q as usize,
            other => panic!("expected plain qubit target, got {other:?}"),
        })
        .collect()
}

fn qubits_with_inversion(targets: &[StimTarget]) -> Vec<(usize, bool)> {
    targets
        .iter()
        .map(|target| match target {
            StimTarget::Qubit(q) => (*q as usize, false),
            StimTarget::QubitInv(q) => (*q as usize, true),
            other => panic!("expected measurement qubit target, got {other:?}"),
        })
        .collect()
}

fn plain_pairs(targets: &[StimTarget]) -> Vec<(usize, usize)> {
    let qubits = plain_qubits(targets);
    assert_eq!(
        qubits.len() % 2,
        0,
        "pair operation requires even target count"
    );
    qubits
        .chunks_exact(2)
        .map(|pair| (pair[0], pair[1]))
        .collect()
}

#[test]
fn packed_measurement_known_answers() {
    let cases = [
        (1, "M 0\n", vec![false]),
        (1, "X 0\nM 0\n", vec![true]),
        (1, "H 0\nMX 0\n", vec![false]),
        (1, "H 0\nZ 0\nMX 0\n", vec![true]),
        (1, "H 0\nS 0\nMY 0\n", vec![false]),
        (1, "H 0\nS_DAG 0\nMY 0\n", vec![true]),
        (1, "X 0\nMR 0\nM 0\n", vec![true, false]),
        (1, "H 0\nZ 0\nMRX 0\nMX 0\n", vec![true, false]),
        (1, "H 0\nS_DAG 0\nMRY 0\nMY 0\n", vec![true, false]),
        (
            3,
            "X 0\nR 0\nM 0\nRX 1\nMX 1\nRY 2\nMY 2\n",
            vec![false, false, false],
        ),
        (2, "H 0\nCX 0 1\nM 0 1\n", vec![false, false]),
        (
            130,
            "H 63\nCX 63 64\nM 63 64\nH 64\nCX 64 129\nM 64 129\n",
            vec![false, false, false, false],
        ),
    ];

    for (num_qubits, circuit, expected) in cases {
        let (bits, _) = apply_packed_circuit(num_qubits, circuit);
        assert_eq!(bits, expected, "circuit:\n{circuit}");
    }
}

#[test]
fn inverted_measurement_target_only_flips_reported_bit() {
    let (bits, snapshot) = apply_packed_circuit(1, "X 0\nMR !0\nM 0\n");
    assert_eq!(bits, vec![false, false]);

    let (_, expected_snapshot) = apply_packed_circuit(1, "X 0\nMR 0\nM 0\n");
    assert_eq!(snapshot, expected_snapshot);
}

#[test]
fn packed_and_legacy_measurement_sequence_match() {
    let circuit = deterministic_measurement_sequence(0x457, 130, 512);
    let (packed_bits, packed_snapshot) = apply_packed_circuit(130, &circuit);
    let (legacy_bits, legacy_snapshot) = apply_legacy_circuit(130, &circuit);
    assert_eq!(packed_bits, legacy_bits);
    assert_eq!(packed_snapshot, legacy_snapshot);
    println!("PASS packed inverse measurement and reset");
}

fn deterministic_measurement_sequence(seed: u64, num_qubits: usize, len: usize) -> String {
    let mut lines = vec![
        "H 0".to_string(),
        "S 1".to_string(),
        "S_DAG 2".to_string(),
        "X 3".to_string(),
        "Y 4".to_string(),
        "Z 5".to_string(),
        "CX 6 7".to_string(),
        "M 0".to_string(),
        "MX 1".to_string(),
        "MY 2".to_string(),
        "MR 3".to_string(),
        "MRX 4".to_string(),
        "MRY 5".to_string(),
        "R 6".to_string(),
        "RX 7".to_string(),
        "RY 8".to_string(),
    ];
    let mut state = seed;
    while lines.len() < len {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let q = ((state >> 17) as usize) % num_qubits;
        let q2 = ((state >> 31) as usize) % (num_qubits - 1);
        let t = if q2 >= q { q2 + 1 } else { q2 };
        let inv = if ((state >> 9) & 1) == 1 { "!" } else { "" };
        lines.push(match ((state >> 61) % 15) as u8 {
            0 => format!("H {q}"),
            1 => format!("S {q}"),
            2 => format!("S_DAG {q}"),
            3 => format!("X {q}"),
            4 => format!("Y {q}"),
            5 => format!("Z {q}"),
            6 => format!("CX {q} {t}"),
            7 => format!("M {inv}{q}"),
            8 => format!("MX {inv}{q}"),
            9 => format!("MY {inv}{q}"),
            10 => format!("MR {inv}{q}"),
            11 => format!("MRX {inv}{q}"),
            12 => format!("MRY {inv}{q}"),
            13 => format!("R {q}"),
            14 => format!("RX {q}"),
            _ => format!("RY {q}"),
        });
    }
    lines.push(String::new());
    lines.join("\n")
}
