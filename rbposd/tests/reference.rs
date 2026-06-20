use std::fs;
use std::path::PathBuf;

#[path = "../dev/parity_runner.rs"]
mod parity_runner;
#[path = "../dev/parity_schema.rs"]
mod parity_schema;

#[test]
fn checked_in_parity_fixtures_match_exact_expected_outputs() {
    let fixture_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/parity");

    // These checked-in fixtures lock the current Rust decoder contract. Cross-runtime
    // drift against Python ldpc is tracked separately by the parity harness.
    for case in parity_schema::load_cases(&fixture_dir) {
        let report = parity_runner::run_case(&case);
        assert_eq!(
            report.expected.as_ref(),
            Some(&report.actual),
            "case={} expected={:?} actual={:?}",
            report.name,
            report.expected,
            report.actual
        );
    }
}

#[test]
fn parity_fixture_directory_contains_the_seed_contract_cases() {
    let fixture_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/parity");
    let mut files: Vec<String> = fs::read_dir(&fixture_dir)
        .unwrap()
        .map(|entry| {
            entry
                .unwrap()
                .path()
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .to_string()
        })
        .collect();
    files.sort();

    assert_eq!(
        files,
        vec![
            "bp_product_sum_serial_sensitive.json",
            "bp_repetition_single_flip.json",
            "osd_equal_reliability_tiebreak.json",
            "osd_small_sparse_code.json",
        ]
    );
}

#[test]
fn task_6_documentation_surfaces_exist() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let basic_example = crate_root.join("examples/basic_decode.rs");
    let profile_example = crate_root.join("examples/profile_repetition.rs");
    let lib_rs = crate_root.join("src/lib.rs");

    assert!(
        basic_example.exists(),
        "missing {}",
        basic_example.display()
    );
    assert!(
        profile_example.exists(),
        "missing {}",
        profile_example.display()
    );

    let lib_contents = fs::read_to_string(&lib_rs).unwrap();
    let lib_rs_display = lib_rs.display().to_string();
    assert!(
        lib_contents.contains(
            "use rbposd::{BpOsdDecoder, ChannelModel, DecoderConfig, ParityCheckMatrix, Syndrome};"
        ),
        "missing crate-level usage example in {}",
        lib_rs_display
    );
    assert!(
        lib_contents.contains(
            "use rbposd::{BpLsdDecoder, ChannelModel, LsdConfig, ParityCheckMatrix, Syndrome};"
        ),
        "missing BpLsdDecoder crate-level usage example in {}",
        lib_rs_display
    );

    let reference_doc = crate_root.join("doc/ldpc_mvp_reference.md");
    let reference_contents = fs::read_to_string(&reference_doc).unwrap();
    let reference_doc_display = reference_doc.display().to_string();
    for required in [
        "BpLsdDecoder",
        "LsdConfig",
        "LsdMethod",
        "UnsupportedLsdOrder",
        "NoLsdSolution",
        "lsd_order=1",
        "lsd_small_sparse_code.json",
        "#98",
        "Shared LSD and BP-Option Fixture Catalog",
        "rbposd/tests/fixtures/catalog.json",
        "bp_product_sum_serial_sensitive.json",
        "python3 rbposd/scripts/parity_harness.py --include-lsd",
        "python3 -m pytest rbposd/scripts/test_parity_harness.py -k lsd",
    ] {
        assert!(
            reference_contents.contains(required),
            "missing {required} in {}",
            reference_doc_display
        );
    }
}
