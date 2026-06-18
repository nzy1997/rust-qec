use std::fs;
use std::path::Path;

use rbposd::{
    BpLsdDecoder, ChannelModel, Correction, DecodeError, LsdConfig, ParityCheckMatrix, Syndrome,
};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct LsdFixture {
    id: String,
    matrix: MatrixFixture,
    channel: ChannelFixture,
    syndrome: Vec<bool>,
    lsd_order: usize,
    expected: ExpectedFixture,
}

#[derive(Debug, Deserialize)]
struct MatrixFixture {
    num_checks: usize,
    num_bits: usize,
    rows: Vec<Vec<usize>>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum ChannelFixture {
    Bsc { error_rate: f64 },
    BitFlipProbabilities { probabilities: Vec<f64> },
}

#[derive(Debug, Deserialize)]
struct ExpectedFixture {
    status: String,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    order_0_correction: Option<Vec<bool>>,
    #[serde(default)]
    order_1_correction: Option<Vec<bool>>,
}

impl LsdFixture {
    fn load(name: &str) -> Self {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("lsd")
            .join(name);
        let contents = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
        serde_json::from_str(&contents)
            .unwrap_or_else(|error| panic!("failed to parse {}: {error}", path.display()))
    }

    fn pcm(&self) -> ParityCheckMatrix {
        ParityCheckMatrix::from_sparse_rows(
            self.matrix.num_checks,
            self.matrix.num_bits,
            self.matrix.rows.clone(),
        )
        .unwrap_or_else(|error| panic!("invalid matrix in {}: {error}", self.id))
    }

    fn channel(&self) -> ChannelModel {
        match &self.channel {
            ChannelFixture::Bsc { error_rate } => ChannelModel::Bsc {
                error_rate: *error_rate,
            },
            ChannelFixture::BitFlipProbabilities { probabilities } => {
                ChannelModel::BitFlipProbabilities(probabilities.clone())
            }
        }
    }

    fn syndrome(&self) -> Syndrome {
        Syndrome::from(self.syndrome.clone())
    }

    fn lsd_config(&self) -> LsdConfig {
        LsdConfig {
            lsd_order: self.lsd_order,
            ..LsdConfig::default()
        }
    }
}

#[test]
fn bplsddecoder_public_api_matches_reference_contract() {
    let pcm = ParityCheckMatrix::from_sparse_rows(2, 3, vec![vec![0, 1], vec![1, 2]]).unwrap();
    let decoder = BpLsdDecoder::new(
        pcm.clone(),
        ChannelModel::Bsc { error_rate: 0.05 },
        LsdConfig::default(),
    )
    .unwrap();

    let syndrome = Syndrome::from(vec![true, false]);
    let result = decoder.decode(&syndrome).unwrap();

    assert!(!result.used_osd);
    assert_eq!(result.residual_syndrome_weight, 0);
    assert_eq!(pcm.multiply(&result.correction), syndrome);
}

#[test]
fn bplsddecoder_clone_preserves_decoding_behavior_with_fresh_workspaces() {
    let pcm = ParityCheckMatrix::from_sparse_rows(2, 3, vec![vec![1, 2], vec![0]]).unwrap();
    let decoder = BpLsdDecoder::new(
        pcm.clone(),
        ChannelModel::Bsc { error_rate: 0.05 },
        LsdConfig::default(),
    )
    .unwrap();

    let cloned = decoder.clone();
    let syndrome = Syndrome::from(vec![true, false]);
    let first = decoder.decode(&syndrome).unwrap();
    let second = cloned.decode(&syndrome).unwrap();

    assert_eq!(second, first);
    assert_eq!(pcm.multiply(&second.correction), syndrome);
}

#[test]
fn bplsddecoder_rejects_syndrome_length_mismatch() {
    let pcm = ParityCheckMatrix::from_sparse_rows(2, 3, vec![vec![0, 1], vec![1, 2]]).unwrap();
    let decoder = BpLsdDecoder::new(
        pcm,
        ChannelModel::Bsc { error_rate: 0.05 },
        LsdConfig::default(),
    )
    .unwrap();

    let err = decoder.decode(&Syndrome::from(vec![true])).unwrap_err();

    assert_eq!(
        err,
        DecodeError::DimensionMismatch {
            what: "syndrome",
            expected: 2,
            actual: 1,
        }
    );
}

#[test]
fn bplsddecoder_zero_syndrome_uses_prior_fast_path() {
    let pcm = ParityCheckMatrix::from_sparse_rows(1, 2, vec![vec![0, 1]]).unwrap();
    let decoder = BpLsdDecoder::new(
        pcm.clone(),
        ChannelModel::BitFlipProbabilities(vec![0.9, 0.9]),
        LsdConfig::default(),
    )
    .unwrap();

    let syndrome = Syndrome::from(vec![false]);
    let result = decoder.decode(&syndrome).unwrap();

    assert!(result.converged);
    assert!(!result.used_osd);
    assert_eq!(result.bp_iterations, 0);
    assert_eq!(result.residual_syndrome_weight, 0);
    assert_eq!(result.correction, Correction::from(vec![true, true]));
    assert_eq!(pcm.multiply(&result.correction), syndrome);
}

#[test]
fn bplsddecoder_order_zero_fallback_repairs_bp_residual_without_osd() {
    let pcm = ParityCheckMatrix::from_sparse_rows(2, 3, vec![vec![1, 2], vec![0]]).unwrap();
    let decoder = BpLsdDecoder::new(
        pcm.clone(),
        ChannelModel::Bsc { error_rate: 0.05 },
        LsdConfig::default(),
    )
    .unwrap();

    let syndrome = Syndrome::from(vec![true, false]);
    let result = decoder.decode(&syndrome).unwrap();

    assert!(!result.converged);
    assert!(!result.used_osd);
    assert_eq!(result.bp_iterations, 30);
    assert_eq!(result.residual_syndrome_weight, 0);
    assert_eq!(
        result.correction,
        Correction::from(vec![false, true, false])
    );
    assert_eq!(pcm.multiply(&result.correction), syndrome);
}

#[test]
fn bplsddecoder_rejects_channel_length_mismatch() {
    let pcm = ParityCheckMatrix::from_sparse_rows(2, 3, vec![vec![0, 1], vec![1, 2]]).unwrap();

    let err = BpLsdDecoder::new(
        pcm,
        ChannelModel::BitFlipProbabilities(vec![0.1, 0.2]),
        LsdConfig::default(),
    )
    .unwrap_err();

    assert_eq!(
        err,
        DecodeError::DimensionMismatch {
            what: "channel probabilities",
            expected: 3,
            actual: 2,
        }
    );
}

#[test]
fn bplsd_order_one_recovers_the_borrowed_small_matrix_cases() {
    for fixture_name in [
        "lsd_small_sparse_code.json",
        "lsd_order_one_improves_over_baseline.json",
    ] {
        let fixture = LsdFixture::load(fixture_name);
        assert_eq!(fixture.expected.status, "success");

        let pcm = fixture.pcm();
        let syndrome = fixture.syndrome();
        let decoder = BpLsdDecoder::new(pcm.clone(), fixture.channel(), fixture.lsd_config())
            .unwrap_or_else(|error| {
                panic!("failed to construct decoder for {}: {error}", fixture.id)
            });
        let result = decoder
            .decode(&syndrome)
            .unwrap_or_else(|error| panic!("failed to decode {}: {error}", fixture.id));

        assert!(
            !result.used_osd,
            "fixture {} unexpectedly used OSD",
            fixture.id
        );
        assert_eq!(result.residual_syndrome_weight, 0, "fixture {}", fixture.id);
        assert_eq!(
            pcm.multiply(&result.correction),
            syndrome,
            "fixture {}",
            fixture.id
        );

        if let Some(expected_order_1) = fixture.expected.order_1_correction.clone() {
            let expected_order_1 = Correction::from(expected_order_1);
            assert_eq!(
                result.correction, expected_order_1,
                "fixture {}",
                fixture.id
            );
        }

        if let Some(expected_order_0) = fixture.expected.order_0_correction.clone() {
            let order_0_decoder = BpLsdDecoder::new(
                pcm.clone(),
                fixture.channel(),
                LsdConfig {
                    lsd_order: 0,
                    ..LsdConfig::default()
                },
            )
            .unwrap_or_else(|error| {
                panic!(
                    "failed to construct order-0 decoder for {}: {error}",
                    fixture.id
                )
            });
            let order_0_result = order_0_decoder.decode(&syndrome).unwrap_or_else(|error| {
                panic!("failed order-0 decode for {}: {error}", fixture.id)
            });
            let expected_order_0 = Correction::from(expected_order_0);
            assert_eq!(
                order_0_result.correction, expected_order_0,
                "fixture {}",
                fixture.id
            );
            assert_ne!(
                result.correction, order_0_result.correction,
                "fixture {} did not exercise a distinct order-1 correction",
                fixture.id
            );
        }
    }
}

#[test]
fn bplsd_returns_a_decoder_error_for_an_unsatisfiable_case() {
    let fixture = LsdFixture::load("lsd_unsatisfiable_case.json");
    assert_eq!(fixture.expected.status, "error");
    assert_eq!(fixture.expected.error.as_deref(), Some("NoLsdSolution"));

    let pcm = fixture.pcm();
    let syndrome = fixture.syndrome();
    let decoder = BpLsdDecoder::new(pcm, fixture.channel(), fixture.lsd_config())
        .unwrap_or_else(|error| panic!("failed to construct decoder for {}: {error}", fixture.id));

    let error = decoder.decode(&syndrome).unwrap_err();

    assert_eq!(error, DecodeError::NoLsdSolution);
}

#[test]
fn bplsddecoder_rejects_lsd_order_above_first_supported_order() {
    let pcm = ParityCheckMatrix::from_sparse_rows(1, 2, vec![vec![0, 1]]).unwrap();
    let config = LsdConfig {
        lsd_order: 2,
        ..LsdConfig::default()
    };

    let err = BpLsdDecoder::new(pcm, ChannelModel::Bsc { error_rate: 0.05 }, config).unwrap_err();

    assert_eq!(err, DecodeError::UnsupportedLsdOrder { order: 2 });
}
