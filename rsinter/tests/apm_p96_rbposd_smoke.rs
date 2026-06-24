use std::collections::BTreeSet;

use qec_code::css::{SparseRowsMatrix, sparse_rows_matrix_from_json_str};
use rbposd::{
    BpOsdDecoder, BpVariant, ChannelModel, Correction, DecoderConfig, OsdVariant,
    ParityCheckMatrix, Schedule, Syndrome,
};

const APM_P96_HX_JSON: &str = include_str!("fixtures/css/apm_p96_hx.json");
const APM_P96_HZ_JSON: &str = include_str!("fixtures/css/apm_p96_hz.json");
const APM_P96_NUM_QUBITS: usize = 1152;
const APM_P96_SEED: u64 = 0xA9_6B_50_D5_EE_D5_14_2A;
const APM_P96_ERROR_WEIGHTS: [usize; 3] = [1, 2, 3];
const APM_P96_EXPECTED_SUPPORTS: &[&[usize]] = &[&[223], &[780, 1033], &[346, 632, 921]];
const APM_P96_CHANNEL_ERROR_RATE: f64 = 0.02;

#[test]
fn apm_p96_rbposd_smoke_decodes_seeded_syndromes() {
    let hx = parse_sparse_rows(APM_P96_HX_JSON, "failed to parse APM P=96 Hx fixture");
    let hz = parse_sparse_rows(APM_P96_HZ_JSON, "failed to parse APM P=96 Hz fixture");
    assert_eq!(hx.num_cols(), APM_P96_NUM_QUBITS);
    assert_eq!(hz.num_cols(), APM_P96_NUM_QUBITS);
    assert!(!hz.rows().is_empty(), "APM P=96 Hz fixture should be loaded");

    let pcm = parity_check_from_sparse_rows(&hx);
    let decoder = BpOsdDecoder::new(
        pcm.clone(),
        ChannelModel::Bsc {
            error_rate: APM_P96_CHANNEL_ERROR_RATE,
        },
        apm_p96_decoder_config(),
    )
    .expect("failed to compile APM P=96 Hx rbposd decoder");

    let supports = seeded_error_supports(
        APM_P96_SEED,
        &APM_P96_ERROR_WEIGHTS,
        APM_P96_NUM_QUBITS,
    );
    assert_eq!(supports, expected_supports());

    let mut zero_control_left_residual = false;
    for support in supports {
        let known_error = correction_from_support(APM_P96_NUM_QUBITS, &support);
        let syndrome = pcm.multiply(&known_error);
        assert!(
            syndrome.weight() > 0,
            "seeded support {support:?} should generate a nonzero syndrome"
        );

        zero_control_left_residual |=
            residual_weight(&pcm, &Correction::zero(APM_P96_NUM_QUBITS), &syndrome) > 0;

        let result = decoder
            .decode(&syndrome)
            .expect("failed to decode seeded APM P=96 syndrome");
        assert_eq!(
            residual_weight(&pcm, &result.correction, &syndrome),
            0,
            "decoded correction should satisfy the seeded support {support:?} syndrome"
        );
    }

    assert!(
        zero_control_left_residual,
        "all-zero correction should leave a residual on at least one nonzero seeded syndrome"
    );
}

fn apm_p96_decoder_config() -> DecoderConfig {
    DecoderConfig {
        max_bp_iterations: 96,
        early_stop: true,
        bp_variant: BpVariant::MinimumSum,
        schedule: Schedule::Parallel,
        osd_variant: OsdVariant::Osd0,
        osd_order: 0,
    }
}

fn parse_sparse_rows(input: &str, error_message: &str) -> SparseRowsMatrix {
    sparse_rows_matrix_from_json_str(input).expect(error_message)
}

fn parity_check_from_sparse_rows(matrix: &SparseRowsMatrix) -> ParityCheckMatrix {
    ParityCheckMatrix::from_sparse_rows(
        matrix.rows().len(),
        matrix.num_cols(),
        matrix.rows().to_vec(),
    )
    .expect("failed to build rbposd parity matrix")
}

fn seeded_error_supports(seed: u64, weights: &[usize], num_bits: usize) -> Vec<Vec<usize>> {
    let mut state = seed;
    weights
        .iter()
        .map(|&weight| {
            let mut support = BTreeSet::new();
            while support.len() < weight {
                support.insert((splitmix64_next(&mut state) % num_bits as u64) as usize);
            }
            support.into_iter().collect()
        })
        .collect()
}

fn splitmix64_next(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn expected_supports() -> Vec<Vec<usize>> {
    APM_P96_EXPECTED_SUPPORTS
        .iter()
        .map(|support| support.to_vec())
        .collect()
}

fn correction_from_support(num_bits: usize, support: &[usize]) -> Correction {
    let mut bits = vec![false; num_bits];
    for &bit in support {
        bits[bit] = true;
    }
    Correction::from(bits)
}

fn residual_weight(
    pcm: &ParityCheckMatrix,
    correction: &Correction,
    target_syndrome: &Syndrome,
) -> usize {
    let decoded_syndrome = pcm.multiply(correction);
    decoded_syndrome
        .as_slice()
        .iter()
        .zip(target_syndrome.as_slice())
        .filter(|(decoded, target)| decoded != target)
        .count()
}
