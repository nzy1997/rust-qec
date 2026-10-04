#[path = "support/near_clifford_oracle.rs"]
mod oracle;

use oracle::DenseOracle;
use rand::{RngCore, SeedableRng, rngs::StdRng};
use rstim::near_clifford::{ActiveState, CliffordGate, MeasurementBasis, NearCliffordExecutor};

#[test]
fn independent_measurements_preserve_conditional_born_probabilities_and_phase() {
    for width in [4, 65, 129, 193] {
        let map = [0, width / 3, 2 * width / 3, width - 1];
        for seed in 0..8 {
            let mut state = ActiveState::new(width, 4);
            let mut oracle = DenseOracle::new(4);
            let mut rng = StdRng::seed_from_u64(1409 + seed);
            for q in 0..4 {
                state.apply_clifford(CliffordGate::H(map[q])).unwrap();
                oracle.h(q);
                if q < 2 {
                    state.t(map[q]).unwrap();
                    oracle.t(q);
                }
            }
            for q in 0..3 {
                state
                    .apply_clifford(CliffordGate::CZ(map[q], map[q + 1]))
                    .unwrap();
                oracle.cz(q, q + 1);
            }
            state.apply_clifford(CliffordGate::S(map[3])).unwrap();
            oracle.s(3);
            for (q, basis, letter) in [
                (3, MeasurementBasis::Y, 'Y'),
                (2, MeasurementBasis::X, 'X'),
                (0, MeasurementBasis::Z, 'Z'),
                (1, MeasurementBasis::Y, 'Y'),
            ] {
                let expected = oracle.measurement_probability(q, letter, false);
                let actual = state.measurement_probabilities(map[q], basis).unwrap().0;
                assert!(
                    (actual - expected).abs() < 1e-10,
                    "width={width} seed={seed} q={q}"
                );
                let bit = state.measure(map[q], basis, &mut rng).unwrap();
                oracle.collapse(q, letter, bit);
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
                                < 1e-10
                        );
                    }
                }
                state.apply_clifford(CliffordGate::H(map[q])).unwrap();
                oracle.h(q);
                state.t(map[q]).unwrap();
                oracle.t(q);
            }
        }
    }
}

#[test]
fn wide_independent_measurements_preserve_batches_feedback_and_rng() {
    let text = "H 0 64 128 192\nT 0 64\nCZ 0 128 64 192\nS 192\nMY 192\nCX rec[-1] 128\nMX 128\nT_DAG 64\nH 0\nM !0\nMY 64\nDETECTOR rec[-1] rec[-3]\nOBSERVABLE_INCLUDE(0) rec[-2]";
    let executor = NearCliffordExecutor::compile_text(text).unwrap();
    for seed in [739, 1943] {
        let mut sampler = executor.prepare_sampler().unwrap();
        let mut a = StdRng::seed_from_u64(seed);
        let mut b = StdRng::seed_from_u64(seed);
        for shots in [0, 1, 64, 256] {
            let actual = sampler.sample(shots, &mut a).unwrap();
            let expected = (0..shots)
                .map(|_| executor.run(&mut b).unwrap())
                .collect::<Vec<_>>();
            assert_eq!(actual, expected);
        }
        assert_eq!(a.next_u64(), b.next_u64());
    }
}

#[test]
fn wide_terminal_independent_branches_preserve_mixed_batches_and_rng() {
    // Repeated targets with different bases preserve record order, keeping
    // independent measurements ahead of the active-coordinate projections.
    let text = "H 0 64 128 192\nT 0 64\nCZ 0 128 64 192\nS 192\nM 192\nMY 192\nM !128\nMX 0\nMY 64\nDETECTOR rec[-1] rec[-3]\nOBSERVABLE_INCLUDE(0) rec[-2]";
    let executor = NearCliffordExecutor::compile_text(text).unwrap();
    let mut sampler = executor.prepare_sampler().unwrap();
    let mut a = StdRng::seed_from_u64(1439);
    let mut b = StdRng::seed_from_u64(1439);
    for (index, shots) in [0, 1, 64, 256].into_iter().enumerate() {
        let expected = (0..shots)
            .map(|_| executor.run(&mut b).unwrap())
            .collect::<Vec<_>>();
        if index % 2 == 0 {
            assert_eq!(sampler.sample(shots, &mut a).unwrap(), expected);
        } else {
            let expected = expected
                .iter()
                .flat_map(|s| s.measurements.iter().map(|&v| u8::from(v)))
                .collect::<Vec<_>>();
            assert_eq!(
                sampler
                    .sample_measurements_u8(shots, &mut a)
                    .unwrap()
                    .measurements,
                expected
            );
        }
    }
    assert_eq!(a.next_u64(), b.next_u64());
}
