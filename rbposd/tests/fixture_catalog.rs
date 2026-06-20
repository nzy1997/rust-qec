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
    expect_catalog_error(
        fixture_catalog::validate_catalog(&missing_provenance, &fixture_catalog::fixture_root())
            .unwrap_err(),
        "provenance",
    );

    let mut missing_verifier = valid.clone();
    missing_verifier.fixtures[0].verifier.clear();
    expect_catalog_error(
        fixture_catalog::validate_catalog(&missing_verifier, &fixture_catalog::fixture_root())
            .unwrap_err(),
        "verifier",
    );
}

#[test]
fn fixture_catalog_rejects_empty_or_duplicate_entry_metadata() {
    let valid = fixture_catalog::load_catalog(&fixture_catalog::catalog_path());

    expect_catalog_error(
        fixture_catalog::validate_catalog(
            &fixture_catalog::FixtureCatalog { fixtures: vec![] },
            &fixture_catalog::fixture_root(),
        )
        .unwrap_err(),
        "must not be empty",
    );

    let mut missing_id = valid.clone();
    missing_id.fixtures[0].id.clear();
    expect_catalog_error(
        fixture_catalog::validate_catalog(&missing_id, &fixture_catalog::fixture_root())
            .unwrap_err(),
        "id",
    );

    let mut missing_decoder = valid.clone();
    missing_decoder.fixtures[0].decoder.clear();
    expect_catalog_error(
        fixture_catalog::validate_catalog(&missing_decoder, &fixture_catalog::fixture_root())
            .unwrap_err(),
        "decoder",
    );

    let mut missing_path = valid.clone();
    missing_path.fixtures[0].path.clear();
    expect_catalog_error(
        fixture_catalog::validate_catalog(&missing_path, &fixture_catalog::fixture_root())
            .unwrap_err(),
        "path",
    );

    let mut missing_pass_condition = valid.clone();
    missing_pass_condition.fixtures[0].pass_condition.clear();
    expect_catalog_error(
        fixture_catalog::validate_catalog(
            &missing_pass_condition,
            &fixture_catalog::fixture_root(),
        )
        .unwrap_err(),
        "pass_condition",
    );

    let mut missing_issue = valid.clone();
    missing_issue.fixtures[0]
        .consumes
        .retain(|value| value != "#98");
    expect_catalog_error(
        fixture_catalog::validate_catalog(&missing_issue, &fixture_catalog::fixture_root())
            .unwrap_err(),
        "#98",
    );

    let mut duplicate_id = valid.clone();
    duplicate_id.fixtures[1].id = duplicate_id.fixtures[0].id.clone();
    expect_catalog_error(
        fixture_catalog::validate_catalog(&duplicate_id, &fixture_catalog::fixture_root())
            .unwrap_err(),
        "duplicate fixture catalog id",
    );

    let mut duplicate_path = valid.clone();
    let duplicate_fixture_path = duplicate_path.fixtures[0].path.clone();
    set_entry_path(&mut duplicate_path.fixtures[1], &duplicate_fixture_path);
    expect_catalog_error(
        fixture_catalog::validate_catalog(&duplicate_path, &fixture_catalog::fixture_root())
            .unwrap_err(),
        "duplicate fixture catalog path",
    );

    let mut missing_fixture = valid;
    set_entry_path(&mut missing_fixture.fixtures[0], "lsd/missing_fixture.json");
    expect_catalog_error(
        fixture_catalog::validate_catalog(&missing_fixture, &fixture_catalog::fixture_root())
            .unwrap_err(),
        "missing fixture",
    );
}

#[test]
fn fixture_catalog_rejects_non_exact_matrix_or_syndrome_pointers() {
    let valid = fixture_catalog::load_catalog(&fixture_catalog::catalog_path());

    let mut wrong_matrix_path = valid.clone();
    wrong_matrix_path.fixtures[0].matrix_path =
        format!("{}#/wrong", wrong_matrix_path.fixtures[0].path);
    expect_catalog_error(
        fixture_catalog::validate_catalog(&wrong_matrix_path, &fixture_catalog::fixture_root())
            .unwrap_err(),
        "matrix_path",
    );

    let mut wrong_syndrome_path = valid;
    wrong_syndrome_path.fixtures[0].syndrome_path =
        format!("{}#/wrong", wrong_syndrome_path.fixtures[0].path);
    expect_catalog_error(
        fixture_catalog::validate_catalog(&wrong_syndrome_path, &fixture_catalog::fixture_root())
            .unwrap_err(),
        "syndrome_path",
    );
}

#[test]
fn fixture_catalog_rejects_path_and_fixture_identity_mismatches() {
    let valid = fixture_catalog::load_catalog(&fixture_catalog::catalog_path());

    let error = validate_catalog_with_extra_fixture(
        &valid,
        "lsd/lsd_small_sparse_code.json",
        "parity/nested/lsd_small_sparse_code.json",
    )
    .unwrap_err();
    expect_catalog_error(error, "must point under lsd/");

    let mut wrong_lsd_fixture_id = valid.clone();
    lsd_entry_mut(&mut wrong_lsd_fixture_id, "lsd_small_sparse_code").id =
        "mismatched_lsd_fixture_id".to_string();
    expect_catalog_error(
        fixture_catalog::validate_catalog(&wrong_lsd_fixture_id, &fixture_catalog::fixture_root())
            .unwrap_err(),
        "does not match fixture id",
    );

    let error = validate_catalog_with_extra_fixture(
        &valid,
        "lsd/lsd_small_sparse_code.json",
        "lsd/nested/lsd_small_sparse_code.json",
    )
    .unwrap_err();
    expect_catalog_error(error, "no checked-in LSD fixture requirement");

    let error = validate_catalog_with_extra_fixture(
        &valid,
        "parity/bp_product_sum_serial_sensitive.json",
        "lsd/bp_product_sum_serial_sensitive.json",
    )
    .unwrap_err();
    expect_catalog_error(error, "must point under parity/");

    let mut wrong_bp_fixture_name = valid.clone();
    bp_entry_mut(&mut wrong_bp_fixture_name).id = "mismatched_bp_fixture_name".to_string();
    expect_catalog_error(
        fixture_catalog::validate_catalog(&wrong_bp_fixture_name, &fixture_catalog::fixture_root())
            .unwrap_err(),
        "does not match fixture name",
    );

    let error = validate_catalog_with_extra_fixture(
        &valid,
        "parity/bp_product_sum_serial_sensitive.json",
        "parity/nested/bp_product_sum_serial_sensitive.json",
    )
    .unwrap_err();
    expect_catalog_error(error, "no checked-in BP-option fixture requirement");

    let mut missing_catalog_entry = valid;
    missing_catalog_entry.fixtures.pop();
    expect_catalog_error(
        fixture_catalog::validate_catalog(&missing_catalog_entry, &fixture_catalog::fixture_root())
            .unwrap_err(),
        "missing fixture catalog entry",
    );
}

#[test]
fn fixture_catalog_rejects_unsupported_bp_option_modes_and_config() {
    let valid = fixture_catalog::load_catalog(&fixture_catalog::catalog_path());

    let mut wrong_decoder = valid.clone();
    let bp_entry = bp_entry_mut(&mut wrong_decoder);
    bp_entry.decoder = "bp_lsd".to_string();
    expect_catalog_error(
        fixture_catalog::validate_catalog(&wrong_decoder, &fixture_catalog::fixture_root())
            .unwrap_err(),
        "decoder",
    );

    let mut missing_required_mode = valid.clone();
    bp_entry_mut(&mut missing_required_mode)
        .modes
        .retain(|mode| mode != "schedule=serial");
    expect_catalog_error(
        fixture_catalog::validate_catalog(&missing_required_mode, &fixture_catalog::fixture_root())
            .unwrap_err(),
        "schedule",
    );

    let mut extra_early_stop_mode = valid.clone();
    bp_entry_mut(&mut extra_early_stop_mode)
        .modes
        .push("early_stop=false".to_string());
    expect_catalog_error(
        fixture_catalog::validate_catalog(&extra_early_stop_mode, &fixture_catalog::fixture_root())
            .unwrap_err(),
        "early_stop",
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
    expect_catalog_error(error, "bp_variant");

    let error = validate_catalog_with_fixture_copy(
        &valid,
        "bp_product_sum_serial_sensitive.json",
        |fixture| {
            fixture["config"]["schedule"] = json!("layered");
        },
    )
    .unwrap_err();
    expect_catalog_error(error, "schedule");

    let error = validate_catalog_with_fixture_copy(
        &valid,
        "bp_product_sum_serial_sensitive.json",
        |fixture| {
            fixture["config"]["bp_variant"] = json!("minimum_sum");
            fixture["config"]["schedule"] = json!("parallel");
        },
    )
    .unwrap_err();
    expect_catalog_error(error, "non-default BP config");

    let error = validate_catalog_with_fixture_copy(
        &valid,
        "bp_product_sum_serial_sensitive.json",
        |fixture| {
            fixture["config"]["early_stop"] = json!(false);
        },
    )
    .unwrap_err();
    expect_catalog_error(error, "early_stop");

    let error = validate_catalog_with_fixture_copy(
        &valid,
        "bp_product_sum_serial_sensitive.json",
        |fixture| {
            fixture["config"]["osd_variant"] = json!("osd1");
        },
    )
    .unwrap_err();
    expect_catalog_error(error, "osd_variant");
}

#[test]
fn fixture_catalog_rejects_mode_syntax_and_duplicate_mode_entries() {
    let valid = fixture_catalog::load_catalog(&fixture_catalog::catalog_path());

    let mut unsupported_fixture_method = valid.clone();
    let error = validate_catalog_with_fixture_copy(
        &unsupported_fixture_method,
        "bp_product_sum_serial_sensitive.json",
        |fixture| {
            fixture["config"]["bp_variant"] = json!("normalized_min_sum");
        },
    )
    .unwrap_err();
    expect_catalog_error(error, "bp_variant");

    bp_entry_mut(&mut unsupported_fixture_method).modes.clear();
    expect_catalog_error(
        fixture_catalog::validate_catalog(
            &unsupported_fixture_method,
            &fixture_catalog::fixture_root(),
        )
        .unwrap_err(),
        "modes must not be empty",
    );

    let mut missing_separator = valid.clone();
    replace_mode(
        &mut bp_entry_mut(&mut missing_separator).modes,
        "bp_variant=product_sum",
        "bp_variant",
    );
    expect_catalog_error(
        fixture_catalog::validate_catalog(&missing_separator, &fixture_catalog::fixture_root())
            .unwrap_err(),
        "key=value",
    );

    let mut empty_mode_field = valid.clone();
    replace_mode(
        &mut bp_entry_mut(&mut empty_mode_field).modes,
        "bp_variant=product_sum",
        "=product_sum",
    );
    expect_catalog_error(
        fixture_catalog::validate_catalog(&empty_mode_field, &fixture_catalog::fixture_root())
            .unwrap_err(),
        "key=value",
    );

    let mut duplicate_mode = valid;
    bp_entry_mut(&mut duplicate_mode)
        .modes
        .push("schedule=serial".to_string());
    expect_catalog_error(
        fixture_catalog::validate_catalog(&duplicate_mode, &fixture_catalog::fixture_root())
            .unwrap_err(),
        "duplicates",
    );
}

#[test]
fn fixture_catalog_rejects_unsupported_lsd_modes_and_decoder_combinations() {
    let valid = fixture_catalog::load_catalog(&fixture_catalog::catalog_path());

    let mut wrong_decoder_tag = valid.clone();
    let lsd_entry = lsd_entry_mut(&mut wrong_decoder_tag, "lsd_small_sparse_code");
    lsd_entry.decoder = "bp_osd".to_string();
    expect_catalog_error(
        fixture_catalog::validate_catalog(&wrong_decoder_tag, &fixture_catalog::fixture_root())
            .unwrap_err(),
        "decoder",
    );

    let mut missing_decoder_mode = valid.clone();
    lsd_entry_mut(&mut missing_decoder_mode, "lsd_small_sparse_code")
        .modes
        .retain(|mode| mode != "decoder=bp_lsd");
    expect_catalog_error(
        fixture_catalog::validate_catalog(&missing_decoder_mode, &fixture_catalog::fixture_root())
            .unwrap_err(),
        "decoder",
    );

    let mut unknown_extra_mode = valid.clone();
    lsd_entry_mut(&mut unknown_extra_mode, "lsd_small_sparse_code")
        .modes
        .push("unknown_mode=value".to_string());
    expect_catalog_error(
        fixture_catalog::validate_catalog(&unknown_extra_mode, &fixture_catalog::fixture_root())
            .unwrap_err(),
        "unknown_mode",
    );

    let mut unsupported_method = valid.clone();
    let lsd_entry = lsd_entry_mut(&mut unsupported_method, "lsd_small_sparse_code");
    replace_mode(
        &mut lsd_entry.modes,
        "lsd_method=localized_statistics",
        "lsd_method=belief_propagation",
    );
    expect_catalog_error(
        fixture_catalog::validate_catalog(&unsupported_method, &fixture_catalog::fixture_root())
            .unwrap_err(),
        "lsd_method",
    );

    let mut mismatched_order_tag = valid.clone();
    let lsd_entry = lsd_entry_mut(&mut mismatched_order_tag, "lsd_small_sparse_code");
    replace_mode(&mut lsd_entry.modes, "lsd_order=1", "lsd_order=0");
    expect_catalog_error(
        fixture_catalog::validate_catalog(&mismatched_order_tag, &fixture_catalog::fixture_root())
            .unwrap_err(),
        "lsd_order",
    );

    let error =
        validate_catalog_with_fixture_copy(&valid, "lsd_small_sparse_code.json", |fixture| {
            fixture["lsd_order"] = json!(2);
        })
        .unwrap_err();
    expect_catalog_error(error, "lsd_order");
}

fn expect_catalog_error(error: String, needle: &str) {
    assert_eq!(
        error.contains(needle),
        true,
        "expected validation error containing {needle:?}, got {error:?}"
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

fn set_entry_path(entry: &mut fixture_catalog::FixtureCatalogEntry, path: &str) {
    entry.path = path.to_string();
    entry.matrix_path = format!("{path}#/matrix");
    entry.syndrome_path = format!("{path}#/syndrome");
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

fn validate_catalog_with_extra_fixture(
    catalog: &fixture_catalog::FixtureCatalog,
    source_fixture: &str,
    catalog_path: &str,
) -> Result<Vec<fixture_catalog::ValidatedFixtureCatalogEntry>, String> {
    let fixture_root = fixture_catalog::fixture_root();
    let temp_root = unique_temp_fixture_root();
    copy_fixture_tree(&fixture_root, &temp_root);

    let source_path = temp_root.join(source_fixture);
    let temp_fixture_path = temp_root.join(catalog_path);
    fs::create_dir_all(temp_fixture_path.parent().unwrap()).unwrap();
    fs::copy(&source_path, &temp_fixture_path).unwrap();

    let mut catalog = catalog.clone();
    let entry = catalog
        .fixtures
        .iter_mut()
        .find(|entry| source_fixture.ends_with(&entry.path))
        .unwrap();
    set_entry_path(entry, catalog_path);

    fixture_catalog::validate_catalog(&catalog, &temp_root)
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
