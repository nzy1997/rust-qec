use rstim::sim::packed_inverse_tableau::{CanonicalTableauSnapshot, PackedInverseTableau};
use rstim::sim::tableau::StabilizerState;

const AUDITED_TABLEAU_LEN: usize = 12_248;
const AUDITED_TABLEAU_FNV1A64: u64 = 0x855e_a7dc_4a84_99c6;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Gate {
    H(usize),
    S(usize),
    SDag(usize),
    X(usize),
    Y(usize),
    Z(usize),
    Cx(usize, usize),
}

fn apply_legacy(state: &mut StabilizerState, gate: Gate) {
    match gate {
        Gate::H(q) => state.h(q),
        Gate::S(q) => state.s(q),
        Gate::SDag(q) => state.s_dag(q),
        Gate::X(q) => state.x_gate(q),
        Gate::Y(q) => state.y_gate(q),
        Gate::Z(q) => state.z_gate(q),
        Gate::Cx(c, t) => state.cx(c, t),
    }
}

fn apply_packed(tableau: &mut PackedInverseTableau, gate: Gate) {
    match gate {
        Gate::H(q) => tableau.h(q),
        Gate::S(q) => tableau.s(q),
        Gate::SDag(q) => tableau.s_dag(q),
        Gate::X(q) => tableau.x_gate(q),
        Gate::Y(q) => tableau.y_gate(q),
        Gate::Z(q) => tableau.z_gate(q),
        Gate::Cx(c, t) => tableau.cx(c, t),
    }
}

fn assert_matches_after_each(num_qubits: usize, gates: &[Gate]) {
    let mut legacy = StabilizerState::new(num_qubits);
    let mut packed = PackedInverseTableau::identity(num_qubits);

    assert_eq!(packed.canonical_snapshot(), legacy.canonical_snapshot());

    for (k, &gate) in gates.iter().enumerate() {
        apply_legacy(&mut legacy, gate);
        apply_packed(&mut packed, gate);
        assert_eq!(
            packed.canonical_snapshot(),
            legacy.canonical_snapshot(),
            "snapshot mismatch after gate {k}: {gate:?}",
        );
    }
}

fn raw_inverse_snapshot(tableau: &PackedInverseTableau) -> CanonicalTableauSnapshot {
    let num_qubits = tableau.num_qubits();
    let mut x = vec![vec![false; num_qubits]; tableau.num_rows()];
    let mut z = vec![vec![false; num_qubits]; tableau.num_rows()];
    let mut phase = vec![0; tableau.num_rows()];

    for row in 0..tableau.num_rows() {
        phase[row] = tableau.canonical_phase(row);
        for qubit in 0..num_qubits {
            x[row][qubit] = tableau.x(row, qubit);
            z[row][qubit] = tableau.z(row, qubit);
        }
    }

    CanonicalTableauSnapshot {
        num_qubits,
        x,
        z,
        phase,
    }
}

fn deterministic_sequence(seed: u64, num_qubits: usize, len: usize) -> Vec<Gate> {
    let mut gates = vec![
        Gate::H(0),
        Gate::S(1),
        Gate::SDag(2),
        Gate::X(3),
        Gate::Y(4),
        Gate::Z(5),
        Gate::Cx(6, 7),
    ];
    let mut state = seed;

    while gates.len() < len {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let selector = (state >> 61) as usize;
        let q = ((state >> 17) as usize) % num_qubits;
        let q2 = ((state >> 31) as usize) % (num_qubits - 1);
        let target = if q2 >= q { q2 + 1 } else { q2 };

        gates.push(match selector % 7 {
            0 => Gate::H(q),
            1 => Gate::S(q),
            2 => Gate::SDag(q),
            3 => Gate::X(q),
            4 => Gate::Y(q),
            5 => Gate::Z(q),
            _ => Gate::Cx(q, target),
        });
    }

    gates
}

fn strip_snapshot_accessor(source: &str) -> String {
    let begin = "    // BEGIN issue-456 read-only snapshot accessor\n";
    let end = "    // END issue-456 read-only snapshot accessor\n";
    let Some(start) = source.find(begin) else {
        return source.to_string();
    };
    let relative_end = source[start..]
        .find(end)
        .expect("snapshot accessor end marker missing");
    let end_index = start + relative_end + end.len();
    let mut stripped = String::with_capacity(source.len() - (end_index - start));
    stripped.push_str(&source[..start]);
    stripped.push_str(&source[end_index..]);
    stripped
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325;
    for &byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

fn assert_legacy_oracle_only_has_snapshot_accessor() {
    let current = include_str!("../src/sim/tableau.rs");
    let stripped = strip_snapshot_accessor(current);
    assert_eq!(stripped.len(), AUDITED_TABLEAU_LEN);
    assert_eq!(fnv1a64(stripped.as_bytes()), AUDITED_TABLEAU_FNV1A64);
}

#[test]
fn each_supported_gate_matches_pinned_legacy() {
    assert_legacy_oracle_only_has_snapshot_accessor();

    let cases = [
        vec![Gate::H(0)],
        vec![Gate::S(0)],
        vec![Gate::SDag(0)],
        vec![Gate::X(0)],
        vec![Gate::Y(0)],
        vec![Gate::Z(0)],
        vec![Gate::Cx(0, 1)],
        vec![Gate::H(0), Gate::Cx(0, 1)],
    ];

    for gates in cases {
        assert_matches_after_each(2, &gates);
    }
}

#[test]
fn directed_cx_0_to_1_is_not_cx_1_to_0() {
    assert_matches_after_each(2, &[Gate::H(0), Gate::Cx(0, 1)]);

    let mut legacy = StabilizerState::new(2);
    let mut swapped = PackedInverseTableau::identity(2);

    apply_legacy(&mut legacy, Gate::H(0));
    apply_packed(&mut swapped, Gate::H(0));
    assert_eq!(swapped.canonical_snapshot(), legacy.canonical_snapshot());

    apply_legacy(&mut legacy, Gate::Cx(0, 1));
    apply_packed(&mut swapped, Gate::Cx(1, 0));
    assert_ne!(
        swapped.canonical_snapshot(),
        legacy.canonical_snapshot(),
        "swapped CX direction unexpectedly matched the directed oracle",
    );
}

#[test]
fn fixed_seed_sequences_match_after_every_gate() {
    for seed in [0x455, 0xC0FFEE, 0x5EED5EED] {
        let gates = deterministic_sequence(seed, 130, 4096);
        assert!(gates.iter().any(|gate| matches!(gate, Gate::H(_))));
        assert!(gates.iter().any(|gate| matches!(gate, Gate::S(_))));
        assert!(gates.iter().any(|gate| matches!(gate, Gate::SDag(_))));
        assert!(gates.iter().any(|gate| matches!(gate, Gate::X(_))));
        assert!(gates.iter().any(|gate| matches!(gate, Gate::Y(_))));
        assert!(gates.iter().any(|gate| matches!(gate, Gate::Z(_))));
        assert!(gates.iter().any(|gate| matches!(gate, Gate::Cx(_, _))));
        assert_matches_after_each(130, &gates);
    }

    println!("PASS packed inverse Clifford evolution");
}

#[test]
fn packed_evolution_crosses_words_63_64_129() {
    let gates = [
        Gate::H(63),
        Gate::S(64),
        Gate::SDag(129),
        Gate::Cx(63, 64),
        Gate::Cx(64, 129),
        Gate::X(63),
        Gate::Y(64),
        Gate::Z(129),
        Gate::H(129),
    ];
    assert_matches_after_each(130, &gates);
}

#[test]
fn raw_inverse_rows_are_not_the_canonical_snapshot_after_s() {
    let mut legacy = StabilizerState::new(1);
    let mut packed = PackedInverseTableau::identity(1);

    legacy.s(0);
    packed.s(0);

    assert_eq!(packed.canonical_snapshot(), legacy.canonical_snapshot());
    assert_ne!(
        raw_inverse_snapshot(&packed),
        legacy.canonical_snapshot(),
        "raw inverse rows were incorrectly accepted as canonical rows",
    );
}
