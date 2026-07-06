use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

fn read_repo_file(relative: &str) -> String {
    let path = repo_root().join(relative);
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
}

fn assert_repo_file_exists(relative: &str) {
    let path = repo_root().join(relative);
    assert!(Path::new(&path).is_file(), "missing site resource {}", path.display());
}

#[test]
fn qp101_browser_resources_are_preserved() {
    let index = read_repo_file("site/index.html");
    let app = read_repo_file("site/app.js");

    for marker in [
        "id=\"qp101\"",
        "href=\"#qp101\"",
        "href=\"qp101.schema.json\"",
        "href=\"QP101-ZY.md\"",
        "href=\"examples/basic.qp101.json\"",
        "href=\"examples/repeat-detector.qp101.json\"",
        "href=\"examples/atom-loss-sample.qp101.json\"",
        "id=\"schema-browser\"",
        "id=\"operations\"",
        "id=\"gallery\"",
        "id=\"examples\"",
        "src=\"gallery/basic-site.svg\"",
        "src=\"gallery/repeat-detector-site.svg\"",
        "src=\"gallery/atom-loss-sample.svg\"",
    ] {
        assert!(index.contains(marker), "site index is missing marker {marker}");
    }

    assert!(
        app.contains("fetch(\"qp101.schema.json\")"),
        "schema browser must keep fetching qp101.schema.json"
    );

    for relative in [
        "rstim/doc/qp101.schema.json",
        "rstim/doc/QP101-ZY.md",
        "qp101-viz/examples/basic.qp101.json",
        "qp101-viz/examples/repeat-detector.qp101.json",
        "qp101-viz/examples/atom-loss-sample.qp101.json",
        "qp101-viz/examples/basic.stim",
        "qp101-viz/examples/repeat-detector.stim",
        "qp101-viz/examples/atom-loss-sample.stim",
    ] {
        assert_repo_file_exists(relative);
    }
}

#[test]
fn benchmark_methodology_lists_required_provenance() {
    let index = read_repo_file("site/index.html");
    let app = read_repo_file("site/app.js");
    let manifest_text = read_repo_file("site/benchmark-site.json");
    let manifest: Value = serde_json::from_str(&manifest_text)
        .expect("site benchmark manifest must be valid JSON");

    for marker in [
        "id=\"benchmarks\"",
        "Benchmark Methodology",
        "Claims Policy",
        "smoke",
        "full",
        "extended",
        "reference reproduction",
        "Publishable Evidence",
        "Local-Only Evidence",
        "smoke checks verify wiring",
        "full evidence can describe the committed checked run",
    ] {
        assert!(
            index.contains(marker),
            "benchmark methodology is missing marker {marker}"
        );
    }

    for field in [
        "OS",
        "CPU",
        "Rust version",
        "Python version",
        "dependency versions",
        "external repository commits",
        "command line",
        "seeds",
        "build profile",
        "shots or error budgets",
        "date",
    ] {
        assert!(
            index.contains(field),
            "benchmark methodology is missing provenance field {field}"
        );
    }

    for marker in [
        "id=\"benchmark-manifest\"",
        "fetch(\"data/benchmark-site.json\")",
        "renderBenchmarkManifest",
        "family.status",
        "family.claims_limit",
        "item.status",
        "item.claims_limit",
    ] {
        let source = if marker.starts_with("id=") { &index } else { &app };
        assert!(
            source.contains(marker),
            "manifest-backed benchmark rendering is missing marker {marker}"
        );
    }

    let families = manifest["families"]
        .as_array()
        .expect("manifest families must be an array");
    assert!(!families.is_empty(), "manifest must list benchmark families");
    for family in families {
        assert!(
            family["status"].as_str().is_some(),
            "family is missing status: {family:?}"
        );
        assert!(
            family["claims_limit"].as_str().is_some(),
            "family is missing claims_limit: {family:?}"
        );
        let items = family["evidence_items"]
            .as_array()
            .expect("family evidence_items must be an array");
        assert!(!items.is_empty(), "family must list evidence items: {family:?}");
        for item in items {
            assert!(
                item["status"].as_str().is_some(),
                "item is missing status: {item:?}"
            );
            assert!(
                item["claims_limit"].as_str().is_some(),
                "item is missing claims_limit: {item:?}"
            );
        }
    }
}
