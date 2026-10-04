#[path = "support/near_clifford_oracle.rs"]
mod oracle;

use oracle::DenseOracle;
use rand::{SeedableRng, rngs::StdRng};
use rstim::near_clifford::{ActiveState, CliffordGate, MeasurementBasis};

#[test]
fn projected_origins_preserve_coherent_feedback_and_reinjection_at_wide_boundaries() {
    let mut nonzero_origins = 0;
    let mut clifford_rebases = 0;
    for width in [4, 63, 64, 65, 127, 128, 129, 193] {
        let map = [0, width / 3, 2 * width / 3, width - 1];
        for seed in 0..8 {
            let mut state = ActiveState::new(width, 4);
            let mut oracle = DenseOracle::new(4);
            let mut rng = StdRng::seed_from_u64(780 + seed);
            for round in 0..4 {
                for q in 0..4 {
                    state.apply_clifford(CliffordGate::H(map[q])).unwrap();
                    oracle.h(q);
                    state.t(map[q]).unwrap();
                    oracle.t(q);
                    state.apply_clifford(CliffordGate::S(map[q])).unwrap();
                    oracle.s(q);
                }
                for q in 0..3 {
                    state
                        .apply_clifford(CliffordGate::CZ(map[q], map[q + 1]))
                        .unwrap();
                    oracle.cz(q, q + 1);
                }
                for (q, basis, letter) in [
                    (0, MeasurementBasis::Y, 'Y'),
                    (1, MeasurementBasis::X, 'X'),
                    (2, MeasurementBasis::Z, 'Z'),
                    (3, MeasurementBasis::Y, 'Y'),
                    (0, MeasurementBasis::X, 'X'),
                ] {
                    let had_origin = state.origin().contains(&true);
                    let was_clifford = state.active_rank() == 0;
                    let fork = state.clone();
                    let snapshot = fork.frame_snapshot();
                    let expected = oracle.measurement_probability(q, letter, false);
                    assert!(
                        (state.measurement_probabilities(map[q], basis).unwrap().0 - expected)
                            .abs()
                            < 1e-10
                    );
                    let bit = state.measure(map[q], basis, &mut rng).unwrap();
                    oracle.collapse(q, letter, bit);
                    assert_eq!(fork.frame_snapshot(), snapshot);
                    assert_eq!(fork.origin().contains(&true), had_origin);
                    nonzero_origins += usize::from(state.origin().contains(&true));
                    clifford_rebases += usize::from(had_origin && was_clifford);
                    for j in 0..4 {
                        for (b, l) in [
                            (MeasurementBasis::X, 'X'),
                            (MeasurementBasis::Y, 'Y'),
                            (MeasurementBasis::Z, 'Z'),
                        ] {
                            assert!(
                                (state.measurement_probabilities(map[j], b).unwrap().0
                                    - oracle.measurement_probability(j, l, false))
                                .abs()
                                    < 1e-10,
                                "width={width} seed={seed} round={round} q={q} j={j} basis={b:?}"
                            );
                        }
                    }
                    if bit {
                        let other = (q + 1) % 4;
                        state.apply_clifford(CliffordGate::Y(map[other])).unwrap();
                        oracle.y(other);
                    }
                }
            }
        }
    }
    assert!(
        nonzero_origins > 0,
        "must exercise projected nonzero origins"
    );
    assert!(
        clifford_rebases > 0,
        "must absorb an origin after all active axes retire"
    );
}
