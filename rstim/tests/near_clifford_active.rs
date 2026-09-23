#[path = "support/near_clifford_oracle.rs"]
mod oracle;

use oracle::{Amp, DenseOracle};
use rand::{Rng, SeedableRng, rngs::StdRng};
use rstim::near_clifford::{ActiveState, CliffordGate, ComplexAmp};

fn assert_close(actual: ComplexAmp, expected: Amp) {
    let error = ((actual.re - expected.re).powi(2) + (actual.im - expected.im).powi(2)).sqrt();
    assert!(
        error < 1e-12,
        "actual={actual:?}, expected={expected:?}, error={error}"
    );
}

fn compare_virtual_state(active: &ActiveState, virtual_oracle: &DenseOracle) {
    let n = active.num_qubits();
    assert!(n <= 10);
    let mut expected = vec![ComplexAmp::default(); 1 << n];
    for (coordinate, amplitude) in active.coefficients().iter().enumerate() {
        let mut virtual_index = 0;
        for (axis_number, axis) in active.active_axes().iter().enumerate() {
            if coordinate & (1 << axis_number) != 0 {
                for (q, &bit) in axis.iter().enumerate() {
                    if bit {
                        virtual_index ^= 1 << (n - q - 1);
                    }
                }
            }
        }
        expected[virtual_index] = expected[virtual_index] + *amplitude * active.global_phase();
    }
    for (actual, expected) in expected.into_iter().zip(virtual_oracle.amplitudes()) {
        assert_close(actual, *expected);
    }
}

#[test]
fn two_t_active_state_matches_independent_oracle() {
    let mut active = ActiveState::new(2, 2);
    let mut oracle = DenseOracle::new(2);
    active.apply_clifford(CliffordGate::H(0)).unwrap();
    oracle.h(0);
    active.apply_clifford(CliffordGate::CX(0, 1)).unwrap();
    oracle.cx(0, 1);
    active.t(0).unwrap();
    oracle.t(0);
    active.apply_clifford(CliffordGate::H(0)).unwrap();
    oracle.h(0);
    active.t(0).unwrap();
    oracle.t(0);
    assert_eq!(active.active_rank(), 2);
    assert_eq!(active.coefficients().len(), 4);
    // U = H0 CX01 H0 is its own inverse. Undo it on the independent oracle.
    oracle.h(0);
    oracle.cx(0, 1);
    oracle.h(0);
    compare_virtual_state(&active, &oracle);
}

#[test]
fn s_and_t_dag_preserve_phase_against_oracle() {
    let mut active = ActiveState::new(3, 3);
    let mut oracle = DenseOracle::new(3);
    active.apply_clifford(CliffordGate::H(0)).unwrap();
    oracle.h(0);
    active.apply_clifford(CliffordGate::S(0)).unwrap();
    oracle.s(0);
    active.apply_clifford(CliffordGate::CX(0, 2)).unwrap();
    oracle.cx(0, 2);
    active.t_dag(2).unwrap();
    oracle.t_dag(2);
    active.apply_clifford(CliffordGate::H(2)).unwrap();
    oracle.h(2);
    active.t(0).unwrap();
    oracle.t(0);
    active.apply_clifford(CliffordGate::SDag(0)).unwrap();
    oracle.s_dag(0);
    // Undo only the Clifford frame, in reverse order.
    oracle.s(0);
    oracle.h(2);
    oracle.cx(0, 2);
    oracle.s_dag(0);
    oracle.h(0);
    compare_virtual_state(&active, &oracle);
}

#[test]
fn dependent_t_gates_do_not_expand_active_rank() {
    let mut active = ActiveState::new(128, 1);
    active.apply_clifford(CliffordGate::H(7)).unwrap();
    for _ in 0..12 {
        active.t(7).unwrap();
        assert_eq!(active.active_rank(), 1);
        assert_eq!(active.coefficients().len(), 2);
    }
    for _ in 0..12 {
        active.t_dag(7).unwrap();
    }
    assert!((active.coefficients()[0].norm_sqr() - 1.0).abs() < 1e-12);
    assert!(active.coefficients()[1].norm_sqr() < 1e-12);
}

#[test]
fn eigenstate_t_and_rank_limit() {
    let mut active = ActiveState::new(3, 1);
    active.t(0).unwrap();
    assert_eq!(active.active_rank(), 0);
    active.apply_clifford(CliffordGate::H(0)).unwrap();
    active.t(0).unwrap();
    assert_eq!(active.active_rank(), 1);
    active.apply_clifford(CliffordGate::H(1)).unwrap();
    let before = active.coefficients().to_vec();
    assert!(active.t(1).unwrap_err().contains("active-state limit"));
    assert_eq!(active.active_rank(), 1);
    assert_eq!(active.coefficients(), before);
    assert_eq!(active.coefficients().len(), 2);
}

#[test]
fn seeded_three_qubit_circuits_match_independent_oracle() {
    let mut rng = StdRng::seed_from_u64(739);
    for case in 0..40 {
        let mut active = ActiveState::new(3, 3);
        let mut oracle = DenseOracle::new(3);
        let mut cliffords = Vec::new();
        for _ in 0..24 {
            let q = rng.gen_range(0..3);
            match rng.gen_range(0..11) {
                0 => {
                    active.apply_clifford(CliffordGate::H(q)).unwrap();
                    oracle.h(q);
                    cliffords.push(CliffordGate::H(q));
                }
                1 => {
                    active.apply_clifford(CliffordGate::S(q)).unwrap();
                    oracle.s(q);
                    cliffords.push(CliffordGate::S(q));
                }
                2 => {
                    active.apply_clifford(CliffordGate::X(q)).unwrap();
                    oracle.x(q);
                    cliffords.push(CliffordGate::X(q));
                }
                3 => {
                    active.apply_clifford(CliffordGate::Z(q)).unwrap();
                    oracle.z(q);
                    cliffords.push(CliffordGate::Z(q));
                }
                4 => {
                    let target = (q + rng.gen_range(1..3)) % 3;
                    active.apply_clifford(CliffordGate::CX(q, target)).unwrap();
                    oracle.cx(q, target);
                    cliffords.push(CliffordGate::CX(q, target));
                }
                5 => {
                    active.t(q).unwrap();
                    oracle.t(q);
                }
                6 => {
                    active.t_dag(q).unwrap();
                    oracle.t_dag(q);
                }
                7 => {
                    active.apply_clifford(CliffordGate::SDag(q)).unwrap();
                    oracle.s_dag(q);
                    cliffords.push(CliffordGate::SDag(q));
                }
                8 => {
                    active.apply_clifford(CliffordGate::Y(q)).unwrap();
                    oracle.y(q);
                    cliffords.push(CliffordGate::Y(q));
                }
                9 => {
                    let target = (q + rng.gen_range(1..3)) % 3;
                    active.apply_clifford(CliffordGate::CZ(q, target)).unwrap();
                    oracle.cz(q, target);
                    cliffords.push(CliffordGate::CZ(q, target));
                }
                _ => {
                    let target = (q + rng.gen_range(1..3)) % 3;
                    active
                        .apply_clifford(CliffordGate::Swap(q, target))
                        .unwrap();
                    oracle.swap(q, target);
                    cliffords.push(CliffordGate::Swap(q, target));
                }
            }
        }
        for gate in cliffords.into_iter().rev() {
            match gate {
                CliffordGate::H(q) => oracle.h(q),
                CliffordGate::S(q) => oracle.s_dag(q),
                CliffordGate::SDag(q) => oracle.s(q),
                CliffordGate::X(q) => oracle.x(q),
                CliffordGate::Z(q) => oracle.z(q),
                CliffordGate::CX(a, b) => oracle.cx(a, b),
                CliffordGate::Y(q) => oracle.y(q),
                CliffordGate::CZ(a, b) => oracle.cz(a, b),
                CliffordGate::Swap(a, b) => oracle.swap(a, b),
            }
        }
        compare_virtual_state(&active, &oracle);
        assert!(active.active_rank() <= 3, "case {case}");
    }
}

#[test]
fn clifford_target_errors_leave_state_unchanged() {
    let mut state = ActiveState::new(2, 2);
    let before = state.frame_snapshot();
    assert!(state.apply_clifford(CliffordGate::H(2)).is_err());
    assert!(state.apply_clifford(CliffordGate::CX(0, 0)).is_err());
    assert!(state.apply_clifford(CliffordGate::Swap(0, 2)).is_err());
    assert_eq!(state.frame_snapshot(), before);
    assert_eq!(state.active_rank(), 0);
}
