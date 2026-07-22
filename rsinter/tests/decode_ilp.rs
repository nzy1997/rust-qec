#![cfg(feature = "ilp-runner")]

use std::collections::HashMap;

use rilpqec::{BackendConfig, BackendKind, IlpDecoderConfig};
use rsinter::collect::{collect, CollectOptions};
use rsinter::decode::{Decoder, IlpDemDecoder as RsinterIlpDemDecoder};
use rsinter::task::{CollectionOptions, Task};
use rstim::dem::DetectorErrorModel;
use rstim::error_analyzer::ErrorAnalyzer;
use rstim::parser::parse_lines;

#[test]
fn ilp_dem_decoder_predicts_a_single_observable_flip() {
    let dem = DetectorErrorModel::parse("error(0.125) D0 L0\nerror(0.25) D1\n").unwrap();
    let decoder = RsinterIlpDemDecoder::default();
    let compiled = decoder.compile_for_dem(&dem).unwrap();

    let predictions = compiled
        .decode_shots_bit_packed(&[0b0000_0001], 1, 2, 1)
        .unwrap();

    assert_eq!(predictions, vec![0b0000_0001]);
}

#[test]
fn ilp_dem_decoder_handles_observable_only_terms() {
    let dem = DetectorErrorModel::parse("error(0.75) L0\n").unwrap();
    let decoder = RsinterIlpDemDecoder::default();
    let compiled = decoder.compile_for_dem(&dem).unwrap();

    let predictions = compiled.decode_shots_bit_packed(&[], 1, 0, 1).unwrap();

    assert_eq!(predictions, vec![0b0000_0001]);
}

#[test]
fn collect_runs_with_the_ilp_adapter() {
    let circuit =
        parse_lines("R 0\nX_ERROR(0.05) 0\nM 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n")
            .unwrap();
    let dem = ErrorAnalyzer::circuit_to_dem(&circuit).unwrap();

    let task = Task {
        circuit,
        decoder: "ilp".into(),
        dem,
        metadata: serde_json::json!({"case": "single-qubit"}),
        collection_options: CollectionOptions {
            max_shots: Some(32),
            max_errors: Some(32),
            max_wall_seconds: None,
        },
    };

    let mut decoders: HashMap<String, Box<dyn Decoder>> = HashMap::new();
    decoders.insert("ilp".into(), Box::new(RsinterIlpDemDecoder::default()));

    let results = collect(
        vec![task],
        decoders,
        &CollectOptions {
            num_workers: 1,
            max_shots: None,
            max_errors: None,
            max_wall_seconds: None,
            max_batch_size: Some(32),
            start_batch_size: 8,
            save_resume_filepath: None,
            print_progress: false,
        },
    )
    .unwrap();

    assert_eq!(results.len(), 1);
    assert!(results[0].shots > 0);
}

#[cfg(feature = "gurobi")]
#[test]
fn ilp_dem_decoder_can_explicitly_use_gurobi_through_rsinter() {
    let dem = DetectorErrorModel::parse("error(0.125) D0 L0\nerror(0.25) D1\n").unwrap();
    let decoder = RsinterIlpDemDecoder::new(IlpDecoderConfig {
        backend: BackendConfig {
            kind: BackendKind::Gurobi,
            time_limit_seconds: None,
            mip_gap: None,
            threads: Some(1),
            verbose: false,
        },
    });
    let compiled = decoder.compile_for_dem(&dem).unwrap();

    let predictions = compiled
        .decode_shots_bit_packed(&[0b0000_0001], 1, 2, 1)
        .unwrap();

    assert_eq!(predictions, vec![0b0000_0001]);
}

#[cfg(not(feature = "gurobi"))]
#[test]
fn ilp_dem_decoder_reports_unavailable_gurobi_backend() {
    let dem = DetectorErrorModel::parse("error(0.125) D0 L0\n").unwrap();
    let decoder = RsinterIlpDemDecoder::new(IlpDecoderConfig {
        backend: BackendConfig {
            kind: BackendKind::Gurobi,
            time_limit_seconds: None,
            mip_gap: None,
            threads: Some(1),
            verbose: false,
        },
    });
    let compiled = decoder.compile_for_dem(&dem).unwrap();

    let err = compiled
        .decode_shots_bit_packed(&[0b0000_0001], 1, 1, 1)
        .unwrap_err();

    assert!(
        err.contains("no ILP backend is available"),
        "error was: {err}"
    );
}
