use rsinter::bench::registry::{build_default_rust_runner_registry, default_rust_runner_names};

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
fn default_rust_runner_names_include_workspace_decoders() {
    let names = default_rust_runner_names();
    assert!(names.contains(&"rmatching".to_string()));
    assert!(names.contains(&"rbposd".to_string()));
    assert!(names.contains(&"rilpqec".to_string()));
}
