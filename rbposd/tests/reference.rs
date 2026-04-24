use std::fs;
use std::path::PathBuf;

#[path = "../dev/parity_runner.rs"]
mod parity_runner;
#[path = "../dev/parity_schema.rs"]
mod parity_schema;

#[test]
fn checked_in_parity_fixtures_match_exact_expected_outputs() {
    let fixture_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/parity");

    for case in parity_schema::load_cases(&fixture_dir) {
        let report = parity_runner::run_case(&case);
        assert_eq!(
            report.matches_expected,
            Some(true),
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

    assert!(fixture_dir.join("bp_repetition_single_flip.json").exists());
    assert!(fixture_dir.join("osd_small_sparse_code.json").exists());
    assert!(fixture_dir.join("osd_equal_reliability_tiebreak.json").exists());
}

#[test]
fn task_6_documentation_surfaces_exist() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let basic_example = crate_root.join("examples/basic_decode.rs");
    let profile_example = crate_root.join("examples/profile_repetition.rs");
    let lib_rs = crate_root.join("src/lib.rs");

    assert!(basic_example.exists(), "missing {}", basic_example.display());
    assert!(profile_example.exists(), "missing {}", profile_example.display());

    let lib_contents = fs::read_to_string(&lib_rs).unwrap();
    assert!(
        lib_contents.contains("use rbposd::{BpOsdDecoder, ChannelModel, DecoderConfig, ParityCheckMatrix, Syndrome};"),
        "missing crate-level usage example in {}",
        lib_rs.display()
    );
}
