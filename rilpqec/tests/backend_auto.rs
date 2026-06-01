use rilpqec::{BackendConfig, BackendKind, IlpDecoderConfig, IlpDemDecoder};
#[cfg(not(feature = "gurobi"))]
use rilpqec::IlpDecodeError;
use rstim::dem::DetectorErrorModel;

#[test]
fn auto_backend_falls_back_to_highs() {
    let dem = DetectorErrorModel::parse("error(0.1) D0 L0\nerror(0.2) D1\n").unwrap();
    let decoder = IlpDemDecoder::from_dem(&dem, IlpDecoderConfig::default()).unwrap();

    let predictions = decoder
        .decode_batch_bit_packed(&[0b0000_0001], 1, 2, 1)
        .unwrap();

    assert_eq!(predictions, vec![0b0000_0001]);
}

#[cfg(not(feature = "gurobi"))]
#[test]
fn explicit_gurobi_selection_reports_unavailable_without_feature() {
    let dem = DetectorErrorModel::parse("error(0.1) D0 L0\n").unwrap();
    let decoder = IlpDemDecoder::from_dem(
        &dem,
        IlpDecoderConfig {
            backend: BackendConfig {
                kind: BackendKind::Gurobi,
                time_limit_seconds: None,
                mip_gap: None,
                threads: None,
                verbose: false,
            },
        },
    )
    .unwrap();

    let err = decoder
        .decode_batch_bit_packed(&[0b0000_0001], 1, 1, 1)
        .unwrap_err();

    assert_eq!(
        err,
        IlpDecodeError::BackendUnavailable {
            requested: BackendKind::Gurobi,
        }
    );
}

#[cfg(feature = "gurobi")]
#[test]
fn explicit_gurobi_selection_decodes_with_the_gurobi_backend() {
    let dem = DetectorErrorModel::parse("error(0.1) D0 L0\nerror(0.2) D1\n").unwrap();
    let decoder = IlpDemDecoder::from_dem(
        &dem,
        IlpDecoderConfig {
            backend: BackendConfig {
                kind: BackendKind::Gurobi,
                time_limit_seconds: None,
                mip_gap: None,
                threads: Some(1),
                verbose: false,
            },
        },
    )
    .unwrap();

    let predictions = decoder
        .decode_batch_bit_packed(&[0b0000_0001], 1, 2, 1)
        .unwrap();

    assert_eq!(predictions, vec![0b0000_0001]);
}
