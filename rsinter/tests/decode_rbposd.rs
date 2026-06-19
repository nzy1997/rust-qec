use std::collections::HashMap;

use rbposd::{DecoderConfig, LsdConfig};
use rsinter::collect::{CollectOptions, collect};
use rsinter::decode::{Decoder, RbposdDemDecoder, RbposdLsdDemDecoder};
use rsinter::task::{CollectionOptions, Task};
use rstim::dem::{DemTarget, DetectorErrorModel};
use rstim::error_analyzer::ErrorAnalyzer;
use rstim::parser::parse_lines;

#[test]
fn rbposd_dem_decoder_predicts_a_single_observable_flip() {
    let dem = DetectorErrorModel::parse("error(0.125) D0 L0\nerror(0.25) D1\n").unwrap();
    let decoder = RbposdDemDecoder::new(DecoderConfig::default());
    let compiled = decoder.compile_for_dem(&dem).unwrap();

    let predictions = compiled
        .decode_shots_bit_packed(&[0b0000_0001], 1, 2, 1)
        .unwrap();

    assert_eq!(predictions, vec![0b0000_0001]);
}

#[test]
fn lsd_dem_decoder_predicts_a_known_single_observable_flip() {
    let dem = DetectorErrorModel::parse("error(0.125) D0 L0\nerror(0.25) D1\n").unwrap();
    let decoder = RbposdLsdDemDecoder::new(LsdConfig::default());
    let compiled = decoder.compile_for_dem(&dem).unwrap();

    let predictions = compiled
        .decode_shots_bit_packed(&[0b0000_0001], 1, 2, 1)
        .unwrap();

    assert_eq!(predictions, vec![0b0000_0001]);
}

#[test]
fn lsd_dem_decoder_returns_compile_error_for_invalid_matrix_problem() {
    let mut dem = DetectorErrorModel::new();
    dem.add_error(
        f64::NAN,
        vec![DemTarget::Detector(0), DemTarget::Observable(0)],
    );
    let decoder = RbposdLsdDemDecoder::new(LsdConfig::default());

    let err = match decoder.compile_for_dem(&dem) {
        Ok(_) => panic!("expected invalid LSD DEM compile to fail"),
        Err(err) => err,
    };

    assert!(
        err.contains("failed to compile rbposd decoder"),
        "expected rbposd compile error, got {err:?}"
    );
}

#[test]
fn rbposd_dem_decoder_reuses_one_compiled_instance_across_multiple_batch_calls() {
    let dem = DetectorErrorModel::parse("error(0.125) D0 L0\nerror(0.25) D1\n").unwrap();
    let decoder = RbposdDemDecoder::new(DecoderConfig::default());
    let compiled = decoder.compile_for_dem(&dem).unwrap();

    let first = compiled
        .decode_shots_bit_packed(&[0b0000_0001], 1, 2, 1)
        .unwrap();
    let second = compiled
        .decode_shots_bit_packed(&[0b0000_0000], 1, 2, 1)
        .unwrap();
    let third = compiled
        .decode_shots_bit_packed(&[0b0000_0001, 0b0000_0000], 2, 2, 1)
        .unwrap();

    assert_eq!(first, vec![0b0000_0001]);
    assert_eq!(second, vec![0b0000_0000]);
    assert_eq!(third, vec![0b0000_0001, 0b0000_0000]);
}

#[test]
fn collect_runs_with_the_rbposd_adapter() {
    let circuit =
        parse_lines("R 0\nX_ERROR(0.05) 0\nM 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n")
            .unwrap();
    let dem = ErrorAnalyzer::circuit_to_dem(&circuit).unwrap();

    let task = Task {
        circuit,
        decoder: "rbposd".into(),
        dem,
        metadata: serde_json::json!({"case": "single-qubit"}),
        collection_options: CollectionOptions {
            max_shots: Some(32),
            max_errors: Some(32),
            max_wall_seconds: None,
        },
    };

    let mut decoders: HashMap<String, Box<dyn Decoder>> = HashMap::new();
    decoders.insert(
        "rbposd".into(),
        Box::new(RbposdDemDecoder::new(DecoderConfig::default())),
    );

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

#[test]
fn rbposd_dem_decoder_handles_observable_only_terms() {
    let dem = DetectorErrorModel::parse("error(0.75) L0\n").unwrap();
    let decoder = RbposdDemDecoder::new(DecoderConfig::default());
    let compiled = decoder.compile_for_dem(&dem).unwrap();

    let predictions = compiled.decode_shots_bit_packed(&[], 1, 0, 1).unwrap();

    assert_eq!(predictions, vec![0b0000_0001]);
}

#[test]
fn rbposd_dem_decoder_handles_exact_probability_terms() {
    let dem = DetectorErrorModel::parse("error(1) D0 L0\nerror(0) D1\n").unwrap();
    let decoder = RbposdDemDecoder::new(DecoderConfig::default());
    let compiled = decoder.compile_for_dem(&dem).unwrap();

    let predictions = compiled
        .decode_shots_bit_packed(&[0b0000_0001], 1, 2, 1)
        .unwrap();

    assert_eq!(predictions, vec![0b0000_0001]);
}

#[test]
fn rbposd_dem_decoder_handles_zero_syndrome_map_cases() {
    let dem = DetectorErrorModel::parse("error(0.9) D0 L0\nerror(0.2) D0\n").unwrap();
    let decoder = RbposdDemDecoder::new(DecoderConfig::default());
    let compiled = decoder.compile_for_dem(&dem).unwrap();

    let predictions = compiled
        .decode_shots_bit_packed(&[0b0000_0000], 1, 1, 1)
        .unwrap();

    assert_eq!(predictions, vec![0b0000_0001]);
}

#[test]
fn rbposd_osd_order_changes_ler() {
    let dem = DetectorErrorModel::parse(concat!(
        "error(0.2689414213699951) D0\n",
        "error(0.2689414213699951) D1\n",
        "error(0.18242552380635635) D0 D1 L0\n",
    ))
    .unwrap();

    let order0_ler = exact_three_error_logical_error_rate(&dem, 0);
    let order10_ler = exact_three_error_logical_error_rate(&dem, 10);

    assert!(
        order10_ler < order0_ler,
        "expected osd_order=10 to improve LER: order0={order0_ler}, order10={order10_ler}"
    );
}

#[test]
fn rbposd_lsd_order_changes_logical_error_rate() {
    let dem = DetectorErrorModel::parse(concat!(
        "error(0.05) D0\n",
        "error(0.1) D0 L0\n",
        "error(0.1) D0\n",
    ))
    .unwrap();

    let order0_ler = exact_three_error_lsd_logical_error_rate(&dem, 0);
    let order1_ler = exact_three_error_lsd_logical_error_rate(&dem, 1);

    assert!(
        (order0_ler - 0.14).abs() < 1e-12,
        "expected exact order0 LER from fixture to be 0.14, got {order0_ler}"
    );
    assert!(
        (order1_ler - 0.1).abs() < 1e-12,
        "expected exact order1 LER from fixture to be 0.1, got {order1_ler}"
    );
    assert!(
        order1_ler < order0_ler,
        "expected lsd_order=1 to improve LER: order0={order0_ler}, order1={order1_ler}"
    );
}

fn exact_three_error_lsd_logical_error_rate(dem: &DetectorErrorModel, lsd_order: usize) -> f64 {
    let lsd_config = LsdConfig {
        lsd_order,
        ..LsdConfig::default()
    };
    let mut bp_config = DecoderConfig::default();
    bp_config.max_bp_iterations = 0;
    let decoder = RbposdLsdDemDecoder::with_bp_config(lsd_config, bp_config);
    let compiled = decoder.compile_for_dem(dem).unwrap();
    let probabilities = [
        0.05,
        0.1,
        0.1,
    ];
    let mut ler = 0.0;
    for e0 in [false, true] {
        for e1 in [false, true] {
            for e2 in [false, true] {
                let event = [e0, e1, e2];
                let probability = event
                    .iter()
                    .zip(probabilities.iter())
                    .map(|(&fired, &p)| if fired { p } else { 1.0 - p })
                    .product::<f64>();
                let det0 = e0 ^ e1 ^ e2;
                let observed = e1;
                let det_byte = u8::from(det0);
                let predicted = compiled
                    .decode_shots_bit_packed(&[det_byte], 1, 1, 1)
                    .unwrap()[0]
                    & 1
                    != 0;
                if predicted != observed {
                    ler += probability;
                }
            }
        }
    }
    ler
}

fn exact_three_error_logical_error_rate(dem: &DetectorErrorModel, osd_order: usize) -> f64 {
    let mut config = DecoderConfig::default();
    config.max_bp_iterations = 0;
    config.osd_order = osd_order;
    let decoder = RbposdDemDecoder::new(config);
    let compiled = decoder.compile_for_dem(dem).unwrap();
    let probabilities = [
        0.268_941_421_369_995_1,
        0.268_941_421_369_995_1,
        0.182_425_523_806_356_35,
    ];
    let mut ler = 0.0;
    for e0 in [false, true] {
        for e1 in [false, true] {
            for e2 in [false, true] {
                let event = [e0, e1, e2];
                let probability = event
                    .iter()
                    .zip(probabilities.iter())
                    .map(|(&fired, &p)| if fired { p } else { 1.0 - p })
                    .product::<f64>();
                let det0 = e0 ^ e2;
                let det1 = e1 ^ e2;
                let observed = e2;
                let det_byte = u8::from(det0) | (u8::from(det1) << 1);
                let predicted = compiled
                    .decode_shots_bit_packed(&[det_byte], 1, 2, 1)
                    .unwrap()[0]
                    & 1
                    != 0;
                if predicted != observed {
                    ler += probability;
                }
            }
        }
    }
    ler
}
