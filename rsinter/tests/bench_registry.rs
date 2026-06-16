use std::collections::BTreeMap;

use rsinter::bench::registry::{
    build_default_rust_runner_registry, default_rust_runner_names, expand_runner_points,
    expand_runner_points_for_runner,
};

#[test]
fn default_rust_runner_registry_contains_workspace_decoders() {
    let registry = build_default_rust_runner_registry();
    let names = default_rust_runner_names();
    assert_eq!(registry.len(), names.len());
    assert!(registry.contains_key("rmatching"));
    assert!(registry.contains_key("rbposd"));
    assert!(registry.contains_key("rilpqec"));
}

#[test]
fn default_rust_runner_registry_exposes_runner_names() {
    let registry = build_default_rust_runner_registry();

    assert_eq!(registry.get("rmatching").unwrap().name(), "rmatching");
    assert_eq!(registry.get("rbposd").unwrap().name(), "rbposd");
    assert_eq!(registry.get("rilpqec").unwrap().name(), "rilpqec");
}

#[test]
fn default_rust_runner_names_include_workspace_decoders() {
    let names = default_rust_runner_names();
    assert!(names.contains(&"rmatching".to_string()));
    assert!(names.contains(&"rbposd".to_string()));
    assert!(names.contains(&"rilpqec".to_string()));
}

#[test]
fn expand_runner_points_rejects_empty_sweeps_and_zero_batch_size() {
    let mut params = valid_runner_params();
    params.insert("distance".into(), toml::Value::Array(vec![]));
    assert_eq!(expand_points_err(&params), "distance must not be empty");

    let mut params = valid_runner_params();
    params.insert("rounds".into(), toml::Value::Array(vec![]));
    assert_eq!(expand_points_err(&params), "rounds must not be empty");

    let mut params = valid_runner_params();
    params.insert("p".into(), toml::Value::Array(vec![]));
    assert_eq!(expand_points_err(&params), "p must not be empty");

    let mut params = valid_runner_params();
    params.insert("batch_size".into(), toml::Value::Integer(0));
    assert_eq!(expand_points_err(&params), "batch_size must be positive");
}

#[test]
fn expand_runner_points_rejects_invalid_entries_and_accepts_integer_p() {
    let mut params = valid_runner_params();
    params.insert(
        "rounds".into(),
        toml::Value::Array(vec![toml::Value::Integer(0)]),
    );
    assert_eq!(expand_points_err(&params), "round entry must be >= 1");

    let mut params = valid_runner_params();
    params.insert(
        "p".into(),
        toml::Value::Array(vec![toml::Value::String("bad".into())]),
    );
    assert_eq!(expand_points_err(&params), "p entry must be numeric");

    let mut params = valid_runner_params();
    params.insert(
        "p".into(),
        toml::Value::Array(vec![toml::Value::Integer(1)]),
    );
    let points = expand_runner_points(&params).unwrap();
    assert_eq!(points.len(), 1);
    assert_eq!(points[0].p, 1.0);
}

#[test]
fn expand_runner_points_rejects_non_string_optional_fields() {
    let mut params = valid_runner_params();
    params.insert(
        "input_type".into(),
        toml::Value::Array(vec![toml::Value::String("css".into())]),
    );
    assert_eq!(expand_points_err(&params), "input_type must be a string");

    let mut params = valid_css_runner_params();
    params.insert("schedule".into(), toml::Value::Integer(3));
    assert_eq!(expand_points_err(&params), "schedule must be a string");
}

#[test]
fn expand_runner_points_rejects_non_positive_max_wall_seconds() {
    let mut params = valid_runner_params();
    params.insert("max_wall_seconds".into(), toml::Value::Float(0.0));
    assert_eq!(
        expand_points_err(&params),
        "max_wall_seconds must be positive"
    );

    let mut params = valid_runner_params();
    params.insert("max_wall_seconds".into(), toml::Value::Float(-1.0));
    assert_eq!(
        expand_points_err(&params),
        "max_wall_seconds must be positive"
    );
}

#[test]
#[should_panic(expected = "expected expand_runner_points to fail")]
fn expand_points_err_panics_when_points_expand_successfully() {
    let params = valid_runner_params();
    let _ = expand_points_err(&params);
}

#[test]
fn expand_runner_points_accepts_css_input_type() {
    let params = BTreeMap::from([
        ("input_type".into(), toml::Value::String("css".into())),
        ("code_id".into(), toml::Value::String("steane".into())),
        (
            "hx".into(),
            toml::Value::String("tests/fixtures/css/steane_hx.json".into()),
        ),
        (
            "hz".into(),
            toml::Value::String("tests/fixtures/css/steane_hz.json".into()),
        ),
        ("basis".into(), toml::Value::String("x".into())),
        ("schedule".into(), toml::Value::String("greedy".into())),
        (
            "observables".into(),
            toml::Value::String("tests/fixtures/css/steane_logicals_x.json".into()),
        ),
        (
            "rounds".into(),
            toml::Value::Array(vec![toml::Value::Integer(1)]),
        ),
        (
            "p".into(),
            toml::Value::Array(vec![toml::Value::Float(0.0)]),
        ),
        ("max_shots".into(), toml::Value::Integer(8)),
        ("max_errors".into(), toml::Value::Integer(4)),
        ("batch_size".into(), toml::Value::Integer(4)),
    ]);

    let points = expand_runner_points(&params).unwrap();

    assert_eq!(points.len(), 1);
    assert_eq!(points[0].rounds, 1);
    assert_eq!(points[0].p, 0.0);
    assert_eq!(points[0].input_type, "css");
    assert_eq!(points[0].basis.as_deref(), Some("x"));
    assert_eq!(points[0].code_id.as_deref(), Some("steane"));
}

#[test]
fn expand_runner_points_defaults_optional_css_fields() {
    let params = BTreeMap::from([
        ("input_type".into(), toml::Value::String("css".into())),
        (
            "hx".into(),
            toml::Value::String("tests/fixtures/css/steane_hx.json".into()),
        ),
        (
            "hz".into(),
            toml::Value::String("tests/fixtures/css/steane_hz.json".into()),
        ),
        ("basis".into(), toml::Value::String("x".into())),
        (
            "rounds".into(),
            toml::Value::Array(vec![toml::Value::Integer(1)]),
        ),
        (
            "p".into(),
            toml::Value::Array(vec![toml::Value::Float(0.0)]),
        ),
        ("max_shots".into(), toml::Value::Integer(8)),
        ("max_errors".into(), toml::Value::Integer(4)),
        ("batch_size".into(), toml::Value::Integer(4)),
    ]);

    let points = expand_runner_points(&params).unwrap();

    assert_eq!(points.len(), 1);
    assert_eq!(points[0].code_id, None);
    assert_eq!(points[0].schedule.as_deref(), Some("greedy"));
    assert_eq!(points[0].observables_path, None);
}

#[test]
fn expand_runner_points_defaults_to_legacy_surface_input() {
    let points = expand_runner_points(&valid_runner_params()).unwrap();

    assert_eq!(points[0].input_type, "surface_rotated_memory_x");
    assert_eq!(points[0].distance, Some(3));
    assert_eq!(points[0].basis, None);
}

#[test]
fn expand_runner_points_accepts_rotated_memory_z_input_type() {
    let mut params = valid_runner_params();
    params.insert(
        "input_type".into(),
        toml::Value::String("surface_rotated_memory_z".into()),
    );

    let points = expand_runner_points(&params).unwrap();

    assert_eq!(points.len(), 1);
    assert_eq!(points[0].input_type, "surface_rotated_memory_z");
    assert_eq!(points[0].distance, Some(3));
    assert_eq!(points[0].rounds, 1);
    assert_eq!(points[0].p, 0.002);
    assert_eq!(points[0].basis, None);
}

#[test]
fn expand_runner_points_accepts_optional_max_wall_seconds() {
    let mut params = valid_runner_params();
    params.insert("max_wall_seconds".into(), toml::Value::Float(2.5));

    let points = expand_runner_points(&params).unwrap();

    assert_eq!(points.len(), 1);
    assert_eq!(points[0].max_wall_seconds, Some(2.5));
}

#[test]
fn expand_runner_points_still_requires_max_shots_with_wall_clock_budget() {
    let mut params = valid_runner_params();
    params.remove("max_shots");
    params.insert("max_wall_seconds".into(), toml::Value::Float(2.5));

    assert_eq!(
        expand_points_err(&params),
        "missing runner param: max_shots"
    );
}

#[test]
fn expand_runner_points_for_runner_carries_decoder_params_without_multiplying_points() {
    let mut params = valid_runner_params();
    params.insert("bp_iters".into(), toml::Value::Integer(50));
    params.insert("osd_order".into(), toml::Value::Integer(10));

    let points = expand_runner_points_for_runner("rbposd", &params).unwrap();

    assert_eq!(points.len(), 1);
    assert_eq!(
        points[0]
            .decoder_params
            .get("bp_iters")
            .and_then(toml::Value::as_integer),
        Some(50)
    );
    assert_eq!(
        points[0]
            .decoder_params
            .get("osd_order")
            .and_then(toml::Value::as_integer),
        Some(10)
    );
    assert_eq!(points[0].distance, Some(3));
}

#[test]
fn expand_runner_points_for_runner_rejects_unknown_decoder_param() {
    let mut params = valid_runner_params();
    params.insert("bogus".into(), toml::Value::Integer(1));

    let err = expand_runner_points_for_runner("rbposd", &params).unwrap_err();

    assert_eq!(err, "unknown rbposd runner param: bogus");
}

#[test]
fn expand_runner_points_for_runner_rejects_decoder_params_for_rmatching() {
    let mut params = valid_runner_params();
    params.insert("osd_order".into(), toml::Value::Integer(10));

    let err = expand_runner_points_for_runner("rmatching", &params).unwrap_err();

    assert_eq!(err, "unknown rmatching runner param: osd_order");
}

fn valid_runner_params() -> BTreeMap<String, toml::Value> {
    BTreeMap::from([
        (
            "distance".into(),
            toml::Value::Array(vec![toml::Value::Integer(3)]),
        ),
        (
            "rounds".into(),
            toml::Value::Array(vec![toml::Value::Integer(1)]),
        ),
        (
            "p".into(),
            toml::Value::Array(vec![toml::Value::Float(0.002)]),
        ),
        ("max_shots".into(), toml::Value::Integer(20)),
        ("max_errors".into(), toml::Value::Integer(5)),
        ("batch_size".into(), toml::Value::Integer(4)),
    ])
}

fn valid_css_runner_params() -> BTreeMap<String, toml::Value> {
    BTreeMap::from([
        ("input_type".into(), toml::Value::String("css".into())),
        (
            "hx".into(),
            toml::Value::String("tests/fixtures/css/steane_hx.json".into()),
        ),
        (
            "hz".into(),
            toml::Value::String("tests/fixtures/css/steane_hz.json".into()),
        ),
        ("basis".into(), toml::Value::String("x".into())),
        ("schedule".into(), toml::Value::String("greedy".into())),
        (
            "rounds".into(),
            toml::Value::Array(vec![toml::Value::Integer(1)]),
        ),
        (
            "p".into(),
            toml::Value::Array(vec![toml::Value::Float(0.0)]),
        ),
        ("max_shots".into(), toml::Value::Integer(8)),
        ("max_errors".into(), toml::Value::Integer(4)),
        ("batch_size".into(), toml::Value::Integer(4)),
    ])
}

fn expand_points_err(params: &BTreeMap<String, toml::Value>) -> String {
    match expand_runner_points(params) {
        Ok(_) => panic!("expected expand_runner_points to fail"),
        Err(error) => error,
    }
}
