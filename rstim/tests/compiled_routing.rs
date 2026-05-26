use rstim::compiled::{
    choose_analyzer_path, choose_sampler_path, compile_circuit, CompiledPathDecision,
};
use rstim::parser::parse_lines;

#[test]
fn sampler_path_uses_fast_path_for_simple_repeat_circuit() {
    let compiled =
        compile_circuit(&parse_lines("REPEAT 8 {\n  X_ERROR(0.001) 0\n  M 0\n}\n").unwrap())
            .unwrap();

    assert_eq!(
        choose_sampler_path(&compiled),
        CompiledPathDecision::FastPath
    );
}

#[test]
fn sampler_path_falls_back_for_loss_circuit() {
    let compiled = compile_circuit(&parse_lines("LOSS(1) 0\nMRL 0\n").unwrap()).unwrap();

    assert_eq!(
        choose_sampler_path(&compiled),
        CompiledPathDecision::Fallback("loss instructions require the interpreted path")
    );
}

#[test]
fn sampler_path_falls_back_for_feedback_circuit() {
    let compiled = compile_circuit(&parse_lines("M 0\nCX rec[-1] 0\n").unwrap()).unwrap();

    assert_eq!(
        choose_sampler_path(&compiled),
        CompiledPathDecision::Fallback("feedback instructions require the interpreted path")
    );
}

#[test]
fn analyzer_path_uses_fast_path_for_reset_based_single_repeat_circuit() {
    let compiled = compile_circuit(
        &parse_lines("REPEAT 8 {\n  X_ERROR(0.001) 0\n  MR 0\n  DETECTOR rec[-1]\n}\n").unwrap(),
    )
    .unwrap();

    assert_eq!(
        choose_analyzer_path(&compiled),
        CompiledPathDecision::FastPath
    );
}

#[test]
fn analyzer_path_falls_back_for_loss_circuit() {
    let compiled = compile_circuit(&parse_lines("LOSS(1) 0\nMRL 0\n").unwrap()).unwrap();

    assert_eq!(
        choose_analyzer_path(&compiled),
        CompiledPathDecision::Fallback("loss instructions require the flattened analyzer")
    );
}

#[test]
fn analyzer_path_falls_back_for_non_reset_repeat_circuit() {
    let compiled = compile_circuit(
        &parse_lines("REPEAT 8 {\n  X_ERROR(0.001) 0\n  M 0\n  DETECTOR rec[-1]\n}\n").unwrap(),
    )
    .unwrap();

    assert_eq!(
        choose_analyzer_path(&compiled),
        CompiledPathDecision::Fallback(
            "compiled analyzer currently supports only reset-based single top-level repeat regions",
        )
    );
}

#[test]
fn analyzer_path_falls_back_for_feedback_circuit() {
    let compiled = compile_circuit(&parse_lines("M 0\nCX rec[-1] 0\n").unwrap()).unwrap();

    assert_eq!(
        choose_analyzer_path(&compiled),
        CompiledPathDecision::Fallback("feedback instructions require the flattened analyzer")
    );
}

#[test]
fn analyzer_path_falls_back_for_nested_repeat_circuit() {
    let compiled = compile_circuit(
        &parse_lines("REPEAT 8 {\n  REPEAT 2 {\n    MR 0\n    DETECTOR rec[-1]\n  }\n}\n")
            .unwrap(),
    )
    .unwrap();

    assert_eq!(
        choose_analyzer_path(&compiled),
        CompiledPathDecision::Fallback("nested repeat blocks require the flattened analyzer")
    );
}

#[test]
fn analyzer_path_falls_back_when_qubit_is_touched_after_reset_measurement() {
    let compiled = compile_circuit(
        &parse_lines("REPEAT 8 {\n  MR 0\n  H 0\n  DETECTOR rec[-1]\n}\n").unwrap(),
    )
    .unwrap();

    assert_eq!(
        choose_analyzer_path(&compiled),
        CompiledPathDecision::Fallback(
            "compiled analyzer currently supports only reset-based single top-level repeat regions",
        )
    );
}

#[test]
fn analyzer_path_falls_back_for_cross_iteration_record_lookback() {
    let compiled = compile_circuit(
        &parse_lines("REPEAT 8 {\n  MR 0\n  DETECTOR rec[-2]\n  MR 0\n}\n").unwrap(),
    )
    .unwrap();

    assert_eq!(
        choose_analyzer_path(&compiled),
        CompiledPathDecision::Fallback(
            "compiled analyzer currently supports only reset-based single top-level repeat regions",
        )
    );
}
