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

fn find_evidence_item<'a>(manifest: &'a Value, item_id: &str) -> (&'a Value, &'a Value) {
    let families = manifest["families"]
        .as_array()
        .expect("manifest families must be an array");
    for family in families {
        let items = family["evidence_items"]
            .as_array()
            .expect("family evidence_items must be an array");
        for item in items {
            if item["id"].as_str() == Some(item_id) {
                return (family, item);
            }
        }
    }
    panic!("missing evidence item {item_id}");
}

fn assert_checked_artifacts(item: &Value, expected: &[(&str, &str)]) {
    let artifacts = item["artifacts"]
        .as_array()
        .expect("evidence item artifacts must be an array");
    for (path, kind) in expected {
        let artifact = artifacts
            .iter()
            .find(|artifact| artifact["path"].as_str() == Some(*path))
            .unwrap_or_else(|| panic!("missing checked artifact {path}"));
        assert_eq!(
            artifact["kind"].as_str(),
            Some(*kind),
            "artifact {path} must have kind {kind}"
        );
        assert_eq!(
            artifact["checked"].as_bool(),
            Some(true),
            "artifact {path} must be checked"
        );
        assert_repo_file_exists(path);
    }
}

fn assert_item_has_text_list_marker(item: &Value, field: &str, marker: &str) {
    let values = item[field]
        .as_array()
        .unwrap_or_else(|| panic!("evidence item field {field} must be an array"));
    assert!(
        values
            .iter()
            .filter_map(Value::as_str)
            .any(|value| value.contains(marker)),
        "evidence item field {field} is missing marker {marker}"
    );
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

#[test]
fn checked_benchmark_artifacts_are_linked() {
    let index = read_repo_file("site/index.html");
    let app = read_repo_file("site/app.js");
    let manifest_text = read_repo_file("site/benchmark-site.json");
    let manifest: Value = serde_json::from_str(&manifest_text)
        .expect("site benchmark manifest must be valid JSON");

    assert_contains_all(
        &index,
        &[
            "id=\"checked-benchmark-results\"",
            "id=\"checked-benchmark-result-cards\"",
            "data-checked-items=\"surface-decoder-full bb-circuit-full\"",
            "Checked Benchmark Results",
        ],
        "checked benchmark result section",
    );

    assert_contains_all(
        &app,
        &[
            "const checkedBenchmarkItems",
            "renderCheckedBenchmarkResults",
            "findEvidenceItem",
            "item.artifacts",
            "artifact.checked",
            "artifact.kind === \"image\"",
            "item.commands",
            "item.caveats",
            "renderArtifactLinks",
            "renderCommandList",
            "renderTextList",
        ],
        "checked benchmark result renderer",
    );

    for hardcoded_path in [
        "benchmarks/surface_decoder_compare/results/full/results.csv",
        "benchmarks/surface_decoder_compare/results/full/surface_decoder_compare.png",
        "benchmarks/bb_circuit_bposd_compare/results/full/results.csv",
        "benchmarks/bb_circuit_bposd_compare/results/full/bb_circuit_bposd_compare.png",
        "benchmarks/bb_circuit_bposd_compare/results/full/reference_gap_report.md",
    ] {
        assert!(
            !index.contains(hardcoded_path),
            "checked artifact path {hardcoded_path} must come from the manifest, not index.html"
        );
        assert!(
            !app.contains(hardcoded_path),
            "checked artifact path {hardcoded_path} must come from the manifest, not app.js"
        );
    }

    let (surface_family, surface_item) = find_evidence_item(&manifest, "surface-decoder-full");
    assert_eq!(surface_family["status"].as_str(), Some("existing"));
    assert_eq!(surface_item["status"].as_str(), Some("existing"));
    assert_eq!(surface_item["tier"].as_str(), Some("full"));
    assert_checked_artifacts(
        surface_item,
        &[
            (
                "benchmarks/surface_decoder_compare/results/full/results.csv",
                "csv",
            ),
            (
                "benchmarks/surface_decoder_compare/results/full/surface_decoder_compare.png",
                "image",
            ),
        ],
    );
    assert_item_has_text_list_marker(surface_item, "commands", "make surface-decoder-compare-full");
    assert_item_has_text_list_marker(surface_item, "caveats", "committed run");
    assert!(
        surface_item["claims_limit"]
            .as_str()
            .is_some_and(|value| value.contains("committed-run evidence")),
        "surface checked item must keep its manifest claims limit"
    );

    let (bb_family, bb_item) = find_evidence_item(&manifest, "bb-circuit-full");
    assert_eq!(bb_family["status"].as_str(), Some("partial"));
    assert_eq!(bb_item["status"].as_str(), Some("existing"));
    assert_eq!(bb_item["tier"].as_str(), Some("full"));
    assert_checked_artifacts(
        bb_item,
        &[
            (
                "benchmarks/bb_circuit_bposd_compare/results/full/results.csv",
                "csv",
            ),
            (
                "benchmarks/bb_circuit_bposd_compare/results/full/bb_circuit_bposd_compare.png",
                "image",
            ),
            (
                "benchmarks/bb_circuit_bposd_compare/results/full/reference_gap_report.md",
                "report",
            ),
        ],
    );
    assert_item_has_text_list_marker(bb_item, "commands", "make bb-circuit-bposd-compare-full");
    assert_item_has_text_list_marker(
        bb_item,
        "caveats",
        "batched, error-budget-stopped paired comparison rows",
    );
    assert_item_has_text_list_marker(bb_item, "caveats", "not a fixed-shot reproduction");
    assert!(
        bb_item["claims_limit"]
            .as_str()
            .is_some_and(|value| value.contains("reference-gap report only")),
        "BB checked item must keep its manifest claims limit"
    );
}
