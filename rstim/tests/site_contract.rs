use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

fn read_site_file(relative: &str) -> String {
    let path = repo_root().join("_site").join(relative);
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
}

fn assert_site_file_exists(relative: &str) {
    let path = repo_root().join("_site").join(relative);
    assert!(Path::new(&path).is_file(), "missing built site file {}", path.display());
}

#[test]
fn qp101_browser_resources_are_preserved() {
    let index = read_site_file("index.html");
    let app = read_site_file("app.js");

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
        assert!(index.contains(marker), "built index is missing marker {marker}");
    }

    assert!(
        app.contains("fetch(\"qp101.schema.json\")"),
        "schema browser must keep fetching qp101.schema.json"
    );

    for relative in [
        "qp101.schema.json",
        "QP101-ZY.md",
        "examples/basic.qp101.json",
        "examples/repeat-detector.qp101.json",
        "examples/atom-loss-sample.qp101.json",
        "gallery/basic-site.svg",
        "gallery/repeat-detector-site.svg",
        "gallery/atom-loss-sample.svg",
    ] {
        assert_site_file_exists(relative);
    }
}
