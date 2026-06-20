#[path = "../dev/fixture_catalog.rs"]
mod fixture_catalog;

#[test]
fn fixture_catalog_manifest_covers_all_checked_in_lsd_and_bp_cases() {
    let catalog = fixture_catalog::load_catalog(&fixture_catalog::catalog_path());
    let entries =
        fixture_catalog::validate_catalog(&catalog, &fixture_catalog::fixture_root()).unwrap();

    let ids = entries
        .iter()
        .map(|entry| entry.id.as_str())
        .collect::<Vec<_>>();

    assert_eq!(
        ids,
        vec![
            "bp_product_sum_serial_sensitive",
            "lsd_order_one_improves_over_baseline",
            "lsd_small_sparse_code",
            "lsd_unsatisfiable_case",
        ]
    );
}

#[test]
fn fixture_catalog_rejects_missing_provenance_or_verifier() {
    let valid = fixture_catalog::load_catalog(&fixture_catalog::catalog_path());

    let mut missing_provenance = valid.clone();
    missing_provenance.fixtures[0].provenance.clear();
    let error =
        fixture_catalog::validate_catalog(&missing_provenance, &fixture_catalog::fixture_root())
            .unwrap_err();
    assert!(
        error.contains("provenance"),
        "expected provenance validation error, got {error:?}"
    );

    let mut missing_verifier = valid.clone();
    missing_verifier.fixtures[0].verifier.clear();
    let error =
        fixture_catalog::validate_catalog(&missing_verifier, &fixture_catalog::fixture_root())
            .unwrap_err();
    assert!(
        error.contains("verifier"),
        "expected verifier validation error, got {error:?}"
    );
}
