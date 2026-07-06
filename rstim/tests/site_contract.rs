use std::fs;
use std::path::{Path, PathBuf};

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

fn assert_contains_all(haystack: &str, markers: &[&str], context: &str) {
    for marker in markers {
        assert!(
            haystack.contains(marker),
            "{context} is missing marker {marker}"
        );
    }
}

fn assert_contains_all_case_insensitive(haystack: &str, markers: &[&str], context: &str) {
    let lower = haystack.to_lowercase();
    for marker in markers {
        assert!(
            lower.contains(&marker.to_lowercase()),
            "{context} is missing marker {marker}"
        );
    }
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
fn workspace_feature_walkthroughs_are_linked() {
    let index = read_repo_file("site/index.html");

    assert_contains_all(
        &index,
        &[
            "id=\"workspace-overview\"",
            "id=\"feature-walkthroughs\"",
            "id=\"benchmark-evidence\"",
            "rstim",
            "rsinter",
            "rmatching",
            "rbposd",
            "rilpqec",
            "qec-code",
            "qec-ilp-core",
            "docs/showcases/rstim-cli-dem-pipeline.md",
            "docs/showcases/rstim-render-svg-atom-loss.md",
            "docs/showcases/qec-code-css-construction.md",
            "docs/showcases/benchmark-evidence.md",
            "docs/showcases/qec-code-random-window-benchmark.md",
            "docs/showcases/README.md",
            "rstim/doc/cli.md",
            "rstim stats",
            "rstim sample",
            "rstim sample_dem",
            "rstim detect",
            "rstim analyze_errors",
            "rstim render_svg",
            "rstim export_json",
            "rsinter bench",
            "code css",
            "random-window-upper-bound",
        ],
        "workspace walkthrough site source",
    );

    assert_contains_all_case_insensitive(
        &index,
        &[
            "circuit parsing",
            "sampling",
            "detection",
            "dem extraction",
            "svg/qp101 export",
            "decoder experiments",
            "css construction",
            "distance-search workflows",
        ],
        "workspace walkthrough copy",
    );
}
