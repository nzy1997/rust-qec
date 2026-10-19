#![cfg(all(feature = "qasm", feature = "unified-cli"))]

use rstim::qasm::{Backend, QasmCircuit, QasmLimits};
use serde_json::Value;
use std::io::Cursor;
use std::path::Path;

struct Invocation {
    ok: bool,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}
fn invoke(args: &[&str], input: &str) -> Invocation {
    invoke_bytes(args, input.as_bytes())
}
fn invoke_bytes(args: &[&str], input: &[u8]) -> Invocation {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let result = rstim::unified_cli::run(
        std::iter::once("rstim").chain(args.iter().copied()),
        &mut Cursor::new(input),
        &mut stdout,
    );
    if let Err(error) = &result {
        rstim::unified_cli::write_error(error, &mut stdout, &mut stderr);
        assert_eq!(rstim::unified_cli::exit_code(error), 2);
    }
    Invocation {
        ok: result.is_ok(),
        stdout,
        stderr,
    }
}
fn success(args: &[&str], input: &str) -> Value {
    let output = invoke(args, input);
    assert!(output.ok, "{}", String::from_utf8_lossy(&output.stderr));
    assert!(output.stderr.is_empty());
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["status"], "ok");
    value
}
fn failure(args: &[&str], input: &str, command: &str, code: &str) -> Value {
    let output = invoke(args, input);
    assert!(!output.ok);
    assert!(
        output.stdout.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    let value: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(value["command"], command);
    assert_eq!(value["error"]["code"], code);
    value
}
fn path(path: &Path) -> &str {
    path.to_str().unwrap()
}
const DYNAMIC: &str = "OPENQASM 2.0; include \"qelib1.inc\"; qreg q[2]; qreg idle[1]; creg unused[2]; creg c[1]; creg out[1]; x q[0]; measure q[0] -> c[0]; reset q[0]; measure q[0] -> c[0]; if(c==0) x q[1]; measure q[1] -> out[0];";
fn import_document(out: &Path, source: &str, backend: &str) {
    success(
        &[
            "circuit",
            "import-qasm",
            "--out",
            path(out),
            "--backend",
            backend,
        ],
        source,
    );
}

#[test]
fn import_stdin_defaults_to_a_complete_document_and_preserves_layout() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("program.qasm2.json");
    let value = success(&["circuit", "import-qasm", "--out", path(&out)], DYNAMIC);
    assert_eq!(value["command"], "circuit.import-qasm");
    assert_eq!(value["result"]["out_format"], "qasm2-json");
    assert_eq!(value["result"]["backend"], "stabilizer");
    assert_eq!(value["result"]["num_qubits"], 3);
    assert_eq!(value["result"]["num_measurements"], 3);
    assert_eq!(value["result"]["num_output_bits"], 4);
    assert_eq!(
        value["result"]["output_layout"]["registers"][0]["name"],
        "unused"
    );
    let circuit = QasmCircuit::from_json(
        &std::fs::read_to_string(&out).unwrap(),
        &QasmLimits::default(),
    )
    .unwrap();
    assert_eq!(circuit.num_qubits(), 3);
    assert_eq!(circuit.num_classical_bits(), 4);
    let plan = circuit
        .lower(Backend::Stabilizer, &QasmLimits::default())
        .unwrap();
    assert_eq!(
        plan.project_classical(&[true, false, true]).unwrap(),
        vec![false, false, false, true]
    );
}

#[test]
fn import_files_resolve_sibling_includes_and_stdin_requires_explicit_paths() {
    let dir = tempfile::tempdir().unwrap();
    let include = dir.path().join("sub");
    std::fs::create_dir(&include).unwrap();
    std::fs::write(include.join("nested.inc"), "gate pulse a { U(pi,0,pi) a; }").unwrap();
    std::fs::write(include.join("g.inc"), "include \"nested.inc\";").unwrap();
    let text = "OPENQASM 2.0; include \"sub/g.inc\"; qreg q[1]; pulse q[0];";
    let input = dir.path().join("input.qasm");
    std::fs::write(&input, text).unwrap();
    let out = dir.path().join("doc.json");
    success(
        &[
            "circuit",
            "import-qasm",
            "--in",
            path(&input),
            "--out",
            path(&out),
        ],
        "",
    );
    failure(
        &["circuit", "import-qasm", "--out", path(&out)],
        text,
        "circuit.import-qasm",
        "qasm_io_error",
    );
    success(
        &[
            "circuit",
            "import-qasm",
            "--in",
            "-",
            "--out",
            path(&out),
            "--include-dir",
            path(dir.path()),
        ],
        text,
    );
    // Cargo.toml exists in the repository cwd but must not be read implicitly.
    failure(
        &["circuit", "import-qasm", "--out", path(&out)],
        "OPENQASM 2.0; include \"Cargo.toml\";",
        "circuit.import-qasm",
        "qasm_io_error",
    );
}

#[test]
fn import_requires_explicit_near_clifford_and_does_not_replace_output_on_failure() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("old.json");
    std::fs::write(&out, "keep").unwrap();
    let source = "OPENQASM 2.0; include \"qelib1.inc\"; qreg q[1]; t q[0]; measure q[0] -> bad[0];";
    let error = failure(
        &["circuit", "import-qasm", "--out", path(&out)],
        source,
        "circuit.import-qasm",
        "qasm_type_error",
    );
    assert!(
        error["error"]["message"]
            .as_str()
            .unwrap()
            .contains("<stdin>:")
    );
    assert_eq!(std::fs::read_to_string(&out).unwrap(), "keep");
    let source = "OPENQASM 2.0; include \"qelib1.inc\"; qreg q[1]; t q[0];";
    failure(
        &["circuit", "import-qasm", "--out", path(&out)],
        source,
        "circuit.import-qasm",
        "qasm_backend_unsupported",
    );
    assert_eq!(std::fs::read_to_string(&out).unwrap(), "keep");
    let value = success(
        &[
            "circuit",
            "import-qasm",
            "--out",
            path(&out),
            "--backend",
            "near-clifford",
        ],
        source,
    );
    assert_eq!(value["result"]["backend"], "near-clifford");
}

#[test]
fn stim_projection_is_explicit_and_reports_layout_loss() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("projection.stim");
    let value = success(
        &[
            "circuit",
            "import-qasm",
            "--out",
            path(&out),
            "--out-format",
            "stim",
        ],
        DYNAMIC,
    );
    assert_eq!(value["result"]["out_format"], "stim");
    assert!(!value["warnings"].as_array().unwrap().is_empty());
    let native = std::fs::read_to_string(&out).unwrap();
    assert!(native.contains("M 0"));
    assert!(native.contains("rec[-1]"));
}

#[test]
fn document_and_native_export_support_stdin_file_inputs_and_feedback() {
    let dir = tempfile::tempdir().unwrap();
    let doc = dir.path().join("doc.json");
    let out = dir.path().join("roundtrip.qasm");
    import_document(&doc, DYNAMIC, "stabilizer");
    let value = success(
        &[
            "circuit",
            "export-qasm",
            "--in",
            path(&doc),
            "--in-format",
            "qasm2-json",
            "--out",
            path(&out),
        ],
        "",
    );
    assert_eq!(value["command"], "circuit.export-qasm");
    assert_eq!(value["result"]["out_format"], "qasm2");
    let reparsed = rstim::qasm::from_qasm(&std::fs::read_to_string(&out).unwrap()).unwrap();
    assert_eq!(reparsed.num_classical_bits(), 4);
    assert_eq!(reparsed.num_qubits(), 3);
    success(
        &["circuit", "export-qasm", "--in", "-", "--out", path(&out)],
        "H 0\nM 0\nCX rec[-1] 1\nM 1\n",
    );
    let native = rstim::qasm::from_qasm(&std::fs::read_to_string(&out).unwrap()).unwrap();
    assert_eq!(native.num_classical_bits(), 2);
}

#[test]
fn annotation_omission_is_explicit_and_never_discards_noise() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("export.qasm");
    std::fs::write(&out, "keep").unwrap();
    let source = "M 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n";
    failure(
        &["circuit", "export-qasm", "--out", path(&out)],
        source,
        "circuit.export-qasm",
        "qasm_unrepresentable_export",
    );
    assert_eq!(std::fs::read_to_string(&out).unwrap(), "keep");
    let report = success(
        &[
            "circuit",
            "export-qasm",
            "--out",
            path(&out),
            "--omit-annotations",
        ],
        source,
    );
    assert_eq!(report["warnings"].as_array().unwrap().len(), 2);
    assert_eq!(
        report["result"]["omitted_annotations"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    std::fs::write(&out, "keep").unwrap();
    failure(
        &[
            "circuit",
            "export-qasm",
            "--out",
            path(&out),
            "--omit-annotations",
        ],
        "X_ERROR(0.1) 0\nM 0\n",
        "circuit.export-qasm",
        "qasm_unrepresentable_export",
    );
    assert_eq!(std::fs::read_to_string(&out).unwrap(), "keep");
}

#[test]
fn sampling_classical_view_preserves_zero_bits_and_latest_writes() {
    let dir = tempfile::tempdir().unwrap();
    let doc = dir.path().join("doc.json");
    import_document(&doc, DYNAMIC, "stabilizer");
    for backend in ["stabilizer", "near-clifford"] {
        let out = dir.path().join(format!("{backend}.01"));
        let report = success(
            &[
                "circuit",
                "sample",
                "--in",
                path(&doc),
                "--in-format",
                "qasm2-json",
                "--backend",
                backend,
                "--result-view",
                "classical",
                "--shots",
                "5",
                "--seed",
                "8",
                "--out",
                path(&out),
            ],
            "",
        );
        assert_eq!(std::fs::read_to_string(&out).unwrap(), "0001\n".repeat(5));
        assert_eq!(report["result"]["backend"], backend);
        assert_eq!(report["result"]["result_view"], "classical");
        assert_eq!(report["result"]["num_measurements"], 3);
        assert_eq!(report["result"]["num_output_bits"], 4);
        assert_eq!(report["result"]["output_layout"]["registers"][0]["size"], 2);
    }
}

#[test]
fn sampling_raw_view_keeps_all_measurement_events() {
    let dir = tempfile::tempdir().unwrap();
    let doc = dir.path().join("doc.json");
    import_document(&doc, DYNAMIC, "stabilizer");
    let out = dir.path().join("raw.01");
    let text = std::fs::read_to_string(&doc).unwrap();
    let report = success(
        &[
            "circuit",
            "sample",
            "--in",
            "-",
            "--in-format",
            "qasm2-json",
            "--shots",
            "3",
            "--out",
            path(&out),
        ],
        &text,
    );
    assert_eq!(std::fs::read_to_string(&out).unwrap(), "101\n".repeat(3));
    assert_eq!(report["result"]["result_view"], "measurement-record");
    assert_eq!(report["result"]["num_output_bits"], 3);
}

#[test]
fn qasm_sampling_preserves_all_existing_measurement_output_formats() {
    let dir = tempfile::tempdir().unwrap();
    let doc = dir.path().join("doc.json");
    import_document(&doc, DYNAMIC, "stabilizer");
    for format in ["01", "b8", "r8", "hits", "ptb64"] {
        let out = dir.path().join(format!("shots.{format}"));
        success(
            &[
                "circuit",
                "sample",
                "--in",
                path(&doc),
                "--in-format",
                "qasm2-json",
                "--result-view",
                "classical",
                "--shots",
                "3",
                "--out-format",
                format,
                "--out",
                path(&out),
            ],
            "",
        );
        let bytes = std::fs::read(&out).unwrap();
        let table = match format {
            "01" => rstim::output::read_shots_01(&bytes, 4),
            "b8" => rstim::output::read_shots_b8(&bytes, 4),
            "r8" => rstim::output::read_shots_r8(&bytes, 4),
            "hits" => rstim::output::read_shots_hits(&bytes, 4),
            _ => rstim::output::read_shots_ptb64(&bytes, 4, 3),
        }
        .unwrap();
        assert_eq!(table.num_minor(), 3);
        for shot in 0..3 {
            assert_eq!(
                (0..4).map(|bit| table.get(bit, shot)).collect::<Vec<_>>(),
                vec![false, false, false, true]
            );
        }
    }
}

#[test]
fn explicit_near_clifford_sampling_executes_native_and_qasm_nonclifford_gates() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("near.01");
    let report = success(
        &[
            "circuit",
            "sample",
            "--shots",
            "8",
            "--backend",
            "near-clifford",
            "--out",
            path(&out),
        ],
        "T 0\nX 0\nM 0\n",
    );
    assert_eq!(report["result"]["backend"], "near-clifford");
    assert_eq!(std::fs::read_to_string(&out).unwrap(), "1\n".repeat(8));
    let doc = dir.path().join("near.json");
    import_document(
        &doc,
        "OPENQASM 2.0; include \"qelib1.inc\"; qreg q[1]; creg c[1]; t q[0]; x q[0]; measure q[0] -> c[0];",
        "near-clifford",
    );
    std::fs::write(&out, "keep").unwrap();
    failure(
        &[
            "circuit",
            "sample",
            "--in",
            path(&doc),
            "--in-format",
            "qasm2-json",
            "--shots",
            "8",
            "--out",
            path(&out),
        ],
        "",
        "circuit.sample",
        "qasm_backend_unsupported",
    );
    assert_eq!(std::fs::read_to_string(&out).unwrap(), "keep");
    success(
        &[
            "circuit",
            "sample",
            "--in",
            path(&doc),
            "--in-format",
            "qasm2-json",
            "--backend",
            "near-clifford",
            "--shots",
            "8",
            "--out",
            path(&out),
        ],
        "",
    );
    assert_eq!(std::fs::read_to_string(&out).unwrap(), "1\n".repeat(8));
}

#[test]
fn missing_layout_and_inapplicable_flags_fail_before_output_publication() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("old");
    std::fs::write(&out, "keep").unwrap();
    failure(
        &[
            "circuit",
            "sample",
            "--shots",
            "1",
            "--out",
            path(&out),
            "--result-view",
            "classical",
        ],
        "M 0\n",
        "circuit.sample",
        "qasm_missing_layout",
    );
    failure(
        &[
            "circuit",
            "sample",
            "--shots",
            "1",
            "--out",
            path(&out),
            "--backend",
            "near-clifford",
            "--skip-reference-sample",
        ],
        "M 0\n",
        "circuit.sample",
        "invalid_arguments",
    );
    let doc = dir.path().join("doc.json");
    import_document(&doc, DYNAMIC, "stabilizer");
    failure(
        &[
            "circuit",
            "sample",
            "--shots",
            "1",
            "--out",
            path(&out),
            "--in",
            path(&doc),
            "--in-format",
            "qasm2-json",
            "--skip-reference-sample",
        ],
        "",
        "circuit.sample",
        "invalid_arguments",
    );
    failure(
        &[
            "circuit",
            "sample",
            "--shots",
            "1",
            "--out",
            path(&out),
            "--in",
            path(&doc),
            "--in-format",
            "qasm2-json",
            "--out-format",
            "dets",
        ],
        "",
        "circuit.sample",
        "invalid_arguments",
    );
    assert_eq!(std::fs::read_to_string(&out).unwrap(), "keep");
}

#[test]
fn sampling_resource_errors_are_actionable_and_atomic() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("old");
    std::fs::write(&out, "keep").unwrap();
    let report = failure(
        &[
            "circuit",
            "sample",
            "--shots",
            "1",
            "--out",
            path(&out),
            "--backend",
            "near-clifford",
            "--max-active-rank",
            "0",
        ],
        "H 0\nT 0\nH 0\nM 0\n",
        "circuit.sample",
        "qasm_backend_unsupported",
    );
    assert!(
        report["error"]["message"]
            .as_str()
            .unwrap()
            .contains("max-active-rank")
    );
    failure(
        &[
            "circuit",
            "sample",
            "--shots",
            "18446744073709551615",
            "--out",
            path(&out),
            "--backend",
            "near-clifford",
        ],
        "M 0\n",
        "circuit.sample",
        "qasm_limit_exceeded",
    );
    assert_eq!(std::fs::read_to_string(&out).unwrap(), "keep");
}

#[test]
fn native_repeat_depth_empty_expansion_and_overflow_are_bounded_before_compilation() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("old");
    std::fs::write(&out, "keep").unwrap();
    for source in [
        "REPEAT 1000001 {\n}\n".to_owned(),
        "REPEAT 18446744073709551615 {\nM 0\n}\n".to_owned(),
        format!(
            "{}M 0\n{}",
            "REPEAT 1 {\n".repeat(10000),
            "}\n".repeat(10000)
        ),
    ] {
        failure(
            &[
                "circuit",
                "sample",
                "--shots",
                "1",
                "--backend",
                "near-clifford",
                "--out",
                path(&out),
            ],
            &source,
            "circuit.sample",
            "qasm_limit_exceeded",
        );
        failure(
            &["circuit", "export-qasm", "--out", path(&out)],
            &source,
            "circuit.export-qasm",
            "qasm_limit_exceeded",
        );
    }
    assert_eq!(std::fs::read_to_string(&out).unwrap(), "keep");
}

#[test]
fn malformed_documents_and_large_sources_preserve_existing_artifacts() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("old");
    std::fs::write(&out, "keep").unwrap();
    failure(
        &[
            "circuit",
            "export-qasm",
            "--in-format",
            "qasm2-json",
            "--out",
            path(&out),
        ],
        "{}",
        "circuit.export-qasm",
        "qasm_invalid_document",
    );
    failure(
        &[
            "circuit",
            "sample",
            "--in-format",
            "qasm2-json",
            "--shots",
            "1",
            "--out",
            path(&out),
        ],
        "{}",
        "circuit.sample",
        "qasm_invalid_document",
    );
    failure(
        &["circuit", "import-qasm", "--out", path(&out)],
        &" ".repeat(4 * 1024 * 1024 + 1),
        "circuit.import-qasm",
        "qasm_limit_exceeded",
    );
    assert_eq!(std::fs::read_to_string(&out).unwrap(), "keep");
    assert_eq!(
        std::fs::read_dir(dir.path()).unwrap().count(),
        1,
        "temporary artifacts must be cleaned up"
    );
}

#[test]
fn clap_errors_use_the_conversion_command_and_selected_error_channel() {
    failure(
        &["circuit", "import-qasm", "--backend", "invalid"],
        "",
        "circuit.import-qasm",
        "invalid_arguments",
    );
    failure(
        &["circuit", "export-qasm", "--in-format", "invalid"],
        "",
        "circuit.export-qasm",
        "invalid_arguments",
    );
    let result = invoke(&["--error-format", "human", "circuit", "import-qasm"], "");
    assert!(!result.ok);
    assert!(serde_json::from_slice::<Value>(&result.stderr).is_err());
}

#[test]
fn unchanged_native_defaults_keep_existing_output_and_result_contract() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("native.01");
    let report = success(
        &["circuit", "sample", "--shots", "3", "--out", path(&out)],
        "X 0\nM 0\n",
    );
    assert_eq!(std::fs::read_to_string(&out).unwrap(), "1\n".repeat(3));
    assert_eq!(report["result"]["num_measurements"], 1);
    assert!(report["result"].get("backend").is_none());
    success(
        &[
            "circuit",
            "sample",
            "--shots",
            "3",
            "--out",
            path(&out),
            "--skip-reference-sample",
        ],
        "R 0\nM 0\n",
    );
    assert_eq!(std::fs::read_to_string(&out).unwrap(), "0\n".repeat(3));
}

#[test]
fn enabled_capabilities_expose_qasm_conversion_commands() {
    let output = invoke(&["capabilities", "--format", "json"], "");
    assert!(output.ok);
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    let names: Vec<_> = value["commands"]
        .as_array()
        .unwrap()
        .iter()
        .map(|command| command["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"circuit.import-qasm"));
    assert!(names.contains(&"circuit.export-qasm"));
}

#[test]
fn empty_measurement_records_still_emit_declared_classical_zeros_and_zero_shots() {
    let dir = tempfile::tempdir().unwrap();
    let doc = dir.path().join("idle.json");
    let out = dir.path().join("idle.01");
    import_document(&doc, "OPENQASM 2.0; qreg idle[2]; creg c[3];", "stabilizer");
    for backend in ["stabilizer", "near-clifford"] {
        let value = success(
            &[
                "circuit",
                "sample",
                "--in",
                path(&doc),
                "--in-format",
                "qasm2-json",
                "--backend",
                backend,
                "--result-view",
                "classical",
                "--shots",
                "2",
                "--out",
                path(&out),
            ],
            "",
        );
        assert_eq!(value["result"]["num_measurements"], 0);
        assert_eq!(value["result"]["num_output_bits"], 3);
        assert_eq!(std::fs::read_to_string(&out).unwrap(), "000\n000\n");
        success(
            &[
                "circuit",
                "sample",
                "--in",
                path(&doc),
                "--in-format",
                "qasm2-json",
                "--backend",
                backend,
                "--result-view",
                "classical",
                "--shots",
                "0",
                "--out",
                path(&out),
            ],
            "",
        );
        assert!(std::fs::read(&out).unwrap().is_empty());
    }
}

#[test]
fn read_failures_and_invalid_utf8_preserve_output_and_command_error_context() {
    struct BrokenReader;
    impl std::io::Read for BrokenReader {
        fn read(&mut self, _buffer: &mut [u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("injected upstream read failure"))
        }
    }
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("keep.txt");
    std::fs::write(&out, "keep").unwrap();
    let missing = dir.path().join("missing-input");
    for (verb, command, extra) in [
        ("import-qasm", "circuit.import-qasm", vec![]),
        ("export-qasm", "circuit.export-qasm", vec![]),
        (
            "sample",
            "circuit.sample",
            vec!["--shots", "1", "--backend", "near-clifford"],
        ),
    ] {
        let mut args = vec!["circuit", verb, "--out", path(&out)];
        args.extend(extra);
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let err = rstim::unified_cli::run(
            std::iter::once("rstim").chain(args.iter().copied()),
            &mut BrokenReader,
            &mut stdout,
        )
        .unwrap_err();
        rstim::unified_cli::write_error(&err, &mut stdout, &mut stderr);
        let report: Value = serde_json::from_slice(&stderr).unwrap();
        assert_eq!(report["command"], command);
        assert_eq!(report["error"]["code"], "input_error");
        assert!(
            report["error"]["message"]
                .as_str()
                .unwrap()
                .contains("upstream read failure")
        );
        assert!(stdout.is_empty());
        let result = invoke_bytes(&args, &[0xff]);
        assert!(!result.ok);
        assert!(result.stdout.is_empty());
        let report: Value = serde_json::from_slice(&result.stderr).unwrap();
        assert_eq!(report["error"]["code"], "input_error");
        assert!(
            report["error"]["message"]
                .as_str()
                .unwrap()
                .contains("UTF-8")
        );
        args.extend(["--in", path(&missing)]);
        let code = if verb == "import-qasm" {
            "qasm_io_error"
        } else {
            "input_error"
        };
        failure(&args, "", command, code);
        assert_eq!(std::fs::read_to_string(&out).unwrap(), "keep");
    }
}

#[test]
fn failing_atomic_publication_cleans_temporaries_and_preserves_destination_directories() {
    let dir = tempfile::tempdir().unwrap();
    let destination = dir.path().join("existing-dir");
    std::fs::create_dir(&destination).unwrap();
    std::fs::write(destination.join("sentinel"), "keep").unwrap();
    let missing = dir.path().join("missing-parent/output");
    for (verb, command, source, extra) in [
        (
            "import-qasm",
            "circuit.import-qasm",
            "OPENQASM 2.0;",
            vec![],
        ),
        ("export-qasm", "circuit.export-qasm", "M 0\n", vec![]),
        (
            "sample",
            "circuit.sample",
            "M 0\n",
            vec!["--shots", "1", "--backend", "near-clifford"],
        ),
    ] {
        for path_out in [&missing, &destination] {
            let mut args = vec!["circuit", verb, "--out", path(path_out)];
            args.extend(extra.iter().copied());
            failure(&args, source, command, "output_error");
        }
        assert_eq!(
            std::fs::read_to_string(destination.join("sentinel")).unwrap(),
            "keep"
        );
        assert_eq!(
            std::fs::read_dir(dir.path()).unwrap().count(),
            1,
            "failed publication must clean temporary files"
        );
    }
}

#[test]
fn stdin_includes_reject_invalid_utf8_and_oversize_files_without_publication() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("keep");
    std::fs::write(&out, "keep").unwrap();
    let included = dir.path().join("g.inc");
    let source = "OPENQASM 2.0; include \"g.inc\";";
    let args = [
        "circuit",
        "import-qasm",
        "--include-dir",
        path(dir.path()),
        "--out",
        path(&out),
    ];
    std::fs::write(&included, [0xff]).unwrap();
    failure(&args, source, "circuit.import-qasm", "qasm_io_error");
    std::fs::write(&included, vec![b' '; 4 * 1024 * 1024 + 1]).unwrap();
    failure(&args, source, "circuit.import-qasm", "qasm_limit_exceeded");
    assert_eq!(std::fs::read_to_string(&out).unwrap(), "keep");
}

#[test]
fn human_conversion_and_sampling_reports_show_artifacts_warnings_and_selected_profile() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("converted");
    for (args, source, expected) in [
        (
            vec![
                "circuit",
                "import-qasm",
                "--format",
                "human",
                "--out-format",
                "stim",
                "--out",
                path(&out),
            ],
            DYNAMIC,
            "Stim execution projection omits",
        ),
        (
            vec![
                "circuit",
                "export-qasm",
                "--format",
                "human",
                "--omit-annotations",
                "--out",
                path(&out),
            ],
            "M 0\nDETECTOR rec[-1]\n",
            "Omitted annotation DETECTOR",
        ),
        (
            vec![
                "circuit",
                "sample",
                "--format",
                "human",
                "--backend",
                "near-clifford",
                "--shots",
                "2",
                "--out",
                path(&out),
            ],
            "M 0\n",
            "backend: near-clifford",
        ),
    ] {
        let result = invoke(&args, source);
        assert!(result.ok, "{}", String::from_utf8_lossy(&result.stderr));
        assert!(result.stderr.is_empty());
        let text = String::from_utf8(result.stdout).unwrap();
        assert!(text.contains("status: ok"));
        assert!(text.contains("artifact: "));
        assert!(text.contains(expected), "{text}");
    }
    let result = invoke(
        &[
            "circuit",
            "import-qasm",
            "--format",
            "human",
            "--error-format",
            "json",
            "--out",
            path(&out),
        ],
        "invalid",
    );
    assert!(!result.ok);
    let report: Value = serde_json::from_slice(&result.stderr).unwrap();
    assert_eq!(report["command"], "circuit.import-qasm");
}

#[test]
fn bounded_native_repeats_preserve_records_and_cross_iteration_feedback() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("native.01");
    let source = "X 0\nREPEAT 2 {\nM 0\nCX rec[-1] 1\n}\nM 1\n";
    let report = success(
        &[
            "circuit",
            "sample",
            "--shots",
            "2",
            "--backend",
            "near-clifford",
            "--out",
            path(&out),
        ],
        source,
    );
    assert_eq!(report["result"]["num_measurements"], 3);
    assert_eq!(std::fs::read_to_string(&out).unwrap(), "110\n110\n");
    let qasm = dir.path().join("repeat.qasm");
    success(&["circuit", "export-qasm", "--out", path(&qasm)], source);
    let checked = rstim::qasm::from_qasm(&std::fs::read_to_string(&qasm).unwrap()).unwrap();
    let plan = checked
        .lower(Backend::Stabilizer, &QasmLimits::default())
        .unwrap();
    assert_eq!(plan.measurement_events.len(), 3);
    success(
        &[
            "circuit",
            "sample",
            "--shots",
            "2",
            "--backend",
            "near-clifford",
            "--out",
            path(&out),
        ],
        "REPEAT 3 {\n}\n",
    );
    assert_eq!(std::fs::read_to_string(&out).unwrap(), "\n\n");
}

#[test]
fn unsupported_native_shapes_bad_formats_and_backend_only_flags_do_not_replace_outputs() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("keep");
    std::fs::write(&out, "keep").unwrap();
    let args = [
        "circuit",
        "sample",
        "--shots",
        "1",
        "--backend",
        "near-clifford",
        "--out",
        path(&out),
    ];
    failure(&args, "M bad-target\n", "circuit.sample", "invalid_circuit");
    failure(
        &args,
        "CX 0\n",
        "circuit.sample",
        "qasm_backend_unsupported",
    );
    failure(&args, "M 5000\n", "circuit.sample", "qasm_limit_exceeded");
    failure(
        &[
            "circuit",
            "sample",
            "--shots",
            "1",
            "--out",
            path(&out),
            "--max-active-rank",
            "3",
        ],
        "M 0\n",
        "circuit.sample",
        "invalid_arguments",
    );
    let doc = dir.path().join("doc.json");
    import_document(&doc, DYNAMIC, "stabilizer");
    failure(
        &[
            "circuit",
            "sample",
            "--shots",
            "1",
            "--in",
            path(&doc),
            "--in-format",
            "qasm2-json",
            "--out-format",
            "bad",
            "--out",
            path(&out),
        ],
        "",
        "circuit.sample",
        "invalid_arguments",
    );
    assert_eq!(std::fs::read_to_string(&out).unwrap(), "keep");
}

#[test]
fn report_channel_failures_leave_completed_artifacts_and_return_output_errors() {
    struct BrokenReport {
        only_warning: bool,
    }
    impl std::io::Write for BrokenReport {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if !self.only_warning || bytes.starts_with(b"warning: ") {
                Err(std::io::Error::other("report channel closed"))
            } else {
                Ok(bytes.len())
            }
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("published.stim");
    for (format, only_warning) in [("json", false), ("human", false), ("human", true)] {
        let args = [
            "rstim",
            "circuit",
            "import-qasm",
            "--out",
            path(&out),
            "--out-format",
            "stim",
            "--format",
            format,
            "--error-format",
            "json",
        ];
        let err = rstim::unified_cli::run(
            args,
            &mut Cursor::new(DYNAMIC),
            &mut BrokenReport { only_warning },
        )
        .unwrap_err();
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        rstim::unified_cli::write_error(&err, &mut stdout, &mut stderr);
        let report: Value = serde_json::from_slice(&stderr).unwrap();
        assert_eq!(report["command"], "circuit.import-qasm");
        assert_eq!(report["error"]["code"], "output_error");
        assert!(
            report["error"]["message"]
                .as_str()
                .unwrap()
                .contains("report channel closed")
        );
        let native = std::fs::read_to_string(&out).unwrap();
        assert!(native.contains("M 0"));
        assert!(native.contains("rec[-1]"));
    }
}

#[test]
fn include_directory_search_falls_through_missing_files_and_respects_declared_priority() {
    let dir = tempfile::tempdir().unwrap();
    let first = dir.path().join("first");
    let second = dir.path().join("second");
    std::fs::create_dir(&first).unwrap();
    std::fs::create_dir(&second).unwrap();
    std::fs::write(second.join("g.inc"), "gate pulse a { U(pi,0,pi) a; }").unwrap();
    let out = dir.path().join("program.json");
    let samples = dir.path().join("result.01");
    let source =
        "OPENQASM 2.0; include \"g.inc\"; qreg q[1]; creg c[1]; pulse q[0]; measure q[0] -> c[0];";
    let args = [
        "circuit",
        "import-qasm",
        "--include-dir",
        path(&first),
        "--include-dir",
        path(&second),
        "--out",
        path(&out),
    ];
    for (first_definition, expected) in
        [(None, "1\n"), (Some("gate pulse a { U(0,0,0) a; }"), "0\n")]
    {
        if let Some(definition) = first_definition {
            std::fs::write(first.join("g.inc"), definition).unwrap();
        }
        success(&args, source);
        success(
            &[
                "circuit",
                "sample",
                "--in",
                path(&out),
                "--in-format",
                "qasm2-json",
                "--shots",
                "1",
                "--out",
                path(&samples),
            ],
            "",
        );
        assert_eq!(std::fs::read_to_string(&samples).unwrap(), expected);
    }
}

#[test]
fn separate_native_operation_and_measurement_budgets_preflight_expansion_before_execution() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("keep");
    std::fs::write(&out, "keep").unwrap();
    let args = [
        "circuit",
        "sample",
        "--backend",
        "near-clifford",
        "--shots",
        "1",
        "--out",
        path(&out),
    ];
    // Charging only repeat iterations would overlook the doubled expanded work.
    failure(
        &args,
        "REPEAT 1000000 {\nI 0\n}\n",
        "circuit.sample",
        "qasm_limit_exceeded",
    );
    // One multi-target measurement op can exceed the event budget on its own.
    let source = format!(
        "M {}\n",
        "0 ".repeat(QasmLimits::default().max_operations + 1)
    );
    failure(&args, &source, "circuit.sample", "qasm_limit_exceeded");
    let shots = QasmLimits::default().max_output_bytes.to_string();
    failure(
        &[
            "circuit",
            "sample",
            "--backend",
            "near-clifford",
            "--shots",
            &shots,
            "--out",
            path(&out),
        ],
        "M 0\n",
        "circuit.sample",
        "qasm_limit_exceeded",
    );
    assert_eq!(std::fs::read_to_string(&out).unwrap(), "keep");
}
