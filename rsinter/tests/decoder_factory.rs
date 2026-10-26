use rsinter::replay::build_decoder;

#[test]
fn factory_rejects_unknown_decoders() {
    assert!(
        build_decoder("unknown", "")
            .err()
            .unwrap()
            .contains("unknown decoder")
    );
}

#[cfg(feature = "rmatching-runner")]
#[test]
fn factory_decodes_in_process_and_checks_configuration_and_shape() {
    let (decoder, normalized) = build_decoder("rmatching", "").unwrap();
    assert_eq!(normalized, serde_json::json!({}));
    assert!(build_decoder("rmatching", "unexpected = true").is_err());
    let dem = rstim::dem::DetectorErrorModel::parse("error(0.1) D0 L0").unwrap();
    let compiled = decoder.compile_for_dem(&dem).unwrap();
    assert_eq!(
        compiled.decode_shots_bit_packed(&[0, 1], 2, 1, 1).unwrap(),
        vec![0, 1]
    );
    assert!(compiled.decode_shots_bit_packed(&[], 2, 1, 1).is_err());
}
