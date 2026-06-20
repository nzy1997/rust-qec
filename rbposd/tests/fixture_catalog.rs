#[path = "../dev/fixture_catalog.rs"]
mod fixture_catalog;

use std::fs;
use std::path::Path;

use serde_json::{json, Value};

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

#[test]
fn fixture_catalog_rejects_unsupported_bp_option_modes_and_config() {
    let valid = fixture_catalog::load_catalog(&fixture_catalog::catalog_path());

    let mut wrong_decoder = valid.clone();
    let bp_entry = bp_entry_mut(&mut wrong_decoder);
    bp_entry.decoder = "bp_lsd".to_string();
    let error = fixture_catalog::validate_catalog(&wrong_decoder, &fixture_catalog::fixture_root())
        .unwrap_err();
    assert!(
        error.contains("decoder"),
        "expected decoder validation error, got {error:?}"
    );

    let mut missing_required_mode = valid.clone();
    bp_entry_mut(&mut missing_required_mode)
        .modes
        .retain(|mode| mode != "schedule=serial");
    let error =
        fixture_catalog::validate_catalog(&missing_required_mode, &fixture_catalog::fixture_root())
            .unwrap_err();
    assert!(
        error.contains("schedule"),
        "expected schedule mode validation error, got {error:?}"
    );

    let mut unsupported_mode_value = valid.clone();
    let bp_entry = bp_entry_mut(&mut unsupported_mode_value);
    replace_mode(
        &mut bp_entry.modes,
        "bp_variant=product_sum",
        "bp_variant=normalized_min_sum",
    );
    let error = fixture_catalog::validate_catalog(
        &unsupported_mode_value,
        &fixture_catalog::fixture_root(),
    )
    .unwrap_err();
    assert!(
        error.contains("bp_variant"),
        "expected bp_variant validation error, got {error:?}"
    );

    let error = validate_catalog_with_fixture_copy(
        &valid,
        "bp_product_sum_serial_sensitive.json",
        |fixture| {
            fixture["config"]["schedule"] = json!("layered");
        },
    )
    .unwrap_err();
    assert!(
        error.contains("schedule"),
        "expected unsupported schedule validation error, got {error:?}"
    );

    let error = validate_catalog_with_fixture_copy(
        &valid,
        "bp_product_sum_serial_sensitive.json",
        |fixture| {
            fixture["config"]["bp_variant"] = json!("minimum_sum");
            fixture["config"]["schedule"] = json!("parallel");
        },
    )
    .unwrap_err();
    assert!(
        error.contains("non-default BP config"),
        "expected default-config validation error, got {error:?}"
    );

    let error = validate_catalog_with_fixture_copy(
        &valid,
        "bp_product_sum_serial_sensitive.json",
        |fixture| {
            fixture["config"]["early_stop"] = json!(false);
        },
    )
    .unwrap_err();
    assert!(
        error.contains("early_stop"),
        "expected early_stop validation error, got {error:?}"
    );

    let error = validate_catalog_with_fixture_copy(
        &valid,
        "bp_product_sum_serial_sensitive.json",
        |fixture| {
            fixture["config"]["osd_variant"] = json!("osd1");
        },
    )
    .unwrap_err();
    assert!(
        error.contains("osd_variant"),
        "expected osd_variant validation error, got {error:?}"
    );
}

#[test]
fn fixture_catalog_rejects_unsupported_lsd_modes_and_decoder_combinations() {
    let valid = fixture_catalog::load_catalog(&fixture_catalog::catalog_path());

    let mut wrong_decoder_tag = valid.clone();
    let lsd_entry = lsd_entry_mut(&mut wrong_decoder_tag, "lsd_small_sparse_code");
    lsd_entry.decoder = "bp_osd".to_string();
    let error =
        fixture_catalog::validate_catalog(&wrong_decoder_tag, &fixture_catalog::fixture_root())
            .unwrap_err();
    assert!(
        error.contains("decoder"),
        "expected decoder validation error, got {error:?}"
    );

    let mut missing_decoder_mode = valid.clone();
    lsd_entry_mut(&mut missing_decoder_mode, "lsd_small_sparse_code")
        .modes
        .retain(|mode| mode != "decoder=bp_lsd");
    let error =
        fixture_catalog::validate_catalog(&missing_decoder_mode, &fixture_catalog::fixture_root())
            .unwrap_err();
    assert!(
        error.contains("decoder"),
        "expected decoder mode validation error, got {error:?}"
    );

    let mut unsupported_method = valid.clone();
    let lsd_entry = lsd_entry_mut(&mut unsupported_method, "lsd_small_sparse_code");
    replace_mode(
        &mut lsd_entry.modes,
        "lsd_method=localized_statistics",
        "lsd_method=belief_propagation",
    );
    let error =
        fixture_catalog::validate_catalog(&unsupported_method, &fixture_catalog::fixture_root())
            .unwrap_err();
    assert!(
        error.contains("lsd_method"),
        "expected lsd_method validation error, got {error:?}"
    );

    let mut mismatched_order_tag = valid.clone();
    let lsd_entry = lsd_entry_mut(&mut mismatched_order_tag, "lsd_small_sparse_code");
    replace_mode(&mut lsd_entry.modes, "lsd_order=1", "lsd_order=0");
    let error =
        fixture_catalog::validate_catalog(&mismatched_order_tag, &fixture_catalog::fixture_root())
            .unwrap_err();
    assert!(
        error.contains("lsd_order"),
        "expected lsd_order mode validation error, got {error:?}"
    );

    let error =
        validate_catalog_with_fixture_copy(&valid, "lsd_small_sparse_code.json", |fixture| {
            fixture["lsd_order"] = json!(2);
        })
        .unwrap_err();
    assert!(
        error.contains("lsd_order"),
        "expected unsupported lsd_order validation error, got {error:?}"
    );
}

fn bp_entry_mut(
    catalog: &mut fixture_catalog::FixtureCatalog,
) -> &mut fixture_catalog::FixtureCatalogEntry {
    catalog
        .fixtures
        .iter_mut()
        .find(|entry| entry.kind == fixture_catalog::FixtureKind::BpOption)
        .unwrap()
}

fn lsd_entry_mut<'a>(
    catalog: &'a mut fixture_catalog::FixtureCatalog,
    id: &str,
) -> &'a mut fixture_catalog::FixtureCatalogEntry {
    catalog
        .fixtures
        .iter_mut()
        .find(|entry| entry.kind == fixture_catalog::FixtureKind::Lsd && entry.id == id)
        .unwrap()
}

fn replace_mode(modes: &mut [String], from: &str, to: &str) {
    let mode = modes.iter_mut().find(|mode| mode.as_str() == from).unwrap();
    *mode = to.to_string();
}

fn validate_catalog_with_fixture_copy(
    catalog: &fixture_catalog::FixtureCatalog,
    fixture_name: &str,
    mutate_fixture: impl FnOnce(&mut Value),
) -> Result<Vec<fixture_catalog::ValidatedFixtureCatalogEntry>, String> {
    let fixture_root = fixture_catalog::fixture_root();
    let temp_root = unique_temp_fixture_root();
    copy_fixture_tree(&fixture_root, &temp_root);

    let group = if fixture_name.starts_with("bp_") {
        "parity"
    } else {
        "lsd"
    };
    let temp_fixture_path = temp_root.join(group).join(fixture_name);
    let mut fixture: Value =
        serde_json::from_str(&fs::read_to_string(&temp_fixture_path).unwrap()).unwrap();
    mutate_fixture(&mut fixture);
    fs::write(
        &temp_fixture_path,
        serde_json::to_string_pretty(&fixture).unwrap(),
    )
    .unwrap();

    fixture_catalog::validate_catalog(catalog, &temp_root)
}

fn copy_fixture_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();

    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let from_path = entry.path();
        let to_path = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_fixture_tree(&from_path, &to_path);
        } else {
            fs::copy(&from_path, &to_path).unwrap();
        }
    }
}

fn unique_temp_fixture_root() -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("rbposd-fixture-catalog-{nanos}"))
}
