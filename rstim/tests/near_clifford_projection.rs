#[path = "support/near_clifford_oracle.rs"]
mod oracle;

use oracle::{Amp, DenseOracle};
use rand::{Rng, SeedableRng, rngs::StdRng};
use rstim::near_clifford::{ActiveState, CliffordGate, MeasurementBasis};

fn compare_marginals(state: &ActiveState, oracle: &DenseOracle, mapping: &[usize; 4]) {
    for (logical, &physical) in mapping.iter().enumerate() {
        for (basis, letter) in [
            (MeasurementBasis::X, 'X'),
            (MeasurementBasis::Y, 'Y'),
            (MeasurementBasis::Z, 'Z'),
        ] {
            let (zero, one) = state.measurement_probabilities(physical, basis).unwrap();
            let expected = oracle.measurement_probability(logical, letter, false);
            assert!((zero - expected).abs() < 1e-10);
            assert!((one - (1.0 - expected)).abs() < 1e-10);
        }
    }
}

fn compare_density_matrix(state: &ActiveState, oracle: &DenseOracle, mapping: &[usize; 4]) {
    // All 255 nonidentity Pauli expectations determine the logical density
    // matrix, including correlations and relative phases hidden by marginals.
    for word in 1..256 {
        let mut measured = state.clone();
        let mut x_mask = 0;
        let mut z_mask = 0;
        let mut y_count = 0;
        let mut pivot = None;
        for (q, &physical) in mapping.iter().enumerate() {
            match (word >> (2 * q)) & 3 {
                0 => continue,
                1 => {
                    x_mask |= 1 << (3 - q);
                    measured.apply_clifford(CliffordGate::H(physical)).unwrap();
                }
                2 => {
                    x_mask |= 1 << (3 - q);
                    z_mask |= 1 << (3 - q);
                    y_count += 1;
                    measured
                        .apply_clifford(CliffordGate::SDag(physical))
                        .unwrap();
                    measured.apply_clifford(CliffordGate::H(physical)).unwrap();
                }
                _ => z_mask |= 1 << (3 - q),
            }
            pivot.get_or_insert(physical);
        }
        let pivot = pivot.unwrap();
        for (q, &physical) in mapping.iter().enumerate() {
            if physical != pivot && (word >> (2 * q)) & 3 != 0 {
                measured
                    .apply_clifford(CliffordGate::CX(physical, pivot))
                    .unwrap();
            }
        }
        let phase = match y_count % 4 {
            0 => Amp::new(1.0, 0.0),
            1 => Amp::new(0.0, 1.0),
            2 => Amp::new(-1.0, 0.0),
            _ => Amp::new(0.0, -1.0),
        };
        let expected = oracle
            .amplitudes()
            .iter()
            .enumerate()
            .map(|(index, &amplitude)| {
                let sign = if (index & z_mask).count_ones() & 1 != 0 {
                    -1.0
                } else {
                    1.0
                };
                (oracle.amplitudes()[index ^ x_mask].conj() * (phase * amplitude * sign)).re
            })
            .sum::<f64>();
        let zero = measured
            .measurement_probabilities(pivot, MeasurementBasis::Z)
            .unwrap()
            .0;
        assert!((2.0 * zero - 1.0 - expected).abs() < 1e-10, "word={word}");
    }
}

#[test]
fn rank_twelve_projection_preserves_correlations_with_spectator_magic_states() {
    for width in [12, 65, 129] {
        for seed in [20261003, 20261004] {
            let mapping = [0, 3, width / 2, width - 1];
            let mut active = ActiveState::new(width, 12);
            let mut oracle = DenseOracle::new(4);
            for q in (0..width).filter(|q| !mapping.contains(q)).take(8) {
                active.apply_clifford(CliffordGate::H(q)).unwrap();
                active.t(q).unwrap();
            }
            for (logical, &physical) in mapping.iter().enumerate() {
                active.apply_clifford(CliffordGate::H(physical)).unwrap();
                active.t(physical).unwrap();
                oracle.h(logical);
                oracle.t(logical);
            }
            assert_eq!(active.active_rank(), 12);
            let mut rng = StdRng::seed_from_u64(seed);
            for step in 0..12 {
                let q = rng.gen_range(0..4);
                let other = (q + rng.gen_range(1..4)) % 4;
                let (physical, target) = (mapping[q], mapping[other]);
                match step % 6 {
                    0 => {
                        active.apply_clifford(CliffordGate::H(physical)).unwrap();
                        oracle.h(q);
                    }
                    1 => {
                        active.apply_clifford(CliffordGate::SDag(physical)).unwrap();
                        oracle.s_dag(q);
                    }
                    2 => {
                        active
                            .apply_clifford(CliffordGate::CX(physical, target))
                            .unwrap();
                        oracle.cx(q, other);
                    }
                    3 => {
                        active
                            .apply_clifford(CliffordGate::CZ(physical, target))
                            .unwrap();
                        oracle.cz(q, other);
                    }
                    4 => {
                        active
                            .apply_clifford(CliffordGate::Swap(physical, target))
                            .unwrap();
                        oracle.swap(q, other);
                    }
                    _ => {
                        active.apply_clifford(CliffordGate::S(physical)).unwrap();
                        oracle.s(q);
                    }
                }
                active.t(physical).unwrap();
                oracle.t(q);
                active.t_dag(target).unwrap();
                oracle.t_dag(other);
                compare_marginals(&active, &oracle, &mapping);
                let (basis, letter) = match step % 3 {
                    0 => (MeasurementBasis::X, 'X'),
                    1 => (MeasurementBasis::Y, 'Y'),
                    _ => (MeasurementBasis::Z, 'Z'),
                };
                let outcome = if step % 4 == 0 {
                    active.measure_reset(physical, basis, &mut rng).unwrap()
                } else {
                    active.measure(physical, basis, &mut rng).unwrap()
                };
                oracle.collapse(q, letter, outcome);
                if step % 4 == 0 && outcome {
                    if letter == 'Z' {
                        oracle.x(q);
                    } else {
                        oracle.z(q);
                    }
                }
                if outcome {
                    active.apply_clifford(CliffordGate::X(target)).unwrap();
                    oracle.x(other);
                }
                assert!(active.active_rank() <= 11);
                assert!(
                    (active
                        .coefficients()
                        .iter()
                        .map(|a| a.norm_sqr())
                        .sum::<f64>()
                        - 1.0)
                        .abs()
                        < 1e-10
                );
                compare_marginals(&active, &oracle, &mapping);
            }
            compare_density_matrix(&active, &oracle, &mapping);
        }
    }
}
