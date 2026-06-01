use rilpqec::{BackendConfig, BackendKind, IlpDecoderConfig};

#[test]
fn default_decoder_config_prefers_auto_backend() {
    let cfg = IlpDecoderConfig::default();

    assert_eq!(cfg.backend.kind, BackendKind::Auto);
    assert_eq!(cfg.backend.time_limit_seconds, None);
    assert_eq!(cfg.backend.mip_gap, None);
    assert_eq!(cfg.backend.threads, None);
    assert!(!cfg.backend.verbose);
}

#[test]
fn explicit_backend_config_is_copyable_and_comparable() {
    let cfg = BackendConfig {
        kind: BackendKind::Highs,
        time_limit_seconds: Some(12.5),
        mip_gap: Some(0.01),
        threads: Some(4),
        verbose: true,
    };

    let expected = BackendConfig {
        kind: BackendKind::Highs,
        time_limit_seconds: Some(12.5),
        mip_gap: Some(0.01),
        threads: Some(4),
        verbose: true,
    };

    assert_eq!(cfg, expected);
}
