use qec_code::QecError;
use qec_code::regular_classical::{
    REGULAR_CLASSICAL_MATRIX_ALGORITHM_V1, RegularClassicalMatrixConfig, SplitMix64V1,
    bounded_index_v1, deterministic_regular_matrix,
};

fn fixture_config(seed: u64) -> RegularClassicalMatrixConfig {
    RegularClassicalMatrixConfig {
        column_count: 6,
        row_count: 4,
        column_weight: 2,
        row_weight: 3,
        seed,
        algorithm_version: REGULAR_CLASSICAL_MATRIX_ALGORITHM_V1,
        retry_limit: 16,
    }
}

fn assert_regular_degrees(
    rows: &[Vec<usize>],
    column_count: usize,
    row_count: usize,
    row_weight: usize,
    column_weight: usize,
) {
    assert_eq!(rows.len(), row_count);
    let mut column_degrees = vec![0; column_count];
    for row in rows {
        assert_eq!(row.len(), row_weight);
        let mut sorted = row.clone();
        sorted.sort_unstable();
        assert_eq!(&sorted, row);
        for &column in row {
            assert!(column < column_count);
            column_degrees[column] += 1;
        }
    }
    assert_eq!(column_degrees, vec![column_weight; column_count]);
}

#[test]
fn deterministic_regular_matrix_matches_v1_fixture() {
    let first = deterministic_regular_matrix(fixture_config(7)).unwrap();
    let second = deterministic_regular_matrix(fixture_config(7)).unwrap();
    assert_eq!(first, second);
    assert_eq!(
        first,
        vec![
            vec![0, 1, 2],
            vec![0, 3, 4],
            vec![1, 3, 5],
            vec![2, 4, 5],
        ]
    );
    assert_regular_degrees(&first, 6, 4, 3, 2);

    let seed8 = deterministic_regular_matrix(fixture_config(8)).unwrap();
    assert_ne!(seed8, first);
    assert_regular_degrees(&seed8, 6, 4, 3, 2);
}

#[test]
fn deterministic_regular_matrix_rejects_invalid_degrees() {
    let mut config = fixture_config(7);
    config.column_count = 0;
    assert!(matches!(
        deterministic_regular_matrix(config),
        Err(QecError::InvalidRegularClassicalMatrixConfig {
            option: "column_count",
            ..
        })
    ));

    let mut config = fixture_config(7);
    config.row_count = 0;
    assert!(matches!(
        deterministic_regular_matrix(config),
        Err(QecError::InvalidRegularClassicalMatrixConfig {
            option: "row_count",
            ..
        })
    ));

    let mut config = fixture_config(7);
    config.column_weight = 0;
    assert!(matches!(
        deterministic_regular_matrix(config),
        Err(QecError::InvalidRegularClassicalMatrixConfig {
            option: "column_weight",
            ..
        })
    ));

    let mut config = fixture_config(7);
    config.row_weight = 0;
    assert!(matches!(
        deterministic_regular_matrix(config),
        Err(QecError::InvalidRegularClassicalMatrixConfig {
            option: "row_weight",
            ..
        })
    ));

    let mut config = fixture_config(7);
    config.retry_limit = 0;
    assert!(matches!(
        deterministic_regular_matrix(config),
        Err(QecError::InvalidRegularClassicalMatrixConfig {
            option: "retry_limit",
            ..
        })
    ));

    let mut config = fixture_config(7);
    config.algorithm_version = 2;
    assert!(matches!(
        deterministic_regular_matrix(config),
        Err(QecError::UnsupportedRegularClassicalMatrixAlgorithm {
            algorithm_version: 2
        })
    ));

    let mut config = fixture_config(7);
    config.row_count = 1;
    assert!(matches!(
        deterministic_regular_matrix(config),
        Err(QecError::InvalidRegularClassicalMatrixConfig {
            option: "column_weight",
            ..
        })
    ));

    let mut config = fixture_config(7);
    config.column_count = 2;
    assert!(matches!(
        deterministic_regular_matrix(config),
        Err(QecError::InvalidRegularClassicalMatrixConfig {
            option: "row_weight",
            ..
        })
    ));

    let mut config = fixture_config(7);
    config.column_count = 5;
    assert!(matches!(
        deterministic_regular_matrix(config),
        Err(QecError::RegularClassicalMatrixStubCountMismatch {
            column_stubs: 10,
            row_stubs: 12,
        })
    ));

    let mut config = fixture_config(7);
    config.column_count = usize::MAX;
    assert!(matches!(
        deterministic_regular_matrix(config),
        Err(QecError::RegularClassicalMatrixStubCountOverflow { side: "column" })
    ));
}

#[test]
fn splitmix64_v1_seed7_matches_golden_words() {
    let mut stream = SplitMix64V1::new(7);
    let words = (0..8).map(|_| stream.next_u64()).collect::<Vec<_>>();
    assert_eq!(
        words,
        vec![
            0x63CBE1E459320DD7,
            0x044C3CD7F43C661C,
            0xE6984080BAB12A02,
            0x953AEB70673E29CB,
            0x73D33B666A1E21DA,
            0x3FDABE86CBBEAA11,
            0x77CBC4A133C2D0F6,
            0x53FCD6513D02BEFE,
        ]
    );

    let mut zero_bound_stream = SplitMix64V1::new(7);
    assert_eq!(bounded_index_v1(&mut zero_bound_stream, 0), None);
    assert_eq!(zero_bound_stream.state(), 7);

    let mut bounded = SplitMix64V1::new(7);
    let values = (0..8)
        .map(|_| bounded_index_v1(&mut bounded, 10).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(values, vec![7, 4, 6, 3, 4, 5, 8, 2]);

    let mut rejection = SplitMix64V1::new(7);
    assert_eq!(
        bounded_index_v1(&mut rejection, (1u64 << 63) + 1),
        Some(7_392_729_709_960_833_537)
    );
}

#[test]
fn deterministic_regular_matrix_retry_limit_one_returns_exhausted() {
    let config = RegularClassicalMatrixConfig {
        column_count: 3,
        row_count: 3,
        column_weight: 2,
        row_weight: 2,
        seed: 1,
        algorithm_version: REGULAR_CLASSICAL_MATRIX_ALGORITHM_V1,
        retry_limit: 1,
    };

    assert!(matches!(
        deterministic_regular_matrix(config),
        Err(QecError::RegularClassicalMatrixGenerationExhausted {
            retry_limit: 1,
            attempts: 1,
            ..
        })
    ));
}
