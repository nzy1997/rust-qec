#[path = "support/near_clifford_oracle.rs"]
mod oracle;

use oracle::DenseOracle;
use rand::{Rng, SeedableRng, rngs::StdRng};
use rstim::near_clifford::{ActiveState, CliffordGate, MeasurementBasis};

fn compare_probabilities(active: &ActiveState, oracle: &DenseOracle, mapping: &[usize; 4]) {
    for (logical, &physical) in mapping.iter().enumerate() {
        for (basis, letter) in [
            (MeasurementBasis::X, 'X'),
            (MeasurementBasis::Y, 'Y'),
            (MeasurementBasis::Z, 'Z'),
        ] {
            let (zero, one) = active.measurement_probabilities(physical, basis).unwrap();
            let expected = oracle.measurement_probability(logical, letter, false);
            assert!(
                (zero - expected).abs() < 1e-10 && (one - (1.0 - expected)).abs() < 1e-10,
                "physical={physical} basis={basis:?} zero={zero} expected={expected}"
            );
        }
    }
}

#[test]
fn wide_single_qubit_measurements_preserve_phase_through_entanglement_reset_and_feedback() {
    // A four-qubit independent state-vector oracle is embedded at nonadjacent
    // physical columns, including the last column across word boundaries.
    for width in [4, 64, 65, 128, 129, 193] {
        let mapping = [0, width / 3, 2 * width / 3, width - 1];
        let mut active = ActiveState::new(width, 8);
        let mut oracle = DenseOracle::new(4);
        let mut rng = StdRng::seed_from_u64(20260930 + width as u64);
        for (logical, &physical) in mapping.iter().enumerate() {
            active.apply_clifford(CliffordGate::H(physical)).unwrap();
            active.t(physical).unwrap();
            oracle.h(logical);
            oracle.t(logical);
        }
        for step in 0..72 {
            let q = rng.gen_range(0..4);
            let other = (q + rng.gen_range(1..4)) % 4;
            let (physical, target) = (mapping[q], mapping[other]);
            match step % 6 {
                0 => {
                    active.apply_clifford(CliffordGate::H(physical)).unwrap();
                    oracle.h(q);
                }
                1 => {
                    active.apply_clifford(CliffordGate::S(physical)).unwrap();
                    oracle.s(q);
                }
                2 => {
                    active.apply_clifford(CliffordGate::SDag(physical)).unwrap();
                    oracle.s_dag(q);
                }
                3 => {
                    active
                        .apply_clifford(CliffordGate::CX(physical, target))
                        .unwrap();
                    oracle.cx(q, other);
                }
                4 => {
                    active
                        .apply_clifford(CliffordGate::CZ(physical, target))
                        .unwrap();
                    oracle.cz(q, other);
                }
                _ => {
                    active
                        .apply_clifford(CliffordGate::Swap(physical, target))
                        .unwrap();
                    oracle.swap(q, other);
                }
            }
            active.t(physical).unwrap();
            oracle.t(q);
            active.t_dag(target).unwrap();
            oracle.t_dag(other);
            compare_probabilities(&active, &oracle, &mapping);
            let (basis, letter) = match (step / 6 + step) % 3 {
                0 => (MeasurementBasis::X, 'X'),
                1 => (MeasurementBasis::Y, 'Y'),
                _ => (MeasurementBasis::Z, 'Z'),
            };
            let outcome = if step % 7 == 0 {
                active.measure_reset(physical, basis, &mut rng).unwrap()
            } else {
                active.measure(physical, basis, &mut rng).unwrap()
            };
            oracle.collapse(q, letter, outcome);
            if step % 7 == 0 && outcome {
                if letter == 'Z' {
                    oracle.x(q);
                } else {
                    oracle.z(q);
                }
            }
            // The sampled branch controls a subsequent physical operation.
            if outcome {
                active.apply_clifford(CliffordGate::X(target)).unwrap();
                oracle.x(other);
            }
            compare_probabilities(&active, &oracle, &mapping);
        }
        let before = active.frame_snapshot();
        for basis in [
            MeasurementBasis::X,
            MeasurementBasis::Y,
            MeasurementBasis::Z,
        ] {
            assert!(
                active
                    .measurement_probabilities(width, basis)
                    .unwrap_err()
                    .contains("outside near-Clifford state")
            );
        }
        assert_eq!(active.frame_snapshot(), before);
    }
}

#[test]
fn dense_signed_parity_rows_preserve_encoded_phase_after_decoding() {
    // Encode one non-Clifford qubit across every physical column. Local S
    // gates give a dense Y tableau row, including more than 255 Y factors.
    // Decoding exposes the accumulated phase as a single-qubit observable.
    for width in [3, 63, 64, 65, 127, 128, 129, 193, 260] {
        let mut state = ActiveState::new(width, 2);
        let mut oracle = DenseOracle::new(1);
        state.apply_clifford(CliffordGate::H(0)).unwrap();
        state.t(0).unwrap();
        oracle.h(0);
        oracle.t(0);
        for q in 1..width {
            state.apply_clifford(CliffordGate::CX(0, q)).unwrap();
        }
        for q in 0..width {
            state.apply_clifford(CliffordGate::S(q)).unwrap();
            oracle.s(0);
            if q % 2 == 1 {
                state.apply_clifford(CliffordGate::Z(q)).unwrap();
                oracle.z(0);
            }
        }
        // Every physical Z acts as the encoded logical Z, even at the last
        // column. General T conversion must retain the same signed phase.
        state.t_dag(width - 1).unwrap();
        oracle.t_dag(0);
        for q in (1..width).rev() {
            state.apply_clifford(CliffordGate::CX(0, q)).unwrap();
        }
        for (basis, letter) in [
            (MeasurementBasis::X, 'X'),
            (MeasurementBasis::Y, 'Y'),
            (MeasurementBasis::Z, 'Z'),
        ] {
            let expected = oracle.measurement_probability(0, letter, false);
            let (zero, one) = state.measurement_probabilities(0, basis).unwrap();
            assert!((zero - expected).abs() < 1e-10, "width={width}, {basis:?}");
            assert!((one - (1.0 - expected)).abs() < 1e-10);
        }
        for q in [1, width / 2, width - 1] {
            assert_eq!(
                state
                    .measurement_probabilities(q, MeasurementBasis::Z)
                    .unwrap(),
                (1.0, 0.0),
                "decoded ancilla at width={width}, q={q}"
            );
        }
    }
}
