use std::fs;
use std::path::Path;

use qec_code::family_contract::{parse_css_construction_json, CssFamilySpec, RequestedFamilyId};
use qec_code::QecError;

const CONTRACT: &str = include_str!("../doc/hyperbolic_5_5_contract.md");
const PERTURBED_HGP_CONTRACT: &str = include_str!("../doc/perturbed_hgp_contract.md");

fn assert_contains(haystack: &str, needle: &str) {
    assert!(
        haystack.contains(needle),
        "missing contract marker: {needle}"
    );
}

fn assert_src_tree_does_not_define_callable_hyperbolic_5_5() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let forbidden = [
        "fn hyperbolic_5_5",
        "pub fn hyperbolic_5_5",
        "hyperbolic_5_5_css_checks",
        "construct_hyperbolic_5_5",
        "Hyperbolic55Spec",
    ];
    for path in rust_sources(&src) {
        let text = fs::read_to_string(&path).expect("Rust source should be readable");
        for marker in forbidden {
            assert!(
                !text.contains(marker),
                "{} must not define callable hyperbolic_5_5 runtime surface via {marker}",
                path.display()
            );
        }
    }
}

fn assert_src_tree_does_not_define_callable_perturbed_hgp() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let forbidden = [
        "fn perturbed_hgp",
        "pub fn perturbed_hgp",
        "perturbed_hgp_css_checks",
        "construct_perturbed_hgp",
        "PerturbedHgpSpec",
    ];
    for path in rust_sources(&src) {
        let text = fs::read_to_string(&path).expect("Rust source should be readable");
        for marker in forbidden {
            assert!(
                !text.contains(marker),
                "{} must not define callable perturbed_hgp runtime surface via {marker}",
                path.display()
            );
        }
    }
}

fn rust_sources(root: &Path) -> Vec<std::path::PathBuf> {
    let mut paths = Vec::new();
    collect_rust_sources(root, &mut paths);
    paths.sort();
    paths
}

fn collect_rust_sources(root: &Path, paths: &mut Vec<std::path::PathBuf>) {
    for entry in fs::read_dir(root).expect("source directory should be readable") {
        let entry = entry.expect("source directory entry should be readable");
        let path = entry.path();
        if path.is_dir() {
            collect_rust_sources(&path, paths);
        } else if path.extension().and_then(|extension| extension.to_str()) == Some("rs") {
            paths.push(path);
        }
    }
}

#[test]
fn hyperbolic_5_5_contract_is_complete_and_deferred() {
    assert_contains(CONTRACT, "# Hyperbolic {5,5} Quotient Contract");
    assert_contains(CONTRACT, "contract_version: 1");
    assert_contains(CONTRACT, "schema_version = 1");
    assert_contains(CONTRACT, "construction = \"hyperbolic_5_5_quotient\"");
    assert_contains(CONTRACT, "## Input Contract");
    assert_contains(CONTRACT, "## Quotient Input Choices");
    assert_contains(CONTRACT, "permutation quotient");
    assert_contains(CONTRACT, "subgroup");
    assert_contains(CONTRACT, "## Coxeter Presentation");
    assert_contains(CONTRACT, "r0^2 = r1^2 = r2^2 = 1");
    assert_contains(CONTRACT, "(r0 r1)^5 = 1");
    assert_contains(CONTRACT, "(r1 r2)^5 = 1");
    assert_contains(CONTRACT, "(r0 r2)^2 = 1");
    assert_contains(CONTRACT, "## Flag-Orbit Enumeration");
    assert_contains(CONTRACT, "vertices = orbits of <r1, r2>");
    assert_contains(CONTRACT, "edges = orbits of <r0, r2>");
    assert_contains(CONTRACT, "faces = orbits of <r0, r1>");
    assert_contains(CONTRACT, "## Canonical Ordering");
    assert_contains(CONTRACT, "independent of hash-map iteration");
    assert_contains(CONTRACT, "## Boundary Maps");
    assert_contains(CONTRACT, "H_X = boundary_1");
    assert_contains(CONTRACT, "H_Z = transpose(boundary_2)");
    assert_contains(CONTRACT, "## Validation");
    assert_contains(CONTRACT, "quotient transitivity");
    assert_contains(CONTRACT, "manifold incidence");
    assert_contains(CONTRACT, "orientability");
    assert_contains(CONTRACT, "torsion");
    assert_contains(CONTRACT, "boundary * boundary = 0");
    assert_contains(CONTRACT, "## Typed Failure Modes");
    assert_contains(CONTRACT, "InvalidCoxeterQuotient");
    assert_contains(CONTRACT, "failed_relation");
    assert_contains(CONTRACT, "## Pure-Rust Algorithms");
    assert_contains(CONTRACT, "union-find");
    assert_contains(CONTRACT, "Todd-Coxeter");
    assert_contains(CONTRACT, "## Resource Limits");
    assert_contains(CONTRACT, "max_flags = 200000");
    assert_contains(CONTRACT, "5 seconds");
    assert_contains(CONTRACT, "512 MiB");
    assert_contains(CONTRACT, "## Fixture: Small Stellated Dodecahedron");
    assert_contains(CONTRACT, "V = 12");
    assert_contains(CONTRACT, "E = 30");
    assert_contains(CONTRACT, "F = 12");
    assert_contains(CONTRACT, "code = [[30,8,3]]");
    assert_contains(CONTRACT, "m_x = 12");
    assert_contains(CONTRACT, "m_z = 12");
    assert_contains(CONTRACT, "rank_x = 11");
    assert_contains(CONTRACT, "rank_z = 11");
    assert_contains(CONTRACT, "x_check_weight = 5");
    assert_contains(CONTRACT, "z_check_weight = 5");
    assert_contains(CONTRACT, "## Negative Quotient Fixture");
    assert_contains(CONTRACT, "violates `(r0 r1)^5 = 1`");
    assert_contains(CONTRACT, "failed_relation = \"(r0 r1)^5 = 1\"");
    assert_contains(CONTRACT, "## Split Decision");
    assert_contains(CONTRACT, "one implementation issue");
    assert_contains(CONTRACT, "quotient enumeration");
    assert_contains(CONTRACT, "cellulation");
    assert_contains(CONTRACT, "## Deferred Runtime Status");
    assert_contains(CONTRACT, "No callable runtime stub");

    assert!(
        !CssFamilySpec::callable_requested_family_ids().contains(&RequestedFamilyId::Hyperbolic55),
        "hyperbolic_5_5 must remain absent from callable family IDs"
    );
    assert_eq!(
        parse_css_construction_json(
            r#"{"schema_version":1,"construction":"hyperbolic_5_5_quotient","num_flags":4,"r0":[1,0,3,2],"r1":[2,3,0,1],"r2":[2,3,0,1]}"#
        ),
        Err(QecError::UnknownCssConstruction {
            construction: "hyperbolic_5_5_quotient".to_owned(),
        }),
        "hyperbolic_5_5 quotient inputs must remain non-callable until implementation"
    );
    assert_src_tree_does_not_define_callable_hyperbolic_5_5();
}

#[test]
fn perturbed_hgp_contract_is_grounded_or_explicitly_unsupported() {
    assert_contains(
        PERTURBED_HGP_CONTRACT,
        "# Perturbed HGP Source-Grounding Decision Record",
    );
    assert_contains(PERTURBED_HGP_CONTRACT, "contract_version: 1");
    assert_contains(PERTURBED_HGP_CONTRACT, "family_id = \"perturbed_hgp\"");
    assert_contains(
        PERTURBED_HGP_CONTRACT,
        "selection_status = \"explicitly_unsupported\"",
    );
    assert_contains(
        PERTURBED_HGP_CONTRACT,
        "disposition_decision = \"remain_deferred_unsupported\"",
    );
    assert_contains(PERTURBED_HGP_CONTRACT, "## Searched Terminology");
    assert_contains(PERTURBED_HGP_CONTRACT, "\"perturbed HGP\"");
    assert_contains(PERTURBED_HGP_CONTRACT, "\"perturbed_hgp\"");
    assert_contains(PERTURBED_HGP_CONTRACT, "\"perturbed hypergraph product\"");
    assert_contains(PERTURBED_HGP_CONTRACT, "\"cross swap\"");
    assert_contains(PERTURBED_HGP_CONTRACT, "## Source Log");
    assert_contains(PERTURBED_HGP_CONTRACT, "arXiv:0903.0566");
    assert_contains(PERTURBED_HGP_CONTRACT, "arXiv:2511.04634");
    assert_contains(PERTURBED_HGP_CONTRACT, "arXiv:2501.09622");
    assert_contains(PERTURBED_HGP_CONTRACT, "arXiv:2409.02193");
    assert_contains(PERTURBED_HGP_CONTRACT, "arXiv:2601.08824");
    assert_contains(PERTURBED_HGP_CONTRACT, "Error Correction Zoo");
    assert_contains(PERTURBED_HGP_CONTRACT, "GitHub code search");
    assert_contains(PERTURBED_HGP_CONTRACT, "## Candidate Definitions And Dispositions");
    assert_contains(PERTURBED_HGP_CONTRACT, "Standard hypergraph product");
    assert_contains(PERTURBED_HGP_CONTRACT, "Okada-Kasai cross-swap repair");
    assert_contains(PERTURBED_HGP_CONTRACT, "HGP optimization by random walks");
    assert_contains(PERTURBED_HGP_CONTRACT, "weight-reduced HGP");
    assert_contains(PERTURBED_HGP_CONTRACT, "active-orthogonality APM-LDPC");
    assert_contains(PERTURBED_HGP_CONTRACT, "Rejected");
    assert_contains(PERTURBED_HGP_CONTRACT, "## Disposition Decision");
    assert_contains(PERTURBED_HGP_CONTRACT, "No construction is selected");
    assert_contains(PERTURBED_HGP_CONTRACT, "## No Selected Construction");
    assert_contains(PERTURBED_HGP_CONTRACT, "selected_primary_source = none");
    assert_contains(PERTURBED_HGP_CONTRACT, "perturbation_rule = none");
    assert_contains(PERTURBED_HGP_CONTRACT, "positive_fixture = none");
    assert_contains(PERTURBED_HGP_CONTRACT, "negative_fixture = none");
    assert_contains(PERTURBED_HGP_CONTRACT, "orthogonality_preservation_rule = none");
    assert_contains(PERTURBED_HGP_CONTRACT, "pure_rust_input_schema = none");
    assert_contains(PERTURBED_HGP_CONTRACT, "## Would-Be Selected Contract Requirements");
    assert_contains(PERTURBED_HGP_CONTRACT, "versioned pure-Rust input schema");
    assert_contains(PERTURBED_HGP_CONTRACT, "orthogonality-preservation rule");
    assert_contains(PERTURBED_HGP_CONTRACT, "one exact positive fixture");
    assert_contains(PERTURBED_HGP_CONTRACT, "one deliberately nonorthogonal negative fixture");
    assert_contains(PERTURBED_HGP_CONTRACT, "resource limits");
    assert_contains(PERTURBED_HGP_CONTRACT, "## Provenance And License Compatibility");
    assert_contains(PERTURBED_HGP_CONTRACT, "Apache-2.0");
    assert_contains(PERTURBED_HGP_CONTRACT, "Creative Commons");
    assert_contains(PERTURBED_HGP_CONTRACT, "## Follow-Up Scope");
    assert_contains(PERTURBED_HGP_CONTRACT, "No implementation issue is filed");
    assert_contains(PERTURBED_HGP_CONTRACT, "## Deferred Runtime Status");
    assert_contains(PERTURBED_HGP_CONTRACT, "No callable runtime stub");

    assert!(
        !CssFamilySpec::callable_requested_family_ids().contains(&RequestedFamilyId::PerturbedHgp),
        "perturbed_hgp must remain absent from callable family IDs"
    );
    assert_eq!(
        parse_css_construction_json(
            r#"{"schema_version":1,"construction":"perturbed_hgp","base":{"schema_version":1,"construction":"hypergraph_product","left":{"num_cols":2,"rows":[[0,1]]},"right":{"num_cols":2,"rows":[[0,1]]}},"operations":[]}"#
        ),
        Err(QecError::UnknownCssConstruction {
            construction: "perturbed_hgp".to_owned(),
        }),
        "perturbed_hgp inputs must remain non-callable until maintainers approve a unique construction"
    );
    assert_src_tree_does_not_define_callable_perturbed_hgp();
}
