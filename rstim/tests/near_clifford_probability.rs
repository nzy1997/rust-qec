#[path = "support/near_clifford_oracle.rs"]
mod oracle;

use oracle::DenseOracle;
use rand::{SeedableRng, rngs::StdRng};
use rstim::near_clifford::{ActiveState, CliffordGate, MeasurementBasis};

fn compare(state: &ActiveState, factors: &[DenseOracle], mapping: &[usize]) {
    for (&physical, factor) in mapping.iter().zip(factors) {
        for (basis, letter) in [
            (MeasurementBasis::X, 'X'),
            (MeasurementBasis::Y, 'Y'),
            (MeasurementBasis::Z, 'Z'),
        ] {
            let expected = factor.measurement_probability(0, letter, false);
            let (zero, one) = state.measurement_probabilities(physical, basis).unwrap();
            assert!(
                (zero - expected).abs() < 1e-12,
                "q={physical} basis={basis:?}: {zero} != {expected}"
            );
            assert!((one - (1.0 - expected)).abs() < 1e-12);
        }
    }
}

#[test]
fn rank_sixteen_born_probabilities_match_independent_factors_through_signed_measurements() {
    for width in [16, 65, 129, 193] {
        let mapping = (0..16).map(|q| q * (width - 1) / 15).collect::<Vec<_>>();
        let mut state = ActiveState::new(width, 16);
        let mut factors = (0..16).map(|_| DenseOracle::new(1)).collect::<Vec<_>>();
        for (q, (&physical, factor)) in mapping.iter().zip(&mut factors).enumerate() {
            state.apply_clifford(CliffordGate::H(physical)).unwrap();
            factor.h(0);
            if q % 2 == 0 {
                state.t(physical).unwrap();
                factor.t(0);
            } else {
                state.t_dag(physical).unwrap();
                factor.t_dag(0);
            }
            match q % 3 {
                0 => {}
                1 => {
                    state.apply_clifford(CliffordGate::S(physical)).unwrap();
                    factor.s(0);
                }
                _ => {
                    state.apply_clifford(CliffordGate::H(physical)).unwrap();
                    factor.h(0);
                }
            }
            if q % 4 == 0 {
                state.apply_clifford(CliffordGate::X(physical)).unwrap();
                factor.x(0);
            }
            if q % 5 == 0 {
                state.apply_clifford(CliffordGate::Y(physical)).unwrap();
                factor.y(0);
            }
        }
        assert_eq!(state.active_rank(), 16);
        compare(&state, &factors, &mapping);
        let mut rng = StdRng::seed_from_u64(20261006);
        for (q, &physical) in mapping.iter().enumerate() {
            let (basis, letter) = match q % 3 {
                0 => (MeasurementBasis::X, 'X'),
                1 => (MeasurementBasis::Y, 'Y'),
                _ => (MeasurementBasis::Z, 'Z'),
            };
            let reset = q % 2 == 0;
            let outcome = if reset {
                state.measure_reset(physical, basis, &mut rng).unwrap()
            } else {
                state.measure(physical, basis, &mut rng).unwrap()
            };
            factors[q].collapse(0, letter, outcome);
            if reset && outcome {
                if letter == 'Z' {
                    factors[q].x(0);
                } else {
                    factors[q].z(0);
                }
            }
            compare(&state, &factors, &mapping);
        }
        assert_eq!(state.active_rank(), 0);
    }
}
