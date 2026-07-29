use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("rstim must live directly below the workspace root")
        .to_path_buf()
}

fn read_repo_file(path: &str) -> String {
    fs::read_to_string(repo_root().join(path))
        .unwrap_or_else(|error| panic!("failed to read {path}: {error}"))
}

#[test]
fn rustqec_is_the_workspace_brand_while_rstim_remains_the_simulator() {
    let readme = read_repo_file("README.md");
    let site_config = read_repo_file("site/config.toml");
    let base_template = read_repo_file("site/templates/base.html");
    let workspace_manifest = read_repo_file("Cargo.toml");
    let simulator_manifest = read_repo_file("rstim/Cargo.toml");

    assert!(readme.starts_with("# RustQEC\n"));
    assert!(readme.contains("RustQEC is a Rust workspace for quantum error correction."));
    assert!(readme.contains("the `rstim` Stim-like circuit simulator and CLI"));
    assert!(site_config.contains("title = \"RustQEC\""));
    assert!(base_template.contains("RustQEC — a quantum error correction workspace in Rust"));

    assert!(workspace_manifest.contains("\"rstim\""));
    assert!(simulator_manifest.contains("name = \"rstim\""));
    assert!(readme.contains("cargo run -p rstim --bin rstim"));
}
