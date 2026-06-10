use rsinter::bench::registry::default_rust_runner_names;

#[test]
fn default_rust_runner_names_include_workspace_decoders() {
    let names = default_rust_runner_names();
    assert!(names.contains(&"rmatching".to_string()));
    assert!(names.contains(&"rbposd".to_string()));
    assert!(names.contains(&"rilpqec".to_string()));
}
