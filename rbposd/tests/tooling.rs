use std::path::PathBuf;

#[test]
fn parity_harness_tooling_surfaces_exist() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workflow_path = crate_root
        .parent()
        .unwrap()
        .join(".github/workflows/rbposd-parity.yml");
    let parity_driver = crate_root.join("examples/parity_driver.rs");
    let parity_harness = crate_root.join("scripts/parity_harness.py");
    let parity_requirements = crate_root.join("scripts/requirements-parity.txt");
    let parity_requirements_display = parity_requirements.display().to_string();

    assert!(parity_driver.exists(), "missing {}", parity_driver.display());
    assert!(parity_harness.exists(), "missing {}", parity_harness.display());
    assert!(
        parity_requirements.exists(),
        "missing {}",
        parity_requirements_display
    );
    assert!(workflow_path.exists(), "missing {}", workflow_path.display());
}
