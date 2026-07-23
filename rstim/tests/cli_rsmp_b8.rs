use rstim::m2d::measurements_to_detections;
use rstim::measurement_transform::MeasurementTransform;
use rstim::output::write_shots_b8;
use rstim::parser::parse_lines;
use rstim::sim::bit_table::BitTable;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

const SEMANTIC_FIXTURES: [&str; 6] = [
    "nonzero_reference",
    "rank_zero",
    "dependent_detectors",
    "repeat_records",
    "observable_recovery",
    "loss_visible_measurements",
];

fn rstim_cmd() -> std::process::Command {
    std::process::Command::new(env!("CARGO_BIN_EXE_rstim"))
}

fn run_cli(args: &[String], stdin: Option<&[u8]>) -> std::process::Output {
    use std::io::Write;
    use std::process::Stdio;
    let mut command = rstim_cmd();
    command.args(args).stdin(if stdin.is_some() {
        Stdio::piped()
    } else {
        Stdio::null()
    });
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command.spawn().expect("spawn rstim");
    if let Some(bytes) = stdin {
        child
            .stdin
            .take()
            .expect("stdin")
            .write_all(bytes)
            .expect("write stdin");
    }
    child.wait_with_output().expect("wait rstim")
}

#[test]
fn rsmp_b8_cli_contract() {
    assert_shared_catalog_roles();
    let dir = tempfile::tempdir().expect("tempdir");
    let valid_cases = verify_positive_cases(dir.path());
    assert_eq!(valid_cases, 7);
    let negative_cases = verify_negative_cases(dir.path());
    assert_eq!(negative_cases, 10);
    println!("PASS rsmp b8 cli valid_cases=7 negative_cases=10");
}

fn verify_positive_cases(root: &Path) -> usize {
    let mut cases = 0;
    for (index, id) in SEMANTIC_FIXTURES.into_iter().enumerate() {
        let mode = match id {
            "nonzero_reference" => OutputMode::MeasurementsOnly,
            "rank_zero" => OutputMode::DetectorsOnly,
            "dependent_detectors" => OutputMode::ObservablesOnly,
            "repeat_records" => OutputMode::All,
            "observable_recovery" => OutputMode::All,
            "loss_visible_measurements" => OutputMode::Pipeline,
            _ => unreachable!("fixture selection is fixed"),
        };
        verify_round_trip(root, id, index, mode);
        cases += 1;
    }

    verify_zero_measurement_round_trip(root, cases);
    cases + 1
}

#[derive(Clone, Copy)]
enum OutputMode {
    MeasurementsOnly,
    DetectorsOnly,
    ObservablesOnly,
    All,
    Pipeline,
}

fn verify_round_trip(root: &Path, id: &str, seed: usize, mode: OutputMode) {
    let circuit_path = fixture_path(&format!("{id}.stim"));
    let circuit_text = fs::read_to_string(&circuit_path).expect("read fixture circuit");
    let instrs = parse_lines(&circuit_text).expect("parse fixture circuit");
    let transform = MeasurementTransform::from_circuit(&instrs).expect("build transform");
    let measurements = patterned_measurements(transform.num_measurements(), 4, seed);
    let measurement_bytes = b8_bytes(&measurements);
    let expected = measurements_to_detections(&instrs, &measurements).expect("m2d expected");
    let expected_detections = b8_bytes(&expected.detections);
    let expected_observables = b8_bytes(&expected.observable_flips);
    let archive = root.join(format!("{id}.rsmp"));

    let pack = run_cli(
        &pack_args(&circuit_path, 4, "-", &archive),
        Some(&measurement_bytes),
    );
    assert_success(&pack, &format!("{id}: pack_samples"));

    match mode {
        OutputMode::MeasurementsOnly => {
            let measurements_out = root.join(format!("{id}.measurements.b8"));
            let unpack = run_cli(
                &unpack_args(&circuit_path, &archive, Some(&measurements_out), None, None),
                None,
            );
            assert_success(&unpack, &format!("{id}: unpack_samples measurements"));
            assert_eq!(
                fs::read(measurements_out).expect("read measurements"),
                measurement_bytes
            );
        }
        OutputMode::DetectorsOnly => {
            let detections_out = root.join(format!("{id}.detections.b8"));
            let unpack = run_cli(
                &unpack_args(&circuit_path, &archive, None, Some(&detections_out), None),
                None,
            );
            assert_success(&unpack, &format!("{id}: unpack_samples detectors"));
            assert_eq!(
                fs::read(detections_out).expect("read detections"),
                expected_detections
            );
        }
        OutputMode::ObservablesOnly => {
            let observables_out = root.join(format!("{id}.observables.b8"));
            let unpack = run_cli(
                &unpack_args(&circuit_path, &archive, None, None, Some(&observables_out)),
                None,
            );
            assert_success(&unpack, &format!("{id}: unpack_samples observables"));
            assert_eq!(
                fs::read(observables_out).expect("read observables"),
                expected_observables
            );
        }
        OutputMode::All => {
            let measurements_out = root.join(format!("{id}.measurements.b8"));
            let detections_out = root.join(format!("{id}.detections.b8"));
            let observables_out = root.join(format!("{id}.observables.b8"));
            let unpack = run_cli(
                &unpack_args(
                    &circuit_path,
                    &archive,
                    Some(&measurements_out),
                    Some(&detections_out),
                    Some(&observables_out),
                ),
                None,
            );
            assert_success(&unpack, &format!("{id}: unpack_samples all outputs"));
            assert_eq!(
                fs::read(measurements_out).expect("read measurements"),
                measurement_bytes
            );
            assert_eq!(
                fs::read(detections_out).expect("read detections"),
                expected_detections
            );
            assert_eq!(
                fs::read(observables_out).expect("read observables"),
                expected_observables
            );
        }
        OutputMode::Pipeline => {
            let packed = run_cli(
                &pack_args(&circuit_path, 4, "-", Path::new("-")),
                Some(&measurement_bytes),
            );
            assert_success(&packed, &format!("{id}: stdin/stdout pack_samples"));
            let unpack = run_cli(
                &unpack_args(
                    &circuit_path,
                    Path::new("-"),
                    Some(Path::new("-")),
                    None,
                    None,
                ),
                Some(&packed.stdout),
            );
            assert_success(&unpack, &format!("{id}: stdin/stdout unpack_samples"));
            assert_eq!(unpack.stdout, measurement_bytes);
        }
    }
    assert_no_sibling_temps(root);
}

fn verify_zero_measurement_round_trip(root: &Path, index: usize) {
    let circuit = root.join("zero_measurements.stim");
    fs::write(&circuit, "R 0\n").expect("write zero-measurement circuit");
    let archive = root.join("zero_measurements.rsmp");
    let measurements_out = root.join("zero_measurements.measurements.b8");
    let detections_out = root.join("zero_measurements.detections.b8");
    let observables_out = root.join("zero_measurements.observables.b8");
    let pack = run_cli(&pack_args(&circuit, 3, "-", &archive), Some(&[]));
    assert_success(&pack, "M = 0 pack_samples");
    let unpack = run_cli(
        &unpack_args(
            &circuit,
            &archive,
            Some(&measurements_out),
            Some(&detections_out),
            Some(&observables_out),
        ),
        None,
    );
    assert_success(&unpack, "M = 0 unpack_samples");
    assert_eq!(
        fs::read(measurements_out).expect("read zero measurements"),
        Vec::<u8>::new()
    );
    assert_eq!(
        fs::read(detections_out).expect("read zero detections"),
        Vec::<u8>::new()
    );
    assert_eq!(
        fs::read(observables_out).expect("read zero observables"),
        Vec::<u8>::new()
    );
    assert_no_sibling_temps(root);
    assert_eq!(index, 6);
}

fn verify_negative_cases(root: &Path) -> usize {
    let circuit = fixture_path("nonzero_reference.stim");
    let measurements = vec![0x01; 4];
    let sentinel = |name: &str| root.join(name);
    let mut cases = 0;

    for (name, input, format) in [
        ("unsupported_format", measurements.as_slice(), "01"),
        ("short_b8", &measurements[..3], "b8"),
        ("extra_b8", &[1, 1, 1, 1, 1][..], "b8"),
        ("padding_b8", &[0x81, 1, 1, 1][..], "b8"),
    ] {
        let archive = sentinel(&format!("{name}.rsmp"));
        write_sentinel(&archive, cases as u8);
        let mut args = pack_args(&circuit, 4, "-", &archive);
        let format_index = args
            .iter()
            .position(|arg| arg == "--in_format")
            .expect("format arg")
            + 1;
        args[format_index] = format.to_owned();
        let entries = directory_entries(root);
        let output = run_cli(&args, Some(input));
        assert_failure(&output, name);
        assert_sentinel(&archive, cases as u8);
        assert_no_new_siblings(root, &entries);
        cases += 1;
    }

    let archive = sentinel("over_limit.rsmp");
    write_sentinel(&archive, cases as u8);
    let entries = directory_entries(root);
    let output = run_cli(
        &pack_args(
            &circuit,
            rstim::sample_archive::ArchiveLimits::default()
                .transform
                .max_shots_per_block
                + 1,
            "-",
            &archive,
        ),
        Some(&vec![0; 1024]),
    );
    assert_failure(&output, "over-limit shots before stdin consumption");
    assert_sentinel(&archive, cases as u8);
    assert_no_new_siblings(root, &entries);
    cases += 1;

    let archive = root.join("valid.rsmp");
    make_valid_archive(&circuit, &archive, &measurements);
    for (name, args) in [
        ("unpack_unsupported_format", {
            let destination = sentinel("unpack_unsupported_format.b8");
            write_sentinel(&destination, 0xa1);
            let mut args = unpack_args(&circuit, &archive, Some(&destination), None, None);
            let format_index = args
                .iter()
                .position(|arg| arg == "--measurements_out_format")
                .expect("format arg")
                + 1;
            args[format_index] = "01".to_owned();
            args
        }),
        (
            "unpack_missing_output",
            unpack_args(&circuit, &archive, None, None, None),
        ),
        ("unpack_duplicate_outputs", {
            let destination = sentinel("unpack_duplicate_outputs.b8");
            write_sentinel(&destination, 0xa2);
            unpack_args(
                &circuit,
                &archive,
                Some(&destination),
                Some(&destination),
                None,
            )
        }),
    ] {
        let entries = directory_entries(root);
        let output = run_cli(&args, None);
        assert_failure(&output, name);
        if name == "unpack_unsupported_format" {
            assert_sentinel(&sentinel("unpack_unsupported_format.b8"), 0xa1);
        }
        if name == "unpack_duplicate_outputs" {
            assert_sentinel(&sentinel("unpack_duplicate_outputs.b8"), 0xa2);
        }
        assert_no_new_siblings(root, &entries);
        cases += 1;
    }

    let mismatch_out = sentinel("circuit_mismatch.b8");
    write_sentinel(&mismatch_out, cases as u8);
    let mismatch_circuit = root.join("mismatch.stim");
    fs::write(&mismatch_circuit, "R 0\nM 0\nDETECTOR rec[-1]\n").expect("write mismatch circuit");
    let entries = directory_entries(root);
    let output = run_cli(
        &unpack_args(&mismatch_circuit, &archive, Some(&mismatch_out), None, None),
        None,
    );
    assert_failure_with_code(&output, "circuit mismatch", "RSMP_CIRCUIT_MISMATCH");
    assert_sentinel(&mismatch_out, cases as u8);
    assert_no_new_siblings(root, &entries);
    cases += 1;

    let corrupt_out = sentinel("corrupt_archive.b8");
    write_sentinel(&corrupt_out, cases as u8);
    let corrupt_archive = root.join("corrupt.rsmp");
    let mut corrupt = fs::read(&archive).expect("read valid archive");
    corrupt[0] ^= 0xff;
    fs::write(&corrupt_archive, corrupt).expect("write corrupt archive");
    let entries = directory_entries(root);
    let output = run_cli(
        &unpack_args(&circuit, &corrupt_archive, Some(&corrupt_out), None, None),
        None,
    );
    assert_failure_with_code(&output, "corrupt archive", "RSMP_BAD_MAGIC");
    assert_sentinel(&corrupt_out, cases as u8);
    assert_no_new_siblings(root, &entries);
    cases += 1;

    cases
}

fn make_valid_archive(circuit: &Path, archive: &Path, measurements: &[u8]) {
    let output = run_cli(&pack_args(circuit, 4, "-", archive), Some(measurements));
    assert_success(&output, "prepare valid archive");
}

fn pack_args(circuit: &Path, shots: u64, input: &str, output: &Path) -> Vec<String> {
    vec![
        "pack_samples".to_owned(),
        "--circuit".to_owned(),
        circuit.display().to_string(),
        "--shots".to_owned(),
        shots.to_string(),
        "--in".to_owned(),
        input.to_owned(),
        "--in_format".to_owned(),
        "b8".to_owned(),
        "--out".to_owned(),
        output.display().to_string(),
    ]
}

fn unpack_args(
    circuit: &Path,
    input: &Path,
    measurements: Option<&Path>,
    detections: Option<&Path>,
    observables: Option<&Path>,
) -> Vec<String> {
    let mut args = vec![
        "unpack_samples".to_owned(),
        "--circuit".to_owned(),
        circuit.display().to_string(),
        "--in".to_owned(),
        input.display().to_string(),
    ];
    append_output_args(
        &mut args,
        "--measurements_out",
        "--measurements_out_format",
        measurements,
    );
    append_output_args(
        &mut args,
        "--detectors_out",
        "--detectors_out_format",
        detections,
    );
    append_output_args(&mut args, "--obs_out", "--obs_out_format", observables);
    args
}

fn append_output_args(
    args: &mut Vec<String>,
    path_flag: &str,
    format_flag: &str,
    path: Option<&Path>,
) {
    if let Some(path) = path {
        args.push(path_flag.to_owned());
        args.push(path.display().to_string());
        args.push(format_flag.to_owned());
        args.push("b8".to_owned());
    }
}

fn patterned_measurements(bits: usize, shots: usize, seed: usize) -> BitTable {
    let mut table = BitTable::try_new(bits, shots).expect("allocate measurements");
    for shot in 0..shots {
        for bit in 0..bits {
            if (bit + shot + seed) % 3 == 1 {
                table.set(bit, shot, true);
            }
        }
    }
    table
}

fn b8_bytes(table: &BitTable) -> Vec<u8> {
    let mut bytes = Vec::new();
    write_shots_b8(table, &mut bytes).expect("serialize b8");
    bytes
}

fn fixture_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .join("rstim/tests/fixtures/rsmp")
        .join(name)
}

fn assert_shared_catalog_roles() {
    let catalog = fs::read_to_string(fixture_path("catalog.json")).expect("read fixture catalog");
    for id in SEMANTIC_FIXTURES {
        assert!(
            catalog.contains(&format!("\"id\": \"{id}\"")),
            "catalog is missing {id}"
        );
    }
}

fn write_sentinel(path: &Path, tag: u8) {
    fs::write(path, [tag, tag.wrapping_add(1), tag.wrapping_add(2)]).expect("write sentinel");
}

fn assert_sentinel(path: &Path, tag: u8) {
    assert_eq!(
        fs::read(path).expect("read sentinel"),
        [tag, tag.wrapping_add(1), tag.wrapping_add(2)]
    );
}

fn directory_entries(root: &Path) -> BTreeSet<std::ffi::OsString> {
    fs::read_dir(root)
        .expect("read tempdir")
        .map(|entry| entry.expect("directory entry").file_name())
        .collect()
}

fn assert_no_sibling_temps(root: &Path) {
    let leaked: Vec<_> = directory_entries(root)
        .into_iter()
        .filter(|name| name.to_string_lossy().contains(".tmp"))
        .collect();
    assert!(
        leaked.is_empty(),
        "leaked sibling temporary files: {leaked:?}"
    );
}

fn assert_no_new_siblings(root: &Path, before: &BTreeSet<std::ffi::OsString>) {
    assert_eq!(
        directory_entries(root),
        *before,
        "leaked sibling temporary file"
    );
}

fn assert_success(output: &std::process::Output, context: &str) {
    assert!(
        output.status.success(),
        "{context} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn assert_failure(output: &std::process::Output, context: &str) {
    assert!(!output.status.success(), "{context} unexpectedly succeeded");
}

fn assert_failure_with_code(output: &std::process::Output, context: &str, code: &str) {
    assert_failure(output, context);
    assert!(
        String::from_utf8_lossy(&output.stderr).contains(code),
        "{context} stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
