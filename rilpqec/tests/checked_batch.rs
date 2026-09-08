use rilpqec::{IlpDecodeError, IlpDecoderConfig, IlpDemDecoder};
use rstim::dem::DetectorErrorModel;

fn check_both(
    model: &str,
    dets: &[u8],
    shots: usize,
    detectors: usize,
    observables: usize,
    expected: Result<Vec<u8>, IlpDecodeError>,
) {
    let dem = DetectorErrorModel::parse(model).unwrap();
    let decoder = IlpDemDecoder::from_dem(&dem, IlpDecoderConfig::default()).unwrap();
    assert_eq!(
        decoder.decode_batch_bit_packed(dets, shots, detectors, observables),
        expected
    );
    let mut compiled = decoder.into_compiled().unwrap();
    assert_eq!(
        compiled.decode_batch_bit_packed(dets, shots, detectors, observables),
        expected
    );
}

#[test]
fn deterministic_models_reject_impossible_syndromes() {
    for (model, valid, prediction) in [("error(0) D0 L0\n", 0, 0), ("error(1) D0 L0\n", 1, 1)] {
        check_both(model, &[valid; 3], 3, 1, 1, Ok(vec![prediction; 3]));
        check_both(
            model,
            &[valid, valid ^ 1, valid],
            3,
            1,
            1,
            Err(IlpDecodeError::InfeasibleSyndrome { shot: 1 }),
        );
    }
}

#[test]
fn deterministic_batches_handle_byte_boundaries_and_ignore_padding() {
    let model = "error(1) D0 D8 L0 L8\nerror(0) D15 L15\n";
    check_both(
        model,
        &[1, 1, 1, 1, 1, 1],
        3,
        16,
        16,
        Ok(vec![1, 1, 1, 1, 1, 1]),
    );
    check_both(
        model,
        &[1, 1, 1, 1, 1, 0x81],
        3,
        16,
        16,
        Err(IlpDecodeError::InfeasibleSyndrome { shot: 2 }),
    );
    check_both("error(1) D8 L8\n", &[0, 0xff], 1, 9, 9, Ok(vec![0, 1]));
}

#[test]
fn packed_buffer_sizes_are_checked_before_allocation_or_backend_use() {
    for (model, detectors, observables, buffer, bits) in [
        ("error(0) D15\n", 16, 0, "detectors", 16),
        ("error(0) L15\n", 0, 16, "observables", 16),
        ("error(0) L0\n", 0, 1, "observables", 1),
    ] {
        check_both(
            model,
            &[],
            usize::MAX,
            detectors,
            observables,
            Err(IlpDecodeError::PackedBufferSizeOverflow {
                buffer,
                shots: usize::MAX,
                bits,
            }),
        );
    }
}

#[test]
fn empty_batches_and_empty_models_are_valid() {
    check_both("error(1) D8 L8\n", &[], 0, 9, 9, Ok(vec![]));
    check_both("", &[], usize::MAX, 0, 0, Ok(vec![]));
    check_both("error(1) L8\n", &[], 3, 0, 9, Ok(vec![0, 1, 0, 1, 0, 1]));
    check_both("error(0) D8\n", &[0, 0, 0, 0], 2, 9, 0, Ok(vec![]));
    check_both(
        "error(0) D8\n",
        &[0, 0, 0, 1],
        2,
        9,
        0,
        Err(IlpDecodeError::InfeasibleSyndrome { shot: 1 }),
    );
}

#[test]
fn compiled_decoder_remains_usable_after_rejected_batch() {
    let dem = DetectorErrorModel::parse("error(1) D0 L0\n").unwrap();
    let mut decoder = IlpDemDecoder::from_dem(&dem, IlpDecoderConfig::default())
        .unwrap()
        .into_compiled()
        .unwrap();
    assert_eq!(
        decoder.decode_batch_bit_packed(&[0], 1, 1, 1),
        Err(IlpDecodeError::InfeasibleSyndrome { shot: 0 })
    );
    assert_eq!(decoder.decode_batch_bit_packed(&[1], 1, 1, 1), Ok(vec![1]));
}
