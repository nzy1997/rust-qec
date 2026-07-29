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

#[test]
fn active_public_links_use_the_rust_qec_slug() {
    let old_github_slug = ["github.com", "nzy1997", "rstim"].join("/");
    let old_pages_path = ["nzy1997.github.io", "rstim"].join("/");
    let old_codecov_slug = ["codecov.io", "gh", "nzy1997", "rstim"].join("/");

    const ACTIVE_FILES: &[&str] = &[
        "README.md",
        "docs/showcases/README.md",
        "site/config.toml",
        "site/templates/base.html",
        "site/templates/rsmp-v1-showcase.html",
        "site/static/js/benchmarks.js",
        "qec-code/Cargo.toml",
        "qec-ilp-core/Cargo.toml",
        "qec-code/README.md",
        "rmatching/README.md",
        "rstim/doc/qp101.schema.json",
        "rstim/tests/site_contract.rs",
        "tools/test_check_site_build.py",
    ];

    for path in ACTIVE_FILES {
        let text = read_repo_file(path);
        assert!(
            !text.contains(&old_github_slug),
            "{path} still contains the old GitHub slug"
        );
        assert!(
            !text.contains(&old_pages_path),
            "{path} still contains the old Pages path"
        );
        assert!(
            !text.contains(&old_codecov_slug),
            "{path} still contains the old Codecov slug"
        );
    }

    assert!(read_repo_file("README.md").contains("github.com/nzy1997/rust-qec"));
    assert!(read_repo_file("site/config.toml")
        .contains("base_url = \"https://nzy1997.github.io/rust-qec\""));
}
