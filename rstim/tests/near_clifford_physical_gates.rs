#[path = "support/near_clifford_oracle.rs"]
mod oracle;

use oracle::DenseOracle;
use rand::{SeedableRng, rngs::StdRng};
use rstim::near_clifford::{ActiveState, CliffordGate, MeasurementBasis};

#[test]
fn fused_physical_gates_preserve_wide_coherent_measurement_and_feedback() {
    for width in [4, 65, 129, 193] {
        let map = [0, width / 3, 2 * width / 3, width - 1];
        let mut state = ActiveState::new(width, 4);
        let mut oracle = DenseOracle::new(4);
        let mut rng = StdRng::seed_from_u64(20261004 + width as u64);
        for q in 0..4 {
            state.apply_clifford(CliffordGate::H(map[q])).unwrap();
            oracle.h(q);
            if q < 2 {
                state.t(map[q]).unwrap();
                oracle.t(q);
            }
        }
        for step in 0..16 {
            let q = step % 4;
            let other = (q + 1) % 4;
            let fork = state.clone();
            let snapshot = fork.frame_snapshot();
            state
                .apply_clifford(CliffordGate::CZ(map[q], map[other]))
                .unwrap();
            oracle.cz(q, other);
            state.apply_clifford(CliffordGate::SDag(map[q])).unwrap();
            oracle.s_dag(q);
            state.apply_clifford(CliffordGate::Y(map[other])).unwrap();
            oracle.y(other);
            assert_eq!(
                fork.frame_snapshot(),
                snapshot,
                "copy-on-write frame must preserve the other branch"
            );
            for j in 0..4 {
                for (basis, letter) in [
                    (MeasurementBasis::X, 'X'),
                    (MeasurementBasis::Y, 'Y'),
                    (MeasurementBasis::Z, 'Z'),
                ] {
                    assert!(
                        (state.measurement_probabilities(map[j], basis).unwrap().0
                            - oracle.measurement_probability(j, letter, false))
                        .abs()
                            < 1e-10,
                        "width={width} step={step} q={j} basis={basis:?}"
                    );
                }
            }
            let bit = state
                .measure(map[q], MeasurementBasis::Y, &mut rng)
                .unwrap();
            oracle.collapse(q, 'Y', bit);
            if bit {
                state.apply_clifford(CliffordGate::Y(map[other])).unwrap();
                oracle.y(other);
            }
            state.apply_clifford(CliffordGate::H(map[q])).unwrap();
            oracle.h(q);
            state.t(map[q]).unwrap();
            oracle.t(q);
        }
        let before = state.frame_snapshot();
        for gate in [
            CliffordGate::SDag(width),
            CliffordGate::Y(width),
            CliffordGate::CZ(0, width),
            CliffordGate::CZ(0, 0),
        ] {
            assert!(state.apply_clifford(gate).is_err());
            assert_eq!(state.frame_snapshot(), before);
        }
    }
}
