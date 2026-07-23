use rstim::m2d::measurements_to_detections;
use rstim::measurement_transform::MeasurementTransform;
use rstim::output::write_shots_b8;
use rstim::parser::parse_lines;
use rstim::sim::bit_table::BitTable;
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

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

fn run_cli_with_open_stdin(args: &[String], timeout: Duration) -> std::process::Output {
    use std::process::Stdio;

    let mut command = rstim_cmd();
    command
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().expect("spawn rstim");
    let deadline = Instant::now() + timeout;

    loop {
        if child.try_wait().expect("poll rstim").is_some() {
            return child.wait_with_output().expect("collect rstim output");
        }
        if Instant::now() >= deadline {
            child.kill().expect("kill stdin-blocked rstim");
            child.wait().expect("reap stdin-blocked rstim");
            panic!("over-limit shots blocked waiting for stdin");
        }
        thread::sleep(Duration::from_millis(10));
    }
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
    let mut cases: usize = 0;
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
    fs::write(&archive, []).expect("prepare archive destination");

    let entries = directory_entries(root);
    let pack = run_cli(
        &pack_args(&circuit_path, 4, "-", &archive),
        Some(&measurement_bytes),
    );
    assert_success(&pack, &format!("{id}: pack_samples"));
    assert_no_new_siblings(root, &entries);

    match mode {
        OutputMode::MeasurementsOnly => {
            let measurements_out = root.join(format!("{id}.measurements.b8"));
            fs::write(&measurements_out, []).expect("prepare measurements destination");
            let entries = directory_entries(root);
            let unpack = run_cli(
                &unpack_args(&circuit_path, &archive, Some(&measurements_out), None, None),
                None,
            );
            assert_success(&unpack, &format!("{id}: unpack_samples measurements"));
            assert_eq!(
                fs::read(measurements_out).expect("read measurements"),
                measurement_bytes
            );
            assert_no_new_siblings(root, &entries);
        }
        OutputMode::DetectorsOnly => {
            let detections_out = root.join(format!("{id}.detections.b8"));
            fs::write(&detections_out, []).expect("prepare detections destination");
            let entries = directory_entries(root);
            let unpack = run_cli(
                &unpack_args(&circuit_path, &archive, None, Some(&detections_out), None),
                None,
            );
            assert_success(&unpack, &format!("{id}: unpack_samples detectors"));
            assert_eq!(
                fs::read(detections_out).expect("read detections"),
                expected_detections
            );
            assert_no_new_siblings(root, &entries);
        }
        OutputMode::ObservablesOnly => {
            let observables_out = root.join(format!("{id}.observables.b8"));
            fs::write(&observables_out, []).expect("prepare observables destination");
            let entries = directory_entries(root);
            let unpack = run_cli(
                &unpack_args(&circuit_path, &archive, None, None, Some(&observables_out)),
                None,
            );
            assert_success(&unpack, &format!("{id}: unpack_samples observables"));
            assert_eq!(
                fs::read(observables_out).expect("read observables"),
                expected_observables
            );
            assert_no_new_siblings(root, &entries);
        }
        OutputMode::All => {
            let measurements_out = root.join(format!("{id}.measurements.b8"));
            let detections_out = root.join(format!("{id}.detections.b8"));
            let observables_out = root.join(format!("{id}.observables.b8"));
            for destination in [&measurements_out, &detections_out, &observables_out] {
                fs::write(destination, []).expect("prepare unpack destination");
            }
            let entries = directory_entries(root);
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
            assert_no_new_siblings(root, &entries);
        }
        OutputMode::Pipeline => {
            let entries = directory_entries(root);
            let packed = run_cli(
                &pack_args(&circuit_path, 4, "-", Path::new("-")),
                Some(&measurement_bytes),
            );
            assert_success(&packed, &format!("{id}: stdin/stdout pack_samples"));
            assert_no_new_siblings(root, &entries);
            let entries = directory_entries(root);
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
            assert_no_new_siblings(root, &entries);
        }
    }
    verify_all_semantic_outputs(
        root,
        id,
        &circuit_path,
        &archive,
        &measurement_bytes,
        &expected_detections,
        &expected_observables,
    );
    #[cfg(unix)]
    if id == "repeat_records" {
        verify_unpack_temp_candidate_does_not_clobber_requested_final(
            root,
            &circuit_path,
            &archive,
            &measurement_bytes,
            &expected_detections,
        );
    }
}

fn verify_all_semantic_outputs(
    root: &Path,
    id: &str,
    circuit: &Path,
    archive: &Path,
    measurements: &[u8],
    detections: &[u8],
    observables: &[u8],
) {
    let measurements_out = root.join(format!("{id}.all.measurements.b8"));
    let detections_out = root.join(format!("{id}.all.detections.b8"));
    let observables_out = root.join(format!("{id}.all.observables.b8"));
    for destination in [&measurements_out, &detections_out, &observables_out] {
        fs::write(destination, []).expect("prepare unpack destination");
    }
    let entries = directory_entries(root);
    let unpack = run_cli(
        &unpack_args(
            circuit,
            archive,
            Some(&measurements_out),
            Some(&detections_out),
            Some(&observables_out),
        ),
        None,
    );
    assert_success(
        &unpack,
        &format!("{id}: unpack_samples all semantic outputs"),
    );
    assert_eq!(
        fs::read(measurements_out).expect("read measurements"),
        measurements
    );
    assert_eq!(
        fs::read(detections_out).expect("read detections"),
        detections
    );
    assert_eq!(
        fs::read(observables_out).expect("read observables"),
        observables
    );
    assert_no_new_siblings(root, &entries);
}

fn verify_zero_measurement_round_trip(root: &Path, index: usize) {
    let circuit = root.join("zero_measurements.stim");
    fs::write(&circuit, "R 0\n").expect("write zero-measurement circuit");
    let archive = root.join("zero_measurements.rsmp");
    let measurements_out = root.join("zero_measurements.measurements.b8");
    let detections_out = root.join("zero_measurements.detections.b8");
    let observables_out = root.join("zero_measurements.observables.b8");
    fs::write(&archive, []).expect("prepare archive destination");
    for destination in [&measurements_out, &detections_out, &observables_out] {
        fs::write(destination, []).expect("prepare unpack destination");
    }
    let entries = directory_entries(root);
    let pack = run_cli(&pack_args(&circuit, 3, "-", &archive), Some(&[]));
    assert_success(&pack, "M = 0 pack_samples");
    assert_no_new_siblings(root, &entries);
    let entries = directory_entries(root);
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
    assert_no_new_siblings(root, &entries);

    let stdin_circuit_text = "M 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n";
    let measurement_input = root.join("zero_shot.measurements-in.b8");
    let zero_shot_archive = root.join("zero_shot_circuit_stdin.rsmp");
    let zero_shot_measurements_out = root.join("zero_shot.measurements.b8");
    let zero_shot_detections_out = root.join("zero_shot.detections.b8");
    let zero_shot_observables_out = root.join("zero_shot.observables.b8");
    fs::write(&measurement_input, []).expect("write zero-shot measurement input");
    fs::write(&zero_shot_archive, [0x51]).expect("prepare zero-shot archive destination");
    for destination in [
        &zero_shot_measurements_out,
        &zero_shot_detections_out,
        &zero_shot_observables_out,
    ] {
        fs::write(destination, [0x52]).expect("prepare zero-shot unpack destination");
    }
    let entries = directory_entries(root);
    let pack = run_cli(
        &pack_args(
            Path::new("-"),
            0,
            measurement_input.to_str().expect("measurement input path"),
            &zero_shot_archive,
        ),
        Some(stdin_circuit_text.as_bytes()),
    );
    assert_success(&pack, "zero-shot stdin circuit pack_samples");
    assert_no_new_siblings(root, &entries);
    let entries = directory_entries(root);
    let unpack = run_cli(
        &unpack_args(
            Path::new("-"),
            &zero_shot_archive,
            Some(&zero_shot_measurements_out),
            Some(&zero_shot_detections_out),
            Some(&zero_shot_observables_out),
        ),
        Some(stdin_circuit_text.as_bytes()),
    );
    assert_success(&unpack, "zero-shot stdin circuit unpack_samples");
    assert_eq!(
        fs::read(zero_shot_measurements_out).expect("read zero-shot measurements"),
        Vec::<u8>::new()
    );
    assert_eq!(
        fs::read(zero_shot_detections_out).expect("read zero-shot detections"),
        Vec::<u8>::new()
    );
    assert_eq!(
        fs::read(zero_shot_observables_out).expect("read zero-shot observables"),
        Vec::<u8>::new()
    );
    assert_no_new_siblings(root, &entries);
    assert_eq!(index, 6);
}

fn verify_negative_cases(root: &Path) -> usize {
    let circuit = fixture_path("nonzero_reference.stim");
    let measurements = vec![0x01; 4];
    let sentinel = |name: &str| root.join(name);
    let mut cases: usize = 0;

    for (name, input) in [("short_b8", &measurements[..3])] {
        let archive = sentinel(&format!("{name}.rsmp"));
        write_sentinel(&archive, cases as u8);
        let entries = directory_entries(root);
        let output = run_cli(&pack_args(&circuit, 4, "-", &archive), Some(input));
        assert_failure(&output, name);
        assert_sentinel(&archive, cases as u8);
        assert_no_new_siblings(root, &entries);
    }
    cases += 1;

    for (name, input) in [
        ("extra_b8", &[1, 1, 1, 1, 1][..]),
        ("padding_b8", &[0x81, 1, 1, 1][..]),
    ] {
        let archive = sentinel(&format!("{name}.rsmp"));
        write_sentinel(&archive, cases as u8);
        let entries = directory_entries(root);
        let output = run_cli(&pack_args(&circuit, 4, "-", &archive), Some(input));
        assert_failure(&output, name);
        assert_sentinel(&archive, cases as u8);
        assert_no_new_siblings(root, &entries);
    }
    cases += 1;

    let archive = sentinel("over_limit.rsmp");
    write_sentinel(&archive, cases as u8);
    let entries = directory_entries(root);
    let output = run_cli_with_open_stdin(
        &pack_args(
            &circuit,
            rstim::sample_archive::ArchiveLimits::default().max_total_shots + 1,
            "-",
            &archive,
        ),
        Duration::from_secs(1),
    );
    assert_failure(&output, "over-limit shots before stdin consumption");
    assert_sentinel(&archive, cases as u8);
    assert_no_new_siblings(root, &entries);
    cases += 1;

    let archive = root.join("valid.rsmp");
    make_valid_archive(&circuit, &archive, &measurements);
    verify_non_b8_format_rejections(root, &circuit, &archive, &measurements);
    verify_pack_sweep_rejection_preserves_code_and_destination(root, &measurements);
    cases += 1;

    for (name, args) in [
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
        if name == "unpack_duplicate_outputs" {
            assert_sentinel(&sentinel("unpack_duplicate_outputs.b8"), 0xa2);
        }
        assert_no_new_siblings(root, &entries);
        cases += 1;
    }

    let stream_conflict_out = sentinel("stream_conflict.rsmp");
    write_sentinel(&stream_conflict_out, cases as u8);
    let entries = directory_entries(root);
    let output = run_cli(
        &pack_args(Path::new("-"), 4, "-", &stream_conflict_out),
        Some(&measurements),
    );
    assert_failure(&output, "pack multiple stdin consumers");
    assert_sentinel(&stream_conflict_out, cases as u8);
    assert_no_new_siblings(root, &entries);

    let stream_conflict_out = sentinel("stream_conflict_stdin.b8");
    write_sentinel(&stream_conflict_out, cases as u8);
    let entries = directory_entries(root);
    let output = run_cli(
        &unpack_args(
            Path::new("-"),
            Path::new("-"),
            Some(&stream_conflict_out),
            None,
            None,
        ),
        Some(&fs::read(&archive).expect("read valid archive")),
    );
    assert_failure(&output, "unpack multiple stdin consumers");
    assert_sentinel(&stream_conflict_out, cases as u8);
    assert_no_new_siblings(root, &entries);

    let stream_conflict_out = sentinel("stream_conflict.b8");
    write_sentinel(&stream_conflict_out, cases as u8);
    let entries = directory_entries(root);
    let output = run_cli(
        &unpack_args(
            &circuit,
            &archive,
            Some(Path::new("-")),
            Some(Path::new("-")),
            Some(&stream_conflict_out),
        ),
        None,
    );
    assert_failure(&output, "unpack multiple stdout outputs");
    assert_sentinel(&stream_conflict_out, cases as u8);
    assert_no_new_siblings(root, &entries);
    cases += 1;

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

    let truncated_measurements_out = sentinel("truncated.measurements.b8");
    let truncated_detections_out = sentinel("truncated.detections.b8");
    let truncated_observables_out = sentinel("truncated.observables.b8");
    write_sentinel(&truncated_measurements_out, cases as u8);
    write_sentinel(&truncated_detections_out, cases.wrapping_add(1) as u8);
    write_sentinel(&truncated_observables_out, cases.wrapping_add(2) as u8);
    let truncated_archive = root.join("truncated.rsmp");
    let mut truncated = fs::read(&archive).expect("read valid archive");
    truncated.pop().expect("archive has bytes to truncate");
    fs::write(&truncated_archive, truncated).expect("write truncated archive");
    let entries = directory_entries(root);
    let output = run_cli(
        &unpack_args(
            &circuit,
            &truncated_archive,
            Some(&truncated_measurements_out),
            Some(&truncated_detections_out),
            Some(&truncated_observables_out),
        ),
        None,
    );
    assert_failure(&output, "truncated archive");
    assert_sentinel(&truncated_measurements_out, cases as u8);
    assert_sentinel(&truncated_detections_out, cases.wrapping_add(1) as u8);
    assert_sentinel(&truncated_observables_out, cases.wrapping_add(2) as u8);
    assert_no_new_siblings(root, &entries);
    cases += 1;

    let corrupt_measurements_out = sentinel("corrupt_archive.measurements.b8");
    let corrupt_detections_out = sentinel("corrupt_archive.detections.b8");
    let corrupt_observables_out = sentinel("corrupt_archive.observables.b8");
    write_sentinel(&corrupt_measurements_out, cases as u8);
    write_sentinel(&corrupt_detections_out, cases.wrapping_add(1) as u8);
    write_sentinel(&corrupt_observables_out, cases.wrapping_add(2) as u8);
    let corrupt_archive = root.join("corrupt.rsmp");
    let mut corrupt = fs::read(&archive).expect("read valid archive");
    corrupt.push(0);
    fs::write(&corrupt_archive, corrupt).expect("write corrupt archive");
    let entries = directory_entries(root);
    let output = run_cli(
        &unpack_args(
            &circuit,
            &corrupt_archive,
            Some(&corrupt_measurements_out),
            Some(&corrupt_detections_out),
            Some(&corrupt_observables_out),
        ),
        None,
    );
    assert_failure_with_code(&output, "archive trailing data", "RSMP_TRAILING_DATA");
    assert_sentinel(&corrupt_measurements_out, cases as u8);
    assert_sentinel(&corrupt_detections_out, cases.wrapping_add(1) as u8);
    assert_sentinel(&corrupt_observables_out, cases.wrapping_add(2) as u8);
    assert_no_new_siblings(root, &entries);
    cases += 1;

    cases
}

#[cfg(unix)]
fn verify_unpack_temp_candidate_does_not_clobber_requested_final(
    root: &Path,
    circuit: &Path,
    archive: &Path,
    measurements: &[u8],
    detections: &[u8],
) {
    use std::process::{Command, Stdio};

    let output_name = "temp_collision.detectors.b8";
    let detections_out = root.join(output_name);
    let before = directory_entries(root);
    assert!(
        !before.contains(&OsString::from(output_name)),
        "temp collision detector output unexpectedly pre-exists"
    );

    let child = Command::new("sh")
        .arg("-c")
        .arg(
            "exec \"$1\" unpack_samples \
             --circuit \"$2\" \
             --in \"$3\" \
             --measurements_out \"$4/.$5.rstim-$$-0.tmp\" \
             --detectors_out \"$4/$5\"",
        )
        .arg("rstim-temp-collision")
        .arg(env!("CARGO_BIN_EXE_rstim"))
        .arg(circuit)
        .arg(archive)
        .arg(root)
        .arg(output_name)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn rstim temp collision run");
    let measurement_name = OsString::from(format!(".{output_name}.rstim-{}-0.tmp", child.id()));
    let measurements_out = root.join(&measurement_name);
    let output = child
        .wait_with_output()
        .expect("wait rstim temp collision run");

    assert_success(&output, "unpack temp/final collision");
    assert_eq!(
        fs::read(&measurements_out).expect("read collision measurements"),
        measurements
    );
    assert_eq!(
        fs::read(&detections_out).expect("read collision detections"),
        detections
    );

    let mut expected = before;
    assert!(
        expected.insert(measurement_name),
        "collision measurement output was already reserved"
    );
    assert!(
        expected.insert(OsString::from(output_name)),
        "collision detector output was already reserved"
    );
    assert_eq!(
        directory_entries(root),
        expected,
        "unpack collision run left unexpected sibling entries"
    );
}

fn verify_pack_sweep_rejection_preserves_code_and_destination(root: &Path, measurements: &[u8]) {
    let circuit = root.join("sweep_rejected.stim");
    fs::write(&circuit, "R 0\nCX sweep[0] 0\nM 0\nDETECTOR rec[-1]\n")
        .expect("write sweep circuit");
    let archive = root.join("sweep_rejected.rsmp");
    write_sentinel(&archive, 0xb8);
    let entries = directory_entries(root);
    let output = run_cli(&pack_args(&circuit, 4, "-", &archive), Some(measurements));
    assert_failure_with_code(&output, "pack sweep rejection", "RSMP_UNSUPPORTED_SWEEP");
    assert_sentinel(&archive, 0xb8);
    assert_no_new_siblings(root, &entries);
}

fn make_valid_archive(circuit: &Path, archive: &Path, measurements: &[u8]) {
    let output = run_cli(&pack_args(circuit, 4, "-", archive), Some(measurements));
    assert_success(&output, "prepare valid archive");
}

fn pack_args(circuit: &Path, shots: u64, input: &str, output: &Path) -> Vec<String> {
    let args = vec![
        "pack_samples".to_owned(),
        "--circuit".to_owned(),
        circuit.display().to_string(),
        "--shots".to_owned(),
        shots.to_string(),
        "--in".to_owned(),
        input.to_owned(),
        "--out".to_owned(),
        output.display().to_string(),
    ];
    args
}

fn pack_args_with_format(
    circuit: &Path,
    shots: u64,
    input: &str,
    output: &Path,
    format: &str,
) -> Vec<String> {
    let mut args = pack_args(circuit, shots, input, output);
    args.extend(["--in_format".to_owned(), format.to_owned()]);
    args
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
    append_output_args(&mut args, "--measurements_out", measurements);
    append_output_args(&mut args, "--detectors_out", detections);
    append_output_args(&mut args, "--obs_out", observables);
    args
}

fn unpack_args_with_output_format(
    circuit: &Path,
    input: &Path,
    output_flag: &str,
    output: &Path,
    format: &str,
) -> Vec<String> {
    let mut args = unpack_args(circuit, input, None, None, None);
    args.extend([output_flag.to_owned(), output.display().to_string()]);
    args.extend([format!("{output_flag}_format"), format.to_owned()]);
    args
}

fn verify_non_b8_format_rejections(
    root: &Path,
    circuit: &Path,
    archive: &Path,
    measurements: &[u8],
) {
    let input_archive = root.join("unsupported_in_format.rsmp");
    write_sentinel(&input_archive, 0xa0);
    let entries = directory_entries(root);
    let output = run_cli(
        &pack_args_with_format(circuit, 4, "-", &input_archive, "r8"),
        Some(measurements),
    );
    assert_failure(&output, "unsupported --in_format");
    assert_sentinel(&input_archive, 0xa0);
    assert_no_new_siblings(root, &entries);

    for (name, output_flag, format, tag) in [
        ("measurements", "--measurements_out", "dets", 0xa1),
        ("detectors", "--detectors_out", "unknown", 0xa2),
        ("observables", "--obs_out", "dets", 0xa3),
    ] {
        let destination = root.join(format!("unsupported_{name}_format.out"));
        write_sentinel(&destination, tag);
        let entries = directory_entries(root);
        let output = run_cli(
            &unpack_args_with_output_format(circuit, archive, output_flag, &destination, format),
            None,
        );
        assert_failure(&output, &format!("unsupported {output_flag}_format"));
        assert_sentinel(&destination, tag);
        assert_no_new_siblings(root, &entries);
    }
}

fn append_output_args(args: &mut Vec<String>, path_flag: &str, path: Option<&Path>) {
    if let Some(path) = path {
        args.push(path_flag.to_owned());
        args.push(path.display().to_string());
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
