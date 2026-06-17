use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rbposd::{Correction, DecodeError, DecodeResult};

#[path = "../dev/parity_runner.rs"]
mod parity_runner;
#[path = "../dev/parity_schema.rs"]
mod parity_schema;

fn unique_temp_path(prefix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("{prefix}-{nanos}"))
}

fn write_case_json(path: &Path, name: &str) {
    let contents = format!(
        r#"{{
  "name": "{name}",
  "matrix": {{
    "num_checks": 1,
    "num_bits": 1,
    "rows": [[0]]
  }},
  "channel": {{
    "kind": "bsc",
    "error_rate": 0.2
  }},
  "syndrome": [true],
  "config": {{
    "max_bp_iterations": 4,
    "early_stop": true,
    "bp_variant": "minimum_sum",
    "schedule": "parallel",
    "osd_variant": "osd0"
  }}
}}"#
    );
    fs::write(path, contents).unwrap();
}

fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else if let Some(message) = payload.downcast_ref::<&str>() {
        message.to_string()
    } else {
        panic!("unexpected panic payload type");
    }
}

fn osd_config() -> parity_schema::ConfigSpec {
    parity_schema::ConfigSpec {
        max_bp_iterations: 0,
        early_stop: true,
        bp_variant: parity_schema::BpVariantSpec::MinimumSum,
        schedule: parity_schema::ScheduleSpec::Parallel,
        osd_variant: parity_schema::OsdVariantSpec::Osd0,
    }
}

#[test]
fn panic_message_handles_all_supported_payload_shapes() {
    assert_eq!(panic_message(Box::new("borrowed panic")), "borrowed panic");

    let unexpected = std::panic::catch_unwind(|| panic_message(Box::new(5usize))).unwrap_err();
    let unexpected_text = panic_message(unexpected);
    assert!(unexpected_text.contains("unexpected panic payload type"));
}

#[test]
fn parity_outcomes_use_stable_error_codes_and_partial_diagnostics_matching() {
    let actual = parity_schema::ParityOutcome::from_decode_result(DecodeResult {
        correction: Correction::from(vec![true, false]),
        converged: false,
        bp_iterations: 7,
        used_osd: true,
        residual_syndrome_weight: 1,
    });

    assert_eq!(
        actual,
        parity_schema::ParityOutcome::Success {
            correction: vec![true, false],
            diagnostics: parity_schema::SuccessDiagnostics {
                converged: Some(false),
                bp_iterations: Some(7),
                used_osd: Some(true),
                residual_syndrome_weight: Some(1),
            },
        }
    );

    let expected_partial = parity_schema::ParityOutcome::Success {
        correction: vec![true, false],
        diagnostics: parity_schema::SuccessDiagnostics {
            converged: Some(false),
            bp_iterations: Some(7),
            used_osd: None,
            residual_syndrome_weight: None,
        },
    };
    assert!(expected_partial.matches_actual(&actual));

    let expected_with_residual = parity_schema::ParityOutcome::Success {
        correction: vec![true, false],
        diagnostics: parity_schema::SuccessDiagnostics {
            residual_syndrome_weight: Some(1),
            ..Default::default()
        },
    };
    assert!(expected_with_residual.matches_actual(&actual));

    let mismatched_diagnostics = parity_schema::ParityOutcome::Success {
        correction: vec![true, false],
        diagnostics: parity_schema::SuccessDiagnostics {
            used_osd: Some(false),
            ..Default::default()
        },
    };
    assert!(!mismatched_diagnostics.matches_actual(&actual));

    let mismatched_correction = parity_schema::ParityOutcome::Success {
        correction: vec![false, false],
        diagnostics: parity_schema::SuccessDiagnostics::default(),
    };
    assert!(!mismatched_correction.matches_actual(&actual));

    let stable_error_cases = [
        (DecodeError::EmptyMatrix, "EmptyMatrix"),
        (DecodeError::InvalidProbability, "InvalidProbability"),
        (
            DecodeError::InvalidColumnIndex {
                column: 3,
                num_bits: 2,
            },
            "InvalidColumnIndex",
        ),
        (
            DecodeError::InvalidRowIndex {
                row: 4,
                num_checks: 2,
            },
            "InvalidRowIndex",
        ),
        (
            DecodeError::DimensionMismatch {
                what: "syndrome",
                expected: 2,
                actual: 1,
            },
            "DimensionMismatch",
        ),
        (DecodeError::SingularSystem, "SingularSystem"),
        (DecodeError::BpDidNotConverge, "BpDidNotConverge"),
        (DecodeError::NoOsdSolution, "NoOsdSolution"),
        (
            DecodeError::UnsupportedLsdOrder { order: 1 },
            "UnsupportedLsdOrder",
        ),
    ];
    for (error, code) in stable_error_cases {
        let expected_error = parity_schema::ParityOutcome::Error {
            error: code.to_string(),
        };
        let actual_error = parity_schema::ParityOutcome::from_decode_error(error);
        assert_eq!(actual_error, expected_error);
        assert!(expected_error.matches_actual(&actual_error));
    }

    let wrong_error = parity_schema::ParityOutcome::Error {
        error: "BpDidNotConverge".to_string(),
    };
    let actual_error =
        parity_schema::ParityOutcome::from_decode_error(DecodeError::NoOsdSolution);
    assert!(!wrong_error.matches_actual(&actual_error));
    assert!(!expected_partial.matches_actual(&actual_error));
}

#[test]
fn run_case_reports_success_build_failures_and_decode_failures() {
    let success_case = parity_schema::ParityCase {
        name: "success_without_expected".to_string(),
        matrix: parity_schema::MatrixSpec {
            num_checks: 1,
            num_bits: 1,
            rows: vec![vec![0]],
        },
        channel: parity_schema::ChannelSpec::Bsc { error_rate: 0.2 },
        syndrome: vec![true],
        config: osd_config(),
        expected: None,
        tags: vec!["success".to_string()],
    };
    let success_report = parity_runner::run_case(&success_case);
    assert_eq!(success_report.name, "success_without_expected");
    assert_eq!(success_report.tags, vec!["success"]);
    assert_eq!(success_report.matches_expected, None);
    assert_eq!(
        success_report.actual,
        parity_schema::ParityOutcome::Success {
            correction: vec![true],
            diagnostics: parity_schema::SuccessDiagnostics {
                converged: Some(false),
                bp_iterations: Some(0),
                used_osd: Some(true),
                residual_syndrome_weight: Some(0),
            },
        }
    );

    let build_error_case = parity_schema::ParityCase {
        name: "invalid_probability".to_string(),
        matrix: parity_schema::MatrixSpec {
            num_checks: 1,
            num_bits: 1,
            rows: vec![vec![0]],
        },
        channel: parity_schema::ChannelSpec::Bsc { error_rate: 1.0 },
        syndrome: vec![true],
        config: osd_config(),
        expected: Some(parity_schema::ParityOutcome::Error {
            error: "InvalidProbability".to_string(),
        }),
        tags: vec!["error".to_string()],
    };
    let build_error_report = parity_runner::run_case(&build_error_case);
    assert_eq!(build_error_report.matches_expected, Some(true));
    assert_eq!(
        build_error_report.actual,
        parity_schema::ParityOutcome::Error {
            error: "InvalidProbability".to_string(),
        }
    );

    let decode_error_case = parity_schema::ParityCase {
        name: "dimension_mismatch".to_string(),
        matrix: parity_schema::MatrixSpec {
            num_checks: 2,
            num_bits: 3,
            rows: vec![vec![0, 1], vec![1, 2]],
        },
        channel: parity_schema::ChannelSpec::BitFlipProbabilities {
            probabilities: vec![0.1, 0.2, 0.3],
        },
        syndrome: vec![true],
        config: osd_config(),
        expected: Some(parity_schema::ParityOutcome::Error {
            error: "DimensionMismatch".to_string(),
        }),
        tags: vec!["error".to_string(), "decode".to_string()],
    };
    let decode_error_report = parity_runner::run_case(&decode_error_case);
    assert_eq!(decode_error_report.matches_expected, Some(true));
    assert_eq!(decode_error_report.tags, vec!["error", "decode"]);
    assert_eq!(
        decode_error_report.actual,
        parity_schema::ParityOutcome::Error {
            error: "DimensionMismatch".to_string(),
        }
    );
}

#[test]
fn load_case_reports_parse_context_and_load_cases_sorts_json_files() {
    let invalid_path = unique_temp_path("rbposd-parity-invalid").with_extension("json");
    fs::write(&invalid_path, "{ definitely not valid json").unwrap();
    let panic = std::panic::catch_unwind(|| parity_schema::load_case(&invalid_path)).unwrap_err();
    let panic_text = panic_message(panic);
    assert!(panic_text.contains("failed to parse"));
    assert!(panic_text.contains(invalid_path.to_str().unwrap()));
    let _ = fs::remove_file(&invalid_path);

    let temp_dir = unique_temp_path("rbposd-parity-cases");
    fs::create_dir(&temp_dir).unwrap();
    let zeta_path = temp_dir.join("zeta.json");
    let alpha_path = temp_dir.join("alpha.json");
    let notes_path = temp_dir.join("notes.txt");
    write_case_json(&zeta_path, "zeta");
    write_case_json(&alpha_path, "alpha");
    fs::write(&notes_path, "ignore me").unwrap();

    let case_names: Vec<String> = parity_schema::load_cases(&temp_dir)
        .into_iter()
        .map(|case| case.name)
        .collect();
    assert_eq!(case_names, vec!["alpha", "zeta"]);

    let _ = fs::remove_file(&zeta_path);
    let _ = fs::remove_file(&alpha_path);
    let _ = fs::remove_file(&notes_path);
    let _ = fs::remove_dir(&temp_dir);
}
