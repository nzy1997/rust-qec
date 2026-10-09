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
    assert!(
        Path::new(&path).is_file(),
        "missing site resource {}",
        path.display()
    );
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

fn assert_exact_checked_artifacts(item: &Value, expected: &[(&str, &str)]) {
    assert_checked_artifacts(item, expected);
    let actual = checked_artifact_paths(item);
    assert_eq!(
        actual.len(),
        expected.len(),
        "evidence item {} must list exactly its assigned checked artifacts",
        item["id"].as_str().unwrap_or("<missing>")
    );
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

fn js_function_body<'a>(
    source: &'a str,
    function_name: &str,
    next_function_marker: &str,
) -> &'a str {
    let signature = format!("function {function_name}(");
    let function_start = source
        .find(&signature)
        .unwrap_or_else(|| panic!("missing function {function_name}"));
    let body_start = source[function_start..]
        .find('{')
        .map(|offset| function_start + offset + 1)
        .unwrap_or_else(|| panic!("function {function_name} is missing an opening brace"));
    let body_end = source[body_start..]
        .find(next_function_marker)
        .map(|offset| {
            let next_function_start = body_start + offset;
            source[body_start..next_function_start]
                .rfind("\n  }\n\n")
                .map(|close_offset| body_start + close_offset)
                .unwrap_or(next_function_start)
        })
        .unwrap_or_else(|| {
            panic!(
                "function {function_name} is missing marker {next_function_marker:?} after its body"
            )
        });
    &source[body_start..body_end]
}

const CANONICAL_PROVENANCE_KEYS: &[&str] = &[
    "schema_version",
    "artifact_date",
    "source_commit",
    "commands",
    "os",
    "cpu_model",
    "rust_version",
    "python_version",
    "dependency_versions",
    "external_repository_commits",
    "seed_policy",
    "build_profile",
    "shots_or_error_budget",
    "artifact_hashes",
];

const EVIDENCE_PAGE_TEMPLATES: &[&str] = &[
    "site/templates/benchmark-simulation.html",
    "site/templates/benchmark-decoders.html",
    "site/templates/css-codes.html",
    "site/templates/validation.html",
];

fn combined_evidence_pages() -> String {
    EVIDENCE_PAGE_TEMPLATES
        .iter()
        .map(|relative| read_repo_file(relative))
        .collect::<Vec<_>>()
        .join("\n")
}

fn evidence_assignment_count(sources: &str, item_id: &str) -> usize {
    sources
        .split("data-evidence-items=\"")
        .skip(1)
        .filter_map(|suffix| suffix.split_once('"').map(|(assigned, _)| assigned))
        .flat_map(str::split_whitespace)
        .filter(|assigned| *assigned == item_id)
        .count()
}

fn checked_artifact_paths(item: &Value) -> Vec<&str> {
    item["artifacts"]
        .as_array()
        .unwrap_or_else(|| panic!("evidence item artifacts must be an array"))
        .iter()
        .filter(|artifact| artifact["checked"].as_bool().unwrap_or(false))
        .map(|artifact| {
            artifact["path"]
                .as_str()
                .unwrap_or_else(|| panic!("checked artifact must carry a path: {artifact:?}"))
        })
        .collect()
}

fn assert_canonical_provenance(item_id: &str, item: &Value) {
    let provenance = item["provenance"]
        .as_object()
        .unwrap_or_else(|| panic!("{item_id} must carry canonical provenance"));

    for key in CANONICAL_PROVENANCE_KEYS {
        assert!(
            provenance.contains_key(*key),
            "{item_id} provenance is missing key {key}"
        );
    }
    assert_eq!(
        provenance["schema_version"].as_i64(),
        Some(1),
        "{item_id} provenance schema_version must be 1"
    );

    for key in CANONICAL_PROVENANCE_KEYS
        .iter()
        .copied()
        .filter(|key| *key != "schema_version")
    {
        let entry = provenance[key]
            .as_object()
            .unwrap_or_else(|| panic!("{item_id} provenance.{key} must be an object"));
        let status = entry
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or_else(|| panic!("{item_id} provenance.{key} must carry status"));
        assert!(
            matches!(status, "recorded" | "not_recorded"),
            "{item_id} provenance.{key} has unsupported status {status}"
        );
        if status == "not_recorded" {
            assert!(
                entry
                    .get("reason")
                    .and_then(Value::as_str)
                    .is_some_and(|reason| !reason.trim().is_empty()),
                "{item_id} provenance.{key} not_recorded entries must carry a reason"
            );
        }
    }

    let artifact_hashes = provenance["artifact_hashes"]
        .as_object()
        .unwrap_or_else(|| panic!("{item_id} provenance.artifact_hashes must be an object"));
    assert_eq!(
        artifact_hashes.get("status").and_then(Value::as_str),
        Some("recorded"),
        "{item_id} provenance.artifact_hashes must be recorded"
    );
    let hash_values = artifact_hashes
        .get("value")
        .and_then(Value::as_object)
        .unwrap_or_else(|| panic!("{item_id} provenance.artifact_hashes.value must be an object"));

    for path in checked_artifact_paths(item) {
        let hash_entry = hash_values.get(path).unwrap_or_else(|| {
            panic!("{item_id} provenance.artifact_hashes is missing checked artifact {path}")
        });
        assert!(
            hash_entry
                .get("sha256")
                .and_then(Value::as_str)
                .is_some_and(|digest| !digest.trim().is_empty()),
            "{item_id} checked artifact {path} must carry provenance.artifact_hashes sha256"
        );
    }
}

#[test]
fn checked_result_provenance_styles_wrap_long_values() {
    let styles = read_repo_file("site/static/styles.css");

    assert_contains_all(
        &styles,
        &[
            ".provenance-hash",
            ".provenance-hash-list code",
            ".provenance-value-list code",
            "overflow-wrap: anywhere",
        ],
        "checked result provenance wrapping styles",
    );
}

#[test]
fn readme_links_benchmarked_site() {
    let readme = read_repo_file("README.md");
    let showcase_index = read_repo_file("docs/showcases/README.md");

    for (context, text) in [
        ("README.md", readme.as_str()),
        ("docs/showcases/README.md", showcase_index.as_str()),
    ] {
        assert_contains_all_case_insensitive(
            text,
            &[
                "benchmarked documentation site",
                "benchmark evidence",
                "qp101",
                "make build-site",
                "python3 tools/check_site_build.py _site",
            ],
            context,
        );
        assert!(
            text.contains("https://nzy1997.github.io/rust-qec/"),
            "{context} must link to the GitHub Pages documentation site"
        );
    }
}

#[test]
fn pages_workflow_builds_benchmarked_site() {
    let workflow = read_repo_file(".github/workflows/deploy-pages.yml");
    let makefile = read_repo_file("Makefile");

    assert_contains_all(
        &workflow,
        &[
            "actions/configure-pages@v5",
            "zola-v0.22.1-x86_64-unknown-linux-gnu.tar.gz",
            "run: make build-site",
            "run: python3 tools/check_site_build.py _site",
            "actions/upload-pages-artifact@v3",
            "python3 tools/site_versions.py build --site-root _site --output _pages",
            "path: _pages",
            "actions/deploy-pages@v4",
        ],
        "Pages deployment workflow",
    );

    for forbidden in [
        "npm install",
        "npm ci",
        "pnpm install",
        "yarn install",
        "vite build",
        "next build",
    ] {
        assert!(
            !workflow.contains(forbidden),
            "Pages workflow must stay focused on make build-site, found {forbidden}"
        );
    }

    assert_contains_all_case_insensitive(
        &makefile,
        &["build-site", "benchmarked documentation site"],
        "Makefile build-site help",
    );
    assert_contains_all(
        &makefile,
        &[
            "python3 tools/prepare_site_docs.py",
            "zola --root site build --output-dir $(CURDIR)/_site",
            "mkdir -p _site/examples _site/data",
            "python3 tools/build_qp101_gallery.py --repo-root . --out-dir _site/gallery",
            "python3 tools/copy_site_benchmark_data.py --repo-root . --site-root _site site/benchmark-site.json",
        ],
        "Makefile build-site target",
    );
    assert!(
        makefile.find("python3 tools/prepare_site_docs.py")
            < makefile.find("zola --root site build --output-dir $(CURDIR)/_site"),
        "canonical docs must be staged inside the Zola root before the site build"
    );
}

#[test]
fn qp101_browser_resources_are_preserved() {
    let qp101 = read_repo_file("site/templates/qp101.html");
    let browser = read_repo_file("site/static/js/qp101-browser.js");

    for marker in [
        "id=\"qp101\"",
        "href=\"../qp101.schema.json\"",
        "href=\"protocol/\"",
        "href=\"../QP101-ZY.md\"",
        "href=\"../examples/basic.qp101.json\"",
        "href=\"../examples/repeat-detector.qp101.json\"",
        "href=\"../examples/atom-loss-sample.qp101.json\"",
        "id=\"schema-browser\"",
        "id=\"operations\"",
        "id=\"gallery\"",
        "id=\"examples\"",
        "src=\"../gallery/basic-site.svg\"",
        "src=\"../gallery/repeat-detector-site.svg\"",
        "src=\"../gallery/atom-loss-sample.svg\"",
    ] {
        assert!(
            qp101.contains(marker),
            "QP101 template is missing marker {marker}"
        );
    }
    assert!(
        browser.contains("fetch(ROOT + \"/qp101.schema.json\")"),
        "schema browser must fetch qp101.schema.json through the relative-root contract"
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
fn task_oriented_content_pages_are_linked() {
    let navigation = read_repo_file("site/templates/partials/docs-navigation.html");
    assert_eq!(navigation.matches("<a href=").count(), 20);
    assert_contains_all(
        &navigation,
        &[
            "Tutorials",
            "CLI &amp; formats",
            "Benchmarks",
            "Support &amp; versions",
            "/rust-api/",
            "/qec-code-cli/",
            "/rsinter-cli/",
            "/circuit-format/",
            "/data-formats/",
            "/benchmarks/simulation/",
            "/benchmarks/decoders/",
            "/versions/",
        ],
        "twenty primary pages in four groups",
    );
    for route in ["simulator", "detector-models", "atom-loss-concepts"] {
        let content = read_repo_file(&format!("site/content/{route}/_index.md"));
        assert!(content.contains("legacy-guide.html"));
        assert!(!navigation.contains(&format!("/{route}/")));
    }
    assert_contains_all(
        &read_repo_file("site/templates/get-started.html"),
        &[
            "rstim circuit dem",
            "id=\"logical-error-rate\"",
            "rstim surface-code-ler",
        ],
        "complete introductory workflow",
    );
    assert_contains_all(
        &read_repo_file("site/templates/css-codes.html"),
        &[
            "num_cols",
            "--hx hx.json --hz hz.json",
            "bound_type: upper",
            "rstim circuit detect",
        ],
        "custom CSS experiment",
    );
    assert_contains_all(
        &read_repo_file("site/templates/decoding.html"),
        &[
            "--decoder rmatching",
            "--decoder rbposd",
            "Different decoder inputs",
        ],
        "frozen-input comparison",
    );
}

#[test]
fn new_documentation_routes_use_canonical_sources() {
    let get_started = read_repo_file("site/templates/get-started.html");
    let installer = read_repo_file("site/static/install.sh");
    let support = read_repo_file("site/templates/support.html");
    let protocol = read_repo_file("site/templates/protocol.html");
    let base = read_repo_file("site/templates/base.html");
    let prepare_docs = read_repo_file("tools/prepare_site_docs.py");
    let installer_version = installer
        .lines()
        .find_map(|line| line.strip_prefix("VERSION="))
        .expect("native installer must pin a release version");
    assert_eq!(installer_version, "v0.3.3");

    assert_contains_all(
        &get_started,
        &[
            "id=\"install\"",
            "cargo install --locked --path rstim --force",
            "id=\"continue-rustqec\"",
            "id=\"further-learning\"",
        ],
        "source-install onboarding entry point",
    );
    assert_contains_all(
        &installer,
        &["archive=rustqec-$VERSION-$target.tar.gz"],
        "pinned historical installer",
    );
    assert_contains_all(
        &support,
        &["generated/support-core.md", "../qp101/protocol/"],
        "rendered support contract page",
    );
    assert_contains_all(
        &protocol,
        &[
            "generated/qp101-protocol.md",
            "../../QP101-ZY.md",
            "download",
        ],
        "rendered QP101 protocol page",
    );
    assert_contains_all(
        &base,
        &[
            "{{ root }}/get-started/",
            "{{ root }}/support/",
            "{{ root }}/validation/",
            "{{ root }}/js/docs.js",
        ],
        "shared documentation navigation",
    );
    assert_contains_all(
        &prepare_docs,
        &[
            "site/generated",
            "rstim/doc/QP101-ZY.md",
            "docs/support-compatibility.md",
            "shutil.copyfile",
        ],
        "canonical documentation staging script",
    );
}

#[test]
fn get_started_uses_one_noisy_circuit_for_definition_and_sampling() {
    let circuit = read_repo_file("site/static/examples/getting-started.stim");
    let page = read_repo_file("site/templates/get-started.html");
    let makefile = read_repo_file("Makefile");

    assert_contains_all(
        &circuit,
        &["X_ERROR(0.1) 0", "X_ERROR(0.1) 1", "CX 0 1"],
        "get-started circuit",
    );
    assert_contains_all(
        &page,
        &[
            "rstim render_svg --in circuit.stim --out circuit.svg",
            "rstim circuit dem --in circuit.stim",
            "--in circuit.stim \\",
            "rstim render_svg --sample_shot --seed 86 --in circuit.stim",
            "id=\"surface-code\"",
            "The four <code>0.02</code> values set error probabilities",
            "<li><code>--after-clifford-depolarization</code>",
            "depolarizing errors after Clifford gates",
            "<li><code>--before-round-data-depolarization</code>",
            "<li><code>--after-reset-flip-probability</code>",
            "<li><code>--before-measure-flip-probability</code>",
            "id=\"surface-sample\"",
            "--in surface.stim --shots 1 --seed 0",
            "010010100000010000000000\t1",
            "id=\"decode\"",
            "A decoder predicts whether <code>L0</code> flipped from the detector events",
            "<code>--decoder rmatching</code> selects the included matching decoder",
            "a remaining component involving three or more detectors (a hyperedge) is rejected",
            "id=\"logical-error-rate\"",
            "rstim circuit decode \\",
            "--decoder rmatching \\",
            "--dem surface.dem --in surface-detectors.01",
            "cat surface-predictions.01</code></pre>\n<pre data-output=\"true\"><code>1</code></pre>",
            "rstim surface-code-ler \\",
            "--distances 3,5,7 --rounds 9,15,21 \\",
            "--physical-error-rates 0.008,0.009,0.01,0.011,0.012 \\",
            "--shots 10000 --seed 86 --out-dir .",
            "logical error rate per round",
            "Distance 7</th>",
            "href=\"../css-codes/\"",
            "href=\"../rust-api/\"",
            "href=\"../decoding/\"",
            "href=\"../qp101/\"",
            "https://github.com/quantumlib/Stim",
            "https://github.com/oscarhiggott/PyMatching",
            "https://errorcorrectionzoo.org/c/rotated_surface",
            "https://arxiv.org/abs/1208.0928",
        ],
        "get-started commands",
    );
    assert!(
        !page.contains("noisy.stim"),
        "get-started page must not introduce a second circuit"
    );
    assert!(
        !page.contains("grep -E '^(MR|DETECTOR)'"),
        "get-started page must not confuse circuit instructions with DEM output"
    );
    assert!(
        !page.contains("<code>surface.stim</code> is the circuit"),
        "get-started page must not repeat circuit annotation details"
    );
    for removed_text in [
        "A seeded example run",
        "href=\"../examples/surface-code-ler.csv\"",
        "Use rmatching from Rust",
        "Sample more circuits",
        "Prepare training data",
        "Explore circuits visually",
        "What is included?",
        "Add optional CLI features",
        "Build from source</h2>",
    ] {
        assert!(
            !page.contains(removed_text),
            "remove extra plot text: {removed_text}"
        );
    }
    assert!(
        page.find("A decoder predicts whether").unwrap()
            < page.find("<code>--decoder rmatching</code>").unwrap(),
        "get-started must define the decoder task before naming rmatching"
    );
    assert!(
        !page.to_lowercase().contains("python"),
        "get-started page should only show the Rust workflow"
    );
    assert!(
        !page.contains("cargo run")
            && !page
                .split_once("id=\"first-circuit\"")
                .unwrap()
                .1
                .contains("rsinter")
            && !page.contains("repository root"),
        "get-started commands should use the installed rstim binary"
    );
    assert!(
        makefile.contains("target/debug/rstim surface-code-ler --distances 3,5,7 --rounds 9,15,21 --physical-error-rates 0.008,0.009,0.01,0.011,0.012 --shots 10000 --seed 86"),
        "site build should generate the plotted data with rstim"
    );
    assert!(
        makefile.contains("--sample_shot --seed 86 --in site/static/examples/getting-started.stim"),
        "sampled diagram must use the same circuit as the definition"
    );
}

#[test]
fn homepage_features_atom_loss_and_routes_installation_to_get_started() {
    let index = read_repo_file("site/templates/index.html");
    let get_started = read_repo_file("site/templates/get-started.html");
    let readme = read_repo_file("README.md");
    let styles = read_repo_file("site/static/layout.css");

    assert_contains_all(
        &index,
        &[
            "id=\"atom-loss\"",
            "Install with Cargo",
            "href=\"get-started/#install\"",
            "From atom-loss circuits to loss-aware decoding",
            "href=\"atom-loss/\"",
            "href=\"get-started/#first-circuit\"",
            "Envelope decoding · Beta",
        ],
        "homepage atom-loss feature and installation routes",
    );
    assert_contains_all(
        &get_started,
        &[
            "cargo install --locked --path rstim --force",
            "id=\"first-circuit\"",
            "id=\"continue-rustqec\"",
            "https://www.rust-lang.org/tools/install",
            "rstim render_svg --in circuit.stim --out circuit.svg",
            "rstim circuit detect \\",
            "rstim render_svg --sample_shot --seed 86",
        ],
        "linked unified installation instructions",
    );
    assert!(
        !index.contains("Download v0.3.0"),
        "homepage hero must not promote a release download button"
    );
    let hero_actions = index
        .split_once("class=\"actions\"")
        .expect("homepage must provide primary actions")
        .1
        .split_once("</div>")
        .expect("homepage actions must have a closing container")
        .0;
    assert_contains_all(
        hero_actions,
        &["class=\"button primary\" href=\"get-started/#install\""],
        "homepage primary Cargo action",
    );
    assert!(
        !hero_actions.contains("get-started/#first-circuit"),
        "homepage primary actions must not skip installation prerequisites"
    );
    assert!(
        !index.contains("only need the command line tools"),
        "native installer copy must not undermine Cargo as the default path"
    );
    assert_contains_all(
        &readme,
        &[
            "cargo install --locked --path rstim --force",
            "rstim circuit stats",
            "rstim capabilities --format json",
        ],
        "README installation section",
    );
    assert_contains_all(
        &styles,
        &[
            ".home-atom-loss",
            ".loss-workflow",
            ".loss-workflow-actions",
        ],
        "homepage atom-loss feature styles",
    );
}

#[test]
fn learning_paths_are_prose_and_homepage_describes_rust_and_cli() {
    let index = read_repo_file("site/templates/index.html");
    let get_started = read_repo_file("site/templates/get-started.html");

    assert_contains_all(
        &index,
        &["native Rust APIs and a unified CLI", "href=\"validation/\""],
        "homepage Rust API and CLI value proposition",
    );

    let after_plot = get_started
        .split_once("id=\"continue-rustqec\"")
        .expect("tutorial must link to further documentation")
        .1;
    assert_contains_all(
        after_plot,
        &[
            "href=\"../css-codes/\"",
            "href=\"../rust-api/\"",
            "href=\"../decoding/\"",
            "href=\"../qp101/\"",
            "inspired by <a href=\"https://github.com/quantumlib/Stim\">Stim</a>",
            "https://github.com/oscarhiggott/PyMatching",
            "https://errorcorrectionzoo.org/c/rotated_surface",
            "https://arxiv.org/abs/1208.0928",
        ],
        "tutorial continuation and further reading",
    );
    assert!(
        !after_plot.contains("<ul") && !after_plot.contains("<li"),
        "tutorial continuation should connect its links in prose",
    );
    assert!(
        !after_plot.contains("getting_started.ipynb")
            && !after_plot.contains("follow that example"),
        "further learning should describe the projects' relationship instead of attributing sweep parameters",
    );
}

#[test]
fn sampling_data_page_preserves_training_and_loss_contracts() {
    let page = read_repo_file("site/templates/sampling-data.html");
    let base = read_repo_file("site/templates/base.html");
    let index = read_repo_file("site/templates/index.html");
    let styles = read_repo_file("site/static/styles.css");

    assert_contains_all(
        &page,
        &[
            "id=\"choose-path\"",
            "id=\"sample\"",
            "id=\"export\"",
            "id=\"b8-format\"",
            "id=\"loss-tensors\"",
            "id=\"marker-contract\"",
            "id=\"load-and-check\"",
            "circuit sample",
            "dataset export",
            "mkdir -p data",
            "--mode measurements_blinded",
            "--logical-x-qubits 1,8,15",
            "--error-trace",
            "shots.b8",
            "answers.b8",
            "masks.b8",
            "trace.jsonl",
            "lsb_first",
            "measurement_loss_mask",
            "detector_valid",
            "TICK[rstim:logical_flip_point]",
            "before every positive-probability noise instruction",
            "load_blinded_training_data.py",
            "this command requires a dataset exported with",
        ],
        "sampling and training-data page",
    );
    assert_contains_all(
        &base,
        &[
            "href=\"{{ root }}/sampling-data/\"",
            ">Sampling &amp; training data</a>",
        ],
        "sampling-data navigation",
    );
    assert_contains_all(
        &index,
        &["href=\"sampling-data/\"", "Sampling &amp; training data"],
        "sampling-data homepage entry",
    );
    assert_contains_all(
        &styles,
        &[
            ".data-pipeline",
            ".data-choice-grid",
            ".bundle-grid",
            ".data-note",
        ],
        "sampling-data styles",
    );
    assert_repo_file_exists("site/content/sampling-data/_index.md");
}

#[test]
fn decode_campaigns_navigation_and_validation_evidence_are_unified() {
    let index = read_repo_file("site/templates/index.html");
    let base = read_repo_file("site/templates/base.html");
    let decoding = format!(
        "{}\n{}",
        read_repo_file("site/templates/decoding.html"),
        read_repo_file("site/templates/benchmark-decoders.html")
    );
    let validation = read_repo_file("site/templates/validation.html");
    let styles = read_repo_file("site/static/styles.css");

    for removed in [
        "Explore the workspace",
        "RSMP v1 showcase",
        "href=\"benchmark-campaigns/\"",
    ] {
        assert!(
            !index.contains(removed),
            "home page still contains removed UI marker {removed}"
        );
    }
    assert!(
        !base.contains(">Bench</a>"),
        "top navigation still contains the removed Bench item"
    );
    assert!(
        !repo_root()
            .join("site/templates/benchmark-campaigns.html")
            .exists(),
        "obsolete benchmark-campaigns template still exists"
    );
    assert!(
        !repo_root()
            .join("site/content/benchmark-campaigns/_index.md")
            .exists(),
        "obsolete benchmark-campaigns content route still exists"
    );

    assert_contains_all(
        &decoding,
        &[
            "id=\"decoder-families\"",
            "id=\"benchmark-campaigns\"",
            "decoder-evidence",
            "surface-decoder-full",
            "bb-circuit-full",
        ],
        "merged Decode page",
    );
    for moved_item in [
        "surface-decoder-local-smoke",
        "bb-circuit-local-readiness",
        "rbposd-parity-gate",
    ] {
        assert!(
            !decoding.contains(moved_item),
            "Decode page still embeds moved evidence item {moved_item}"
        );
    }
    assert_contains_all(
        &validation,
        &[
            "id=\"reading-results\"",
            "id=\"historical-results\"",
            "id=\"local-workflows\"",
            "rstim-vs-stim-full",
            "rstim-vs-stim-release",
            "rstim-perf-ci",
            "rbposd-parity-gate",
            "surface-decoder-local-smoke",
            "bb-circuit-local-readiness",
            "qec-code-random-window-local-pipeline",
        ],
        "central performance and validation page",
    );
    assert_contains_all(
        &base,
        &[
            "section.extra.nav",
            "nav-link",
            "current",
            "aria-current=\"page\"",
        ],
        "active navigation template",
    );
    assert_contains_all(
        &styles,
        &[
            ".nav-link.current",
            ".decoder-evidence .result-card.has-plot",
        ],
        "active navigation and full-width decoder evidence styles",
    );

    for relative in [
        "site/content/_index.md",
        "site/content/simulator/_index.md",
        "site/content/detector-models/_index.md",
        "site/content/decoding/_index.md",
        "site/content/css-codes/_index.md",
        "site/content/rsmp-v1-showcase/_index.md",
        "site/content/qp101/_index.md",
        "site/content/get-started/_index.md",
        "site/content/support/_index.md",
        "site/content/qp101/protocol/_index.md",
        "site/content/validation/_index.md",
    ] {
        assert!(
            read_repo_file(relative).contains("nav ="),
            "{relative} is missing its active navigation key"
        );
    }
}

#[test]
fn benchmark_methodology_lists_required_provenance() {
    let evidence_pages = combined_evidence_pages();
    let app = read_repo_file("site/static/js/benchmarks.js");
    let manifest_text = read_repo_file("site/benchmark-site.json");
    let manifest: Value =
        serde_json::from_str(&manifest_text).expect("site benchmark manifest must be valid JSON");

    for marker in [
        "data-evidence-items=",
        "Smoke runs check wiring",
        "checked full artifacts support",
        "not a general performance claim",
        "local-only",
    ] {
        assert!(
            evidence_pages.contains(marker),
            "distributed benchmark methodology is missing marker {marker}"
        );
    }

    for marker in [
        "fetch(ROOT + \"/data/benchmark-site.json\")",
        "evidenceContainers",
        "renderEvidenceContainers",
        "family.status",
        "family.claims_limit",
        "item.status",
        "item.claims_limit",
        "repoSourceHref",
        "https://github.com/nzy1997/rust-qec/blob/master/",
        "repoSourceHref(path)",
    ] {
        assert!(
            app.contains(marker),
            "manifest-backed benchmark rendering is missing marker {marker}"
        );
    }

    let families = manifest["families"]
        .as_array()
        .expect("manifest families must be an array");
    assert!(
        !families.is_empty(),
        "manifest must list benchmark families"
    );
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
        assert!(
            !items.is_empty(),
            "family must list evidence items: {family:?}"
        );
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
    let evidence_pages = combined_evidence_pages();
    let app = read_repo_file("site/static/js/benchmarks.js");
    let manifest_text = read_repo_file("site/benchmark-site.json");
    let manifest: Value =
        serde_json::from_str(&manifest_text).expect("site benchmark manifest must be valid JSON");

    const CHECKED_ITEM_IDS: &[&str] = &[
        "surface-decoder-full",
        "bb-circuit-full",
        "rstim-vs-stim-correctness",
        "rstim-vs-stim-full",
        "rstim-vs-stim-release",
        "rstim-vs-stim-release-repetition-sample",
        "rstim-vs-stim-release-surface-detect",
        "rstim-vs-stim-release-dem-sample",
    ];
    for item_id in CHECKED_ITEM_IDS {
        assert_eq!(
            evidence_assignment_count(&evidence_pages, item_id),
            1,
            "checked evidence item ID {item_id} must be assigned to exactly one content page"
        );
    }

    assert_contains_all(
        &app,
        &[
            "evidenceContainers",
            "renderEvidenceContainers",
            "container.dataset.evidenceItems",
            "findEvidenceItem",
            "item.artifacts",
            "artifact.checked",
            "artifact.kind === \"image\"",
            "item.commands",
            "item.caveats",
            "renderArtifactLinks",
            "renderCommandList",
            "renderTextList",
            "renderProvenance",
            "renderProvenance(item.provenance)",
            "item.provenance",
            "recorded",
            "not_recorded",
            "artifact_hashes",
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
            !evidence_pages.contains(hardcoded_path),
            "checked artifact path {hardcoded_path} must come from the manifest, not a content template"
        );
        assert!(
            !app.contains(hardcoded_path),
            "checked artifact path {hardcoded_path} must come from the manifest, not benchmarks.js"
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
    assert_item_has_text_list_marker(
        surface_item,
        "commands",
        "make surface-decoder-compare-full",
    );
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

    let (rstim_vs_stim_family, rstim_vs_stim_correctness_item) =
        find_evidence_item(&manifest, "rstim-vs-stim-correctness");
    assert_eq!(rstim_vs_stim_family["status"].as_str(), Some("partial"));
    assert_eq!(
        rstim_vs_stim_correctness_item["status"].as_str(),
        Some("existing")
    );
    assert_eq!(
        rstim_vs_stim_correctness_item["tier"].as_str(),
        Some("full")
    );
    assert_exact_checked_artifacts(
        rstim_vs_stim_correctness_item,
        &[
            (
                "benchmarks/rstim_vs_stim_simulator/results/full/correctness-summary.json",
                "correctness-summary",
            ),
            (
                "benchmarks/rstim_vs_stim_simulator/results/distributions/summary.json",
                "correctness-summary",
            ),
            (
                "benchmarks/rstim_vs_stim_simulator/results/distributions/expanded-correctness.json",
                "correctness-summary",
            ),
            (
                "benchmarks/rstim_vs_stim_simulator/results/distributions/report.md",
                "correctness-report",
            ),
            (
                "benchmarks/rstim_vs_stim_simulator/cases.full.toml",
                "fixture-manifest",
            ),
            (
                "benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim",
                "stim-fixture",
            ),
            ("docs/showcases/rstim-vs-stim-simulator.md", "showcase"),
        ],
    );
    assert_item_has_text_list_marker(
        rstim_vs_stim_correctness_item,
        "commands",
        "python3 tools/check_rstim_vs_stim_expanded_correctness.py",
    );
    assert_item_has_text_list_marker(rstim_vs_stim_correctness_item, "caveats", "eight");
    assert!(
        rstim_vs_stim_correctness_item["claims_limit"]
            .as_str()
            .is_some_and(|value| value.contains("eight") && value.contains("d11/r100")),
        "rstim-vs-stim correctness item must keep its bounded manifest claims limit"
    );

    let (_, rstim_vs_stim_item) = find_evidence_item(&manifest, "rstim-vs-stim-full");
    assert_eq!(rstim_vs_stim_item["status"].as_str(), Some("existing"));
    assert_eq!(rstim_vs_stim_item["tier"].as_str(), Some("full"));
    assert_exact_checked_artifacts(
        rstim_vs_stim_item,
        &[
            (
                "benchmarks/rstim_vs_stim_simulator/results/full/speed-summary.json",
                "speed-summary",
            ),
            (
                "benchmarks/rstim_vs_stim_simulator/results/full/speed-report.md",
                "speed-report",
            ),
        ],
    );
    assert_item_has_text_list_marker(rstim_vs_stim_item, "caveats", "#406");
    assert!(
        rstim_vs_stim_item["claims_limit"]
            .as_str()
            .is_some_and(|value| value.contains("#406") && value.contains("debug-profile")),
        "rstim-vs-stim historical item must keep its narrow manifest claims limit"
    );

    let (_, rstim_vs_stim_release_item) = find_evidence_item(&manifest, "rstim-vs-stim-release");
    assert_eq!(
        rstim_vs_stim_release_item["status"].as_str(),
        Some("existing")
    );
    assert_eq!(rstim_vs_stim_release_item["tier"].as_str(), Some("release"));
    assert_exact_checked_artifacts(
        rstim_vs_stim_release_item,
        &[
            (
                "benchmarks/rstim_vs_stim_simulator/results/release/summary.json",
                "speed-summary",
            ),
            (
                "benchmarks/rstim_vs_stim_simulator/results/release/report.md",
                "speed-report",
            ),
            (
                "benchmarks/rstim_vs_stim_simulator/results/release/environment.json",
                "environment",
            ),
        ],
    );
    assert_item_has_text_list_marker(
        rstim_vs_stim_release_item,
        "commands",
        "python3 -m benchmarks.rstim_vs_stim_simulator.run_speed_case --profile release",
    );
    assert_item_has_text_list_marker(
        rstim_vs_stim_release_item,
        "caveats",
        "historical #406 debug-profile artifact remains separate",
    );
    assert!(
        rstim_vs_stim_release_item["claims_limit"]
            .as_str()
            .is_some_and(|value| {
                value.contains("one recorded d11/r100 selected-case workload")
                    && value.contains("one recorded environment")
                    && value.contains("not broad rstim/Stim parity")
            }),
        "rstim-vs-stim release item must keep its narrow manifest claims limit"
    );

    let (_, repetition_item) =
        find_evidence_item(&manifest, "rstim-vs-stim-release-repetition-sample");
    assert_eq!(repetition_item["status"].as_str(), Some("existing"));
    assert_eq!(repetition_item["tier"].as_str(), Some("release"));
    assert_exact_checked_artifacts(
        repetition_item,
        &[
            (
                "benchmarks/rstim_vs_stim_simulator/results/release-repetition-sample/summary.json",
                "speed-summary",
            ),
            (
                "benchmarks/rstim_vs_stim_simulator/results/release-repetition-sample/report.md",
                "speed-report",
            ),
            (
                "benchmarks/rstim_vs_stim_simulator/results/release-repetition-sample/environment.json",
                "environment",
            ),
        ],
    );

    let (_, surface_detect_item) =
        find_evidence_item(&manifest, "rstim-vs-stim-release-surface-detect");
    assert_eq!(surface_detect_item["status"].as_str(), Some("existing"));
    assert_eq!(surface_detect_item["tier"].as_str(), Some("release"));
    assert_exact_checked_artifacts(
        surface_detect_item,
        &[
            (
                "benchmarks/rstim_vs_stim_simulator/results/release-surface-detect/summary.json",
                "speed-summary",
            ),
            (
                "benchmarks/rstim_vs_stim_simulator/results/release-surface-detect/report.md",
                "speed-report",
            ),
            (
                "benchmarks/rstim_vs_stim_simulator/results/release-surface-detect/environment.json",
                "environment",
            ),
        ],
    );

    let (_, dem_sample_item) = find_evidence_item(&manifest, "rstim-vs-stim-release-dem-sample");
    assert_eq!(dem_sample_item["status"].as_str(), Some("existing"));
    assert_eq!(dem_sample_item["tier"].as_str(), Some("release"));
    assert_exact_checked_artifacts(
        dem_sample_item,
        &[
            (
                "benchmarks/rstim_vs_stim_simulator/results/release-dem-sample/raw.jsonl",
                "speed-raw",
            ),
            (
                "benchmarks/rstim_vs_stim_simulator/results/release-dem-sample/summary.json",
                "speed-summary",
            ),
            (
                "benchmarks/rstim_vs_stim_simulator/results/release-dem-sample/report.md",
                "speed-report",
            ),
            (
                "benchmarks/rstim_vs_stim_simulator/results/release-dem-sample/environment.json",
                "environment",
            ),
        ],
    );

    for (item_id, item) in [
        ("surface-decoder-full", surface_item),
        ("bb-circuit-full", bb_item),
        ("rstim-vs-stim-correctness", rstim_vs_stim_correctness_item),
        ("rstim-vs-stim-full", rstim_vs_stim_item),
        ("rstim-vs-stim-release", rstim_vs_stim_release_item),
        ("rstim-vs-stim-release-repetition-sample", repetition_item),
        ("rstim-vs-stim-release-surface-detect", surface_detect_item),
        ("rstim-vs-stim-release-dem-sample", dem_sample_item),
    ] {
        assert_canonical_provenance(item_id, item);
    }
}

#[test]
fn checked_benchmark_provenance_is_manifest_backed() {
    let app = read_repo_file("site/static/js/benchmarks.js");
    let evidence_pages = combined_evidence_pages();
    let manifest_text = read_repo_file("site/benchmark-site.json");
    let manifest: Value =
        serde_json::from_str(&manifest_text).expect("site benchmark manifest must be valid JSON");

    let checked_renderer = js_function_body(
        &app,
        "renderEvidenceContainers",
        "\n  if (benchmarkManifest || evidenceContainers.length)",
    );

    assert_contains_all(
        checked_renderer,
        &[
            "container.innerHTML",
            "container.dataset.evidenceItems",
            "findEvidenceItem",
            "renderProvenance(item.provenance)",
        ],
        "checked benchmark provenance renderer",
    );

    for hardcoded in [
        "schema_version",
        "artifact_hashes",
        "source_commit",
        "cpu_model",
        "benchmarks/surface_decoder_compare/results/full/results.csv",
        "benchmarks/surface_decoder_compare/results/full/surface_decoder_compare.png",
        "benchmarks/bb_circuit_bposd_compare/results/full/results.csv",
        "benchmarks/bb_circuit_bposd_compare/results/full/summary.md",
        "benchmarks/bb_circuit_bposd_compare/results/full/bb_circuit_bposd_compare.png",
        "benchmarks/bb_circuit_bposd_compare/results/full/reference_gap_report.md",
    ] {
        assert!(
            !evidence_pages.contains(hardcoded),
            "checked provenance value {hardcoded} must come from the manifest renderer, not a content template"
        );
    }

    for item_id in ["surface-decoder-full", "bb-circuit-full"] {
        let (_, item) = find_evidence_item(&manifest, item_id);
        assert_canonical_provenance(item_id, item);
    }
}

#[test]
fn qec_code_and_future_benchmarks_are_classified() {
    let simulator = read_repo_file("site/templates/benchmark-simulation.html");
    let css_codes = read_repo_file("site/templates/css-codes.html");
    let validation = read_repo_file("site/templates/validation.html");
    let manifest_text = read_repo_file("site/benchmark-site.json");
    let manifest: Value =
        serde_json::from_str(&manifest_text).expect("site benchmark manifest must be valid JSON");

    assert_contains_all(
        &format!("{css_codes}\n{validation}"),
        &[
            "id=\"css-construction\"",
            "id=\"distance-search\"",
            "local-only",
            "qec-code-random-window-local-pipeline",
        ],
        "qec-code benchmark content section",
    );
    let detector_models = read_repo_file("site/templates/benchmark-simulation.html");
    let simulator_evidence = format!("{simulator}\n{detector_models}\n{validation}");
    assert_contains_all(
        &simulator_evidence,
        &[
            "rstim-vs-stim-correctness",
            "rstim-vs-stim-release",
            "rstim-vs-stim-release-surface-detect",
            "rstim-vs-stim-release-dem-sample",
            "named fixtures",
        ],
        "rstim-versus-Stim simulator evidence section",
    );

    let families = manifest["families"]
        .as_array()
        .expect("manifest families must be an array");
    let qec_family = families
        .iter()
        .find(|family| family["id"] == "qec-code-random-window")
        .expect("qec-code random-window family must exist");
    let qec_status = qec_family["status"]
        .as_str()
        .expect("qec-code family status must be a string");
    assert!(
        matches!(qec_status, "local-only" | "partial"),
        "qec-code family must be local-only or partial, got {qec_status}"
    );
    let qec_items = qec_family["evidence_items"]
        .as_array()
        .expect("qec-code family evidence_items must be an array");
    assert!(
        !qec_items.is_empty(),
        "qec-code family must list evidence items"
    );
    assert!(
        !css_codes
            .contains("QEC-code random-window upper-bound evidence, no-target smoke profiles"),
        "qec-code random-window site copy must not describe evidence without local-only or partial status"
    );
    for item in qec_items {
        let item_id = item["id"].as_str().unwrap_or("<missing>");
        let status = item["status"]
            .as_str()
            .unwrap_or_else(|| panic!("qec-code item {item_id} missing status"));
        assert!(
            matches!(status, "local-only" | "partial"),
            "qec-code item {item_id} must be local-only or partial, got {status}"
        );
        item["artifacts"]
            .as_array()
            .unwrap_or_else(|| panic!("qec-code item {item_id} artifacts must be an array"));
    }

    let rstim_vs_stim_family = families
        .iter()
        .find(|family| family["id"] == "rstim-vs-stim-simulator")
        .expect("rstim-vs-stim simulator family must exist");
    assert_eq!(
        rstim_vs_stim_family["status"], "partial",
        "rstim versus Stim simulator family must be partial checked evidence"
    );
    let rstim_vs_stim_items = rstim_vs_stim_family["evidence_items"]
        .as_array()
        .expect("rstim-vs-stim simulator evidence_items must be an array");
    assert!(
        !rstim_vs_stim_items.is_empty(),
        "rstim-vs-stim simulator family must list evidence items"
    );
    let rstim_vs_stim_item = rstim_vs_stim_items
        .iter()
        .find(|item| item["id"] == "rstim-vs-stim-correctness")
        .expect("rstim-vs-stim correctness item must exist");
    assert_eq!(
        rstim_vs_stim_item["status"], "existing",
        "rstim-vs-stim correctness item must stay existing inside the partial family"
    );
    assert_exact_checked_artifacts(
        rstim_vs_stim_item,
        &[
            (
                "benchmarks/rstim_vs_stim_simulator/results/full/correctness-summary.json",
                "correctness-summary",
            ),
            (
                "benchmarks/rstim_vs_stim_simulator/results/distributions/summary.json",
                "correctness-summary",
            ),
            (
                "benchmarks/rstim_vs_stim_simulator/results/distributions/expanded-correctness.json",
                "correctness-summary",
            ),
            (
                "benchmarks/rstim_vs_stim_simulator/results/distributions/report.md",
                "correctness-report",
            ),
            (
                "benchmarks/rstim_vs_stim_simulator/cases.full.toml",
                "fixture-manifest",
            ),
            (
                "benchmarks/rstim_vs_stim_simulator/fixtures/stim_surface_code_rotated_memory_z_d11_r100.stim",
                "stim-fixture",
            ),
            ("docs/showcases/rstim-vs-stim-simulator.md", "showcase"),
        ],
    );
    for item in rstim_vs_stim_items {
        let item_id = item["id"].as_str().unwrap_or("<missing>");
        assert!(
            item["artifacts"]
                .as_array()
                .is_some_and(|artifacts| !artifacts.is_empty()),
            "rstim-vs-stim item {item_id} must list checked artifacts"
        );
    }

    for family in families {
        let Some(items) = family["evidence_items"].as_array() else {
            continue;
        };
        for item in items {
            let Some(artifacts) = item["artifacts"].as_array() else {
                continue;
            };
            for artifact in artifacts {
                if artifact["checked"].as_bool().unwrap_or(false) {
                    let path = artifact["path"].as_str().unwrap_or("");
                    assert!(
                        !path.starts_with("benchmarks/out/"),
                        "checked artifact must not point under benchmarks/out/: {path}"
                    );
                }
            }
        }
    }
}
