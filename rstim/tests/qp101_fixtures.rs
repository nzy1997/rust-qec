use std::fs;
use std::path::{Path, PathBuf};

use rstim::cli;
use rstim::codegen::{repetition_code_memory, surface_code};
use rstim::executor::Executor;
use rstim::ir::{circuit_to_string, StimInstr};
use rstim::qp101::{export_qp101, export_qp101_with_sample_trace, Qp101Document};

const MIXED_NOISE_BASE_FIXTURE: &str = "surface_code_rotated_memory_x_d3_r3_mixed_noise.json";
const MIXED_NOISE_SAMPLE_FIXTURE: &str =
    "surface_code_rotated_memory_x_d3_r3_mixed_noise_sample_seed7.json";
const MIXED_NOISE_SAMPLE_SEED: u64 = 7;
const MIXED_NOISE_ROUNDS: usize = 3;
const MAX_SPARSE_PAULI_TARGETS_PER_KIND: usize = 6;

fn fixture_path(file_name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("qp101")
        .join(file_name)
}

fn load_fixture(file_name: &str) -> Qp101Document {
    let path = fixture_path(file_name);
    assert!(path.exists(), "fixture file does not exist: {}", path.display());

    let text = fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("failed to read fixture {}: {err}", path.display()));
    serde_json::from_str(&text)
        .unwrap_or_else(|err| panic!("failed to parse fixture {} as Qp101Document: {err}", path.display()))
}

fn assert_common_markers(doc: &Qp101Document) {
    assert_eq!(doc.standard, "QP101-ZY");
    assert_eq!(doc.version, "1.0");
    assert!(!doc.operations.is_empty(), "operations should be non-empty");

    let serialized = serde_json::to_value(doc).expect("Qp101Document should serialize");
    let ops = serialized["operations"]
        .as_array()
        .expect("operations should serialize to a JSON array");
    let has_qubit_coords = ops.iter().any(|op| op["type"] == "qubit_coords");
    let has_tick = ops.iter().any(|op| op["type"] == "tick");
    let has_detector = ops.iter().any(|op| op["type"] == "detector");
    let has_observable_include = ops.iter().any(|op| op["type"] == "observable_include");

    assert!(has_qubit_coords, "expected at least one qubit_coords operation");
    assert!(has_tick, "expected at least one tick operation");
    assert!(has_detector, "expected at least one detector operation");
    assert!(
        has_observable_include,
        "expected at least one observable_include operation"
    );
}

fn mixed_noise_showcase_instrs() -> Vec<StimInstr> {
    rstim::showcase::mixed_noise_rotated_memory_x_d3_r3()
}

fn count_target_tokens_for_op(circuit_text: &str, op_prefix: &str) -> usize {
    circuit_text
        .lines()
        .filter_map(|line| line.trim().strip_prefix(op_prefix))
        .map(|suffix| suffix.split_whitespace().count())
        .sum()
}

fn assert_seeded_sample_trace_is_visibly_non_empty(trace: &rstim::sample_trace::SampleTrace) {
    let fired_noise_events = trace
        .noise_events
        .iter()
        .filter(|event| event.occurred)
        .count();
    let visible_measurement_events = trace
        .measurement_events
        .iter()
        .filter(|event| event.bit || event.loss_cause)
        .count();
    let flipped_detector_events = trace
        .detector_events
        .iter()
        .filter(|event| event.flipped)
        .count();

    assert!(
        fired_noise_events > 0,
        "seed {} should produce at least one fired noise event for the showcase sample",
        MIXED_NOISE_SAMPLE_SEED
    );
    assert!(
        visible_measurement_events + flipped_detector_events > 0,
        "seed {} should produce at least one visible measurement or detector consequence",
        MIXED_NOISE_SAMPLE_SEED
    );
}

#[test]
fn repetition_code_fixture_has_expected_qp101_markers() {
    // Regenerate with: rstim gen ... && rstim export_json ... (Task 5 fixture flow).
    let generated = export_qp101(&repetition_code_memory(3, 3, 0.0))
        .expect("export of repetition code should succeed");
    let fixture = load_fixture("repetition_code_memory_d3_r3.json");

    assert_eq!(generated, fixture);
    assert_common_markers(&generated);
}

#[test]
fn surface_code_fixture_has_expected_qp101_markers() {
    let generated = export_qp101(&surface_code::rotated_memory_x(3, 3, 0.0))
        .expect("export of rotated surface code should succeed");
    let fixture = load_fixture("surface_code_rotated_memory_x_d3_r3.json");

    assert_eq!(generated, fixture);
    assert_common_markers(&generated);
}

#[test]
fn mixed_noise_showcase_circuit_uses_generated_after_clifford_loss_and_common_pauli_noise() {
    let instrs = mixed_noise_showcase_instrs();
    let circuit_text = circuit_to_string(&instrs);

    for noise_op in [
        "LOSS(0.01)",
        "X_ERROR(0.01)",
        "Z_ERROR(0.01)",
        "DEPOLARIZE1(0.01)",
        "DEPOLARIZE2(0.01)",
    ] {
        assert!(
            circuit_text.contains(noise_op),
            "mixed-noise showcase is missing required noise op {noise_op}:\n{circuit_text}"
        );
    }

    let loss_target_count = count_target_tokens_for_op(&circuit_text, "LOSS(0.01)");
    assert!(
        loss_target_count >= MIXED_NOISE_ROUNDS * 6,
        "showcase should get dense after-Clifford atom loss from generation, got {loss_target_count} loss targets"
    );
    assert!(
        circuit_text.contains("H 4\nH 6\nH 10\nH 12\nLOSS(0.01) 4 6 10 12"),
        "showcase should place LOSS immediately after H layers:\n{circuit_text}"
    );
    assert!(
        circuit_text.contains("CX") && circuit_text.contains("\nLOSS(0.01)"),
        "showcase should place LOSS after CX layers:\n{circuit_text}"
    );

    let pauli_target_counts = [
        ("X_ERROR(0.01)", count_target_tokens_for_op(&circuit_text, "X_ERROR(0.01)")),
        ("Z_ERROR(0.01)", count_target_tokens_for_op(&circuit_text, "Z_ERROR(0.01)")),
        (
            "DEPOLARIZE1(0.01)",
            count_target_tokens_for_op(&circuit_text, "DEPOLARIZE1(0.01)"),
        ),
        (
            "DEPOLARIZE2(0.01)",
            count_target_tokens_for_op(&circuit_text, "DEPOLARIZE2(0.01)"),
        ),
    ];
    for (noise_op, target_count) in pauli_target_counts {
        assert!(
            target_count > 0,
            "{noise_op} should affect at least one target in the mixed-noise showcase"
        );
        assert!(
            target_count <= MAX_SPARSE_PAULI_TARGETS_PER_KIND,
            "{noise_op} should stay sparse (<= {MAX_SPARSE_PAULI_TARGETS_PER_KIND} targets), got {target_count}"
        );
    }
}

#[test]
fn mixed_noise_showcase_base_fixture_matches_exported_qp101() {
    let instrs = mixed_noise_showcase_instrs();
    let generated =
        export_qp101(&instrs).expect("export of mixed-noise showcase should succeed");
    let fixture = load_fixture(MIXED_NOISE_BASE_FIXTURE);

    assert_eq!(generated, fixture);
    assert_common_markers(&generated);
}

#[test]
fn mixed_noise_showcase_sample_fixture_matches_seeded_trace_export() {
    let instrs = mixed_noise_showcase_instrs();
    let mut executor =
        Executor::from_instrs(instrs.clone()).expect("mixed-noise showcase should execute");
    let mut rng = cli::make_rng(Some(MIXED_NOISE_SAMPLE_SEED));
    let (_out, trace) = executor
        .run_with_trace(&mut rng)
        .expect("mixed-noise showcase should produce a sample trace");
    assert_seeded_sample_trace_is_visibly_non_empty(&trace);
    let generated = export_qp101_with_sample_trace(&instrs, &trace)
        .expect("sample trace export of mixed-noise showcase should succeed");
    let fixture = load_fixture(MIXED_NOISE_SAMPLE_FIXTURE);

    assert_eq!(generated, fixture);
    assert_common_markers(&generated);
}
