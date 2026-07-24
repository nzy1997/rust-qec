use rstim::sample_archive::corruption_corpus::materialize_named_corruption;
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const PASS_LINE: &str = "PASS rsmp CLI publication pack=1 unpack=1 duplicate_paths=1 normalized_paths=4 rename_failure=1 verify_only=1";
const VERIFY_ONLY_LINE: &str =
    "PASS rsmp version=1.0 shots=4 blocks=2 M=10 D=9 L=1 circuit=18a857fb71f4\n";
const FAIL_RENAME_ENV: &str = "RSTIM_TEST_RSMP_FAIL_RENAME_AT";
const FAIL_PACK_FINISH_ENV: &str = "RSTIM_TEST_RSMP_FAIL_PACK_FINISH";

#[test]
fn rsmp_cli_publication_contract() {
    assert_eq!(verify_pack_publication(), 1);
    assert_eq!(verify_unpack_publication(), 1);
    duplicate_unpack_paths_fail_before_open_impl();
    second_rename_failure_keeps_already_published_output_impl();
    verify_only_rejects_output_options_impl();
    verify_only_matches_unpack_error_code_impl();
    verify_only_success_creates_no_files_impl();
    println!("{PASS_LINE}");
}

#[test]
fn second_rename_failure_keeps_already_published_output() {
    second_rename_failure_keeps_already_published_output_impl();
}

#[test]
fn duplicate_unpack_paths_fail_before_open() {
    duplicate_unpack_paths_fail_before_open_impl();
}

#[test]
fn verify_only_rejects_output_options() {
    verify_only_rejects_output_options_impl();
}

#[test]
fn verify_only_matches_unpack_error_code() {
    verify_only_matches_unpack_error_code_impl();
}

fn verify_pack_publication() -> usize {
    let dir = tempfile::tempdir().expect("tempdir");
    let circuit = fixture("known_mpad_multi.stim");
    let measurements = fixture("known_mpad_multi.measurements.b8");
    let destination = dir.path().join("packed.rsmp");

    write_sentinel(&destination, 0x10);
    let before = directory_entries(dir.path());
    let output = run_cli(&pack_args(&circuit, 4, &measurements, &destination), None);
    assert_success(&output, "successful pack");
    assert_ne!(
        fs::read(&destination).expect("read archive"),
        sentinel(0x10)
    );
    assert_no_rsmp_temps(dir.path());
    assert_eq!(directory_entries(dir.path()), before);

    for (tag, name, input_bytes, envs) in [
        (0x20, "short-input", Some(vec![0x00, 0x00, 0x00]), vec![]),
        (
            0x30,
            "extra-input",
            Some(vec![0x00, 0x00, 0x00, 0x00, 0x00]),
            vec![],
        ),
        (
            0x40,
            "finalization-failure",
            None,
            vec![(FAIL_PACK_FINISH_ENV, "1")],
        ),
    ] {
        let output_path = dir.path().join(format!("{name}.rsmp"));
        write_sentinel(&output_path, tag);
        let input_path = if let Some(bytes) = input_bytes {
            let path = dir.path().join(format!("{name}.b8"));
            fs::write(&path, bytes).expect("write invalid input");
            path
        } else {
            measurements.clone()
        };
        let before = directory_entries(dir.path());
        let output = run_cli_with_env(
            &pack_args(&circuit, 4, &input_path, &output_path),
            None,
            &envs,
        );
        assert_failure(&output, name);
        assert_sentinel(&output_path, tag);
        assert_eq!(directory_entries(dir.path()), before, "{name} leaked files");
        assert_no_rsmp_temps(dir.path());
    }

    1
}

fn verify_unpack_publication() -> usize {
    let dir = tempfile::tempdir().expect("tempdir");
    let archive = fixture("v1/compat-v1.rsmp");
    let circuit = fixture("v1/compat.stim");

    let measurements_out = dir.path().join("measurements.01");
    let detectors_out = dir.path().join("detectors.01");
    let obs_out = dir.path().join("observables.01");
    for (path, tag) in [
        (&measurements_out, 0x50),
        (&detectors_out, 0x51),
        (&obs_out, 0x52),
    ] {
        write_sentinel(path, tag);
    }
    let before = directory_entries(dir.path());
    let output = run_cli(
        &unpack_all_01_args(
            &circuit,
            &archive,
            &measurements_out,
            &detectors_out,
            &obs_out,
        ),
        None,
    );
    assert_success(&output, "successful unpack");
    assert_eq!(
        fs::read(&measurements_out).unwrap(),
        fs::read(fixture("v1/compat-measurements.01")).unwrap()
    );
    assert_eq!(
        fs::read(&detectors_out).unwrap(),
        fs::read(fixture("v1/compat-expected-detectors.01")).unwrap()
    );
    assert_eq!(
        fs::read(&obs_out).unwrap(),
        fs::read(fixture("v1/compat-expected-observables.01")).unwrap()
    );
    assert_eq!(directory_entries(dir.path()), before);
    assert_no_rsmp_temps(dir.path());

    for (index, recipe) in ["checksum_mismatch", "truncated_trailer", "trailing_data"]
        .into_iter()
        .enumerate()
    {
        let corrupt = materialized_corruption(recipe);
        let corrupt_path = dir.path().join(format!("{recipe}.rsmp"));
        fs::write(&corrupt_path, corrupt.archive).expect("write corrupt archive");
        for (path, tag) in [
            (&measurements_out, 0x60 + index as u8),
            (&detectors_out, 0x70 + index as u8),
            (&obs_out, 0x80 + index as u8),
        ] {
            write_sentinel(path, tag);
        }
        let before = directory_entries(dir.path());
        let output = run_cli(
            &unpack_all_01_args(
                &circuit,
                &corrupt_path,
                &measurements_out,
                &detectors_out,
                &obs_out,
            ),
            None,
        );
        assert_failure_with_code(&output, recipe, &corrupt.expected_error);
        assert_sentinel(&measurements_out, 0x60 + index as u8);
        assert_sentinel(&detectors_out, 0x70 + index as u8);
        assert_sentinel(&obs_out, 0x80 + index as u8);
        assert_eq!(
            directory_entries(dir.path()),
            before,
            "{recipe} leaked files"
        );
        assert_no_rsmp_temps(dir.path());
    }

    1
}

fn duplicate_unpack_paths_fail_before_open_impl() {
    let circuit = fixture("v1/compat.stim");

    let duplicate_dir = tempfile::tempdir().expect("duplicate tempdir");
    fs::create_dir(duplicate_dir.path().join("sub")).expect("create subdir");
    let variants = normalized_variants(duplicate_dir.path());
    assert_eq!(variants.len(), 4);
    let guarded_archive = duplicate_dir.path().join("archive.rsmp");
    make_guarded_input(&guarded_archive);
    fs::write(duplicate_dir.path().join("out"), sentinel(0x91)).expect("write sentinel");
    for variant in &variants {
        let before = directory_entries(duplicate_dir.path());
        let run = run_cli_in_dir_with_timeout(
            &[
                "unpack_samples",
                "--circuit",
                circuit.to_str().unwrap(),
                "--in",
                "archive.rsmp",
                "--measurements_out",
                "out",
                "--detectors_out",
                variant.as_str(),
            ],
            duplicate_dir.path(),
            Duration::from_secs(10),
        );
        assert!(
            !run.timed_out,
            "duplicate output opened guarded input for {variant}"
        );
        assert_failure(&run.output, &format!("duplicate output {variant}"));
        assert_eq!(
            fs::read(duplicate_dir.path().join("out")).unwrap(),
            sentinel(0x91)
        );
        assert_eq!(directory_entries(duplicate_dir.path()), before);
        assert_no_rsmp_temps(duplicate_dir.path());
    }

    let collision_dir = tempfile::tempdir().expect("collision tempdir");
    fs::create_dir(collision_dir.path().join("sub")).expect("create subdir");
    let variants = normalized_variants(collision_dir.path());
    make_guarded_input(&collision_dir.path().join("out"));
    for variant in &variants {
        let before = directory_entries(collision_dir.path());
        let run = run_cli_in_dir_with_timeout(
            &[
                "unpack_samples",
                "--circuit",
                circuit.to_str().unwrap(),
                "--in",
                variant.as_str(),
                "--measurements_out",
                "out",
            ],
            collision_dir.path(),
            Duration::from_secs(10),
        );
        assert!(
            !run.timed_out,
            "unpack input/output collision opened guarded input for {variant}"
        );
        assert_failure(
            &run.output,
            &format!("unpack input/output collision {variant}"),
        );
        assert_eq!(directory_entries(collision_dir.path()), before);
        assert_no_rsmp_temps(collision_dir.path());
    }

    let pack_dir = tempfile::tempdir().expect("pack collision tempdir");
    fs::create_dir(pack_dir.path().join("sub")).expect("create subdir");
    let variants = normalized_variants(pack_dir.path());
    make_guarded_input(&pack_dir.path().join("out"));
    for variant in &variants {
        let before = directory_entries(pack_dir.path());
        let run = run_cli_in_dir_with_timeout(
            &[
                "pack_samples",
                "--circuit",
                circuit.to_str().unwrap(),
                "--shots",
                "4",
                "--in",
                variant.as_str(),
                "--in_format",
                "b8",
                "--out",
                "out",
            ],
            pack_dir.path(),
            Duration::from_secs(10),
        );
        assert!(
            !run.timed_out,
            "pack input/output collision opened guarded input for {variant}"
        );
        assert_failure(
            &run.output,
            &format!("pack input/output collision {variant}"),
        );
        assert_eq!(directory_entries(pack_dir.path()), before);
        assert_no_rsmp_temps(pack_dir.path());
    }
}

fn second_rename_failure_keeps_already_published_output_impl() {
    let dir = tempfile::tempdir().expect("tempdir");
    let circuit = fixture("v1/compat.stim");
    let archive = fixture("v1/compat-v1.rsmp");
    let measurements_out = dir.path().join("measurements.01");
    let detectors_out = dir.path().join("detectors.01");
    write_sentinel(&measurements_out, 0xa0);
    write_sentinel(&detectors_out, 0xa1);
    let before = directory_entries(dir.path());

    let output = run_cli_with_env(
        &unpack_measurements_detectors_01_args(
            &circuit,
            &archive,
            &measurements_out,
            &detectors_out,
        ),
        None,
        &[(FAIL_RENAME_ENV, "2")],
    );

    assert_failure_with_code(&output, "second rename failure", "RSMP_IO");
    assert_eq!(
        fs::read(&measurements_out).unwrap(),
        fs::read(fixture("v1/compat-measurements.01")).unwrap()
    );
    assert_sentinel(&detectors_out, 0xa1);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("already published"),
        "diagnostic did not report already-published paths: {stderr}"
    );
    assert!(
        stderr.contains(&measurements_out.display().to_string()),
        "diagnostic did not name first published path: {stderr}"
    );
    assert_eq!(directory_entries(dir.path()), before);
    assert_no_rsmp_temps(dir.path());
}

fn verify_only_rejects_output_options_impl() {
    let dir = tempfile::tempdir().expect("tempdir");
    let circuit = fixture("v1/compat.stim");
    let archive = fixture("v1/compat-v1.rsmp");
    let output_path = dir.path().join("measurements.01");
    let before = directory_entries(dir.path());
    let output = run_cli(
        &[
            "unpack_samples".into(),
            "--circuit".into(),
            circuit.display().to_string(),
            "--in".into(),
            archive.display().to_string(),
            "--verify_only".into(),
            "--measurements_out".into(),
            output_path.display().to_string(),
        ],
        None,
    );
    assert_failure(&output, "verify-only with output");
    assert!(
        !output_path.exists(),
        "verify-only rejection created a result file"
    );
    assert_eq!(directory_entries(dir.path()), before);
    assert_no_rsmp_temps(dir.path());
}

fn verify_only_matches_unpack_error_code_impl() {
    let dir = tempfile::tempdir().expect("tempdir");
    let circuit = fixture("v1/compat.stim");
    let corrupt = materialized_corruption("trailing_data");
    let archive = dir.path().join("trailing-data.rsmp");
    fs::write(&archive, corrupt.archive).expect("write corrupt archive");
    let measurements_out = dir.path().join("ordinary.measurements.01");
    write_sentinel(&measurements_out, 0xb1);

    let ordinary = run_cli(
        &unpack_measurements_01_args(&circuit, &archive, &measurements_out),
        None,
    );
    let verify = run_cli(
        &[
            "unpack_samples".into(),
            "--circuit".into(),
            circuit.display().to_string(),
            "--in".into(),
            archive.display().to_string(),
            "--verify_only".into(),
        ],
        None,
    );

    assert_failure_with_code(
        &ordinary,
        "ordinary corrupt unpack",
        &corrupt.expected_error,
    );
    assert_failure_with_code(
        &verify,
        "verify-only corrupt unpack",
        &corrupt.expected_error,
    );
    assert!(!String::from_utf8_lossy(&ordinary.stdout).contains("PASS rsmp"));
    assert!(!String::from_utf8_lossy(&verify.stdout).contains("PASS rsmp"));
    assert_sentinel(&measurements_out, 0xb1);
    assert_no_rsmp_temps(dir.path());
}

fn verify_only_success_creates_no_files_impl() {
    let dir = tempfile::tempdir().expect("tempdir");
    let circuit = fixture("v1/compat.stim");
    let archive = fixture("v1/compat-v1.rsmp");
    let before = directory_entries(dir.path());

    let output = run_cli_in_dir(
        &[
            "unpack_samples",
            "--circuit",
            circuit.to_str().unwrap(),
            "--in",
            archive.to_str().unwrap(),
            "--verify_only",
        ],
        dir.path(),
    );

    assert_success(&output, "verify-only success");
    assert_eq!(String::from_utf8_lossy(&output.stdout), VERIFY_ONLY_LINE);
    assert!(output.stderr.is_empty(), "verify-only stderr was not empty");
    assert_eq!(directory_entries(dir.path()), before);
    assert_no_rsmp_temps(dir.path());
}

fn materialized_corruption(
    id: &str,
) -> rstim::sample_archive::corruption_corpus::MaterializedCorruption {
    materialize_named_corruption(
        &repo_path("rstim/tests/fixtures/rsmp/catalog.json"),
        &repo_path("rstim/tests/fixtures/rsmp/v1/manifest.toml"),
        id,
    )
    .expect("materialize named corruption recipe")
}

fn pack_args(circuit: &Path, shots: u64, input: &Path, output: &Path) -> Vec<String> {
    vec![
        "pack_samples".into(),
        "--circuit".into(),
        circuit.display().to_string(),
        "--shots".into(),
        shots.to_string(),
        "--in".into(),
        input.display().to_string(),
        "--in_format".into(),
        "b8".into(),
        "--out".into(),
        output.display().to_string(),
    ]
}

fn unpack_measurements_01_args(circuit: &Path, archive: &Path, output: &Path) -> Vec<String> {
    vec![
        "unpack_samples".into(),
        "--circuit".into(),
        circuit.display().to_string(),
        "--in".into(),
        archive.display().to_string(),
        "--measurements_out".into(),
        output.display().to_string(),
        "--measurements_out_format".into(),
        "01".into(),
    ]
}

fn unpack_measurements_detectors_01_args(
    circuit: &Path,
    archive: &Path,
    measurements: &Path,
    detectors: &Path,
) -> Vec<String> {
    let mut args = unpack_measurements_01_args(circuit, archive, measurements);
    args.extend([
        "--detectors_out".into(),
        detectors.display().to_string(),
        "--detectors_out_format".into(),
        "01".into(),
    ]);
    args
}

fn unpack_all_01_args(
    circuit: &Path,
    archive: &Path,
    measurements: &Path,
    detectors: &Path,
    observables: &Path,
) -> Vec<String> {
    let mut args = unpack_measurements_detectors_01_args(circuit, archive, measurements, detectors);
    args.extend([
        "--obs_out".into(),
        observables.display().to_string(),
        "--obs_out_format".into(),
        "01".into(),
    ]);
    args
}

fn normalized_variants(root: &Path) -> Vec<String> {
    let child_cwd = fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    vec![
        "out".to_string(),
        "./out".to_string(),
        "sub/../out".to_string(),
        child_cwd.join("out").display().to_string(),
    ]
}

fn run_cli(args: &[String], stdin: Option<&[u8]>) -> Output {
    run_cli_with_env(args, stdin, &[])
}

fn run_cli_with_env(args: &[String], stdin: Option<&[u8]>, envs: &[(&str, &str)]) -> Output {
    let mut command = rstim_cmd();
    command.args(args);
    for (name, value) in envs {
        command.env(name, value);
    }
    run_command(command, stdin)
}

fn run_cli_in_dir(args: &[&str], cwd: &Path) -> Output {
    let mut command = rstim_cmd();
    command.args(args).current_dir(cwd);
    run_command(command, None)
}

fn run_command(mut command: Command, stdin: Option<&[u8]>) -> Output {
    use std::io::Write;

    command.stdin(if stdin.is_some() {
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
            .expect("open stdin")
            .write_all(bytes)
            .expect("write stdin");
    }
    child.wait_with_output().expect("wait rstim")
}

struct TimedOutput {
    output: Output,
    timed_out: bool,
}

fn run_cli_in_dir_with_timeout(args: &[&str], cwd: &Path, timeout: Duration) -> TimedOutput {
    let mut command = rstim_cmd();
    command
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().expect("spawn rstim");
    let deadline = Instant::now() + timeout;

    loop {
        if child.try_wait().expect("poll rstim").is_some() {
            return TimedOutput {
                output: child.wait_with_output().expect("collect rstim"),
                timed_out: false,
            };
        }
        if Instant::now() >= deadline {
            child.kill().expect("kill stuck rstim");
            return TimedOutput {
                output: child.wait_with_output().expect("collect killed rstim"),
                timed_out: true,
            };
        }
        thread::sleep(Duration::from_millis(10));
    }
}

fn rstim_cmd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_rstim"))
}

fn fixture(name: &str) -> PathBuf {
    repo_path(&format!("rstim/tests/fixtures/rsmp/{name}"))
}

fn repo_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .join(relative)
}

fn write_sentinel(path: &Path, tag: u8) {
    fs::write(path, sentinel(tag)).expect("write sentinel");
}

fn assert_sentinel(path: &Path, tag: u8) {
    assert_eq!(fs::read(path).expect("read sentinel"), sentinel(tag));
}

fn sentinel(tag: u8) -> Vec<u8> {
    vec![tag, tag.wrapping_add(1), tag.wrapping_add(2)]
}

fn directory_entries(root: &Path) -> BTreeSet<OsString> {
    fs::read_dir(root)
        .expect("read dir")
        .map(|entry| entry.expect("directory entry").file_name())
        .collect()
}

fn assert_no_rsmp_temps(root: &Path) {
    for entry in fs::read_dir(root).expect("read dir") {
        let name = entry.expect("directory entry").file_name();
        let name = name.to_string_lossy();
        assert!(
            !name.contains(".rstim-") || !name.ends_with(".tmp"),
            "leaked staged output {name}"
        );
    }
}

fn assert_success(output: &Output, context: &str) {
    assert!(
        output.status.success(),
        "{context} failed: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn assert_failure(output: &Output, context: &str) {
    assert!(
        !output.status.success(),
        "{context} unexpectedly succeeded: stdout={}",
        String::from_utf8_lossy(&output.stdout)
    );
}

fn assert_failure_with_code(output: &Output, context: &str, code: &str) {
    assert_failure(output, context);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(code),
        "{context} stderr did not contain {code}: {stderr}"
    );
}

#[cfg(unix)]
fn make_guarded_input(path: &Path) {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    let c_path = CString::new(path.as_os_str().as_bytes()).expect("path without nul");
    let result = unsafe { libc::mkfifo(c_path.as_ptr(), 0o600) };
    assert_eq!(result, 0, "mkfifo {}", path.display());
}

#[cfg(not(unix))]
fn make_guarded_input(path: &Path) {
    fs::write(path, b"guarded input placeholder").expect("write guarded input");
}
