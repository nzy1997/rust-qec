use crate::compiled::{CompiledBlock, CompiledCircuit, CompiledRepeatRegion};
use crate::ir::{StimInstr, StimTarget};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompiledPathDecision {
    FastPath,
    Fallback(&'static str),
}

pub fn choose_sampler_path(compiled: &CompiledCircuit) -> CompiledPathDecision {
    if compiled.flags.has_loss {
        return CompiledPathDecision::Fallback("loss instructions require the interpreted path");
    }
    if compiled.flags.has_feedback {
        return CompiledPathDecision::Fallback(
            "feedback instructions require the interpreted path",
        );
    }

    CompiledPathDecision::FastPath
}

pub fn choose_analyzer_path(compiled: &CompiledCircuit) -> CompiledPathDecision {
    if compiled.flags.has_loss {
        return CompiledPathDecision::Fallback("loss instructions require the flattened analyzer");
    }
    if compiled.flags.has_feedback {
        return CompiledPathDecision::Fallback(
            "feedback instructions require the flattened analyzer",
        );
    }
    if compiled.flags.has_nested_repeat {
        return CompiledPathDecision::Fallback(
            "nested repeat blocks require the flattened analyzer",
        );
    }
    if let [CompiledBlock::Repeat(region)] = compiled.blocks.as_slice() {
        if supports_reset_periodic_body(region) {
            return CompiledPathDecision::FastPath;
        }
        return CompiledPathDecision::Fallback(
            "compiled analyzer currently supports only reset-based single top-level repeat regions",
        );
    }

    CompiledPathDecision::Fallback(
        "compiled analyzer currently supports only a single top-level repeat region",
    )
}

fn supports_reset_periodic_body(region: &CompiledRepeatRegion) -> bool {
    let mut has_reset_measurement = false;
    let mut measurements_seen = 0usize;
    let mut last_touch_was_reset_measurement = BTreeMap::<u32, bool>::new();

    for instr in &region.body_source {
        let StimInstr::Op { name, targets, .. } = instr else {
            return false;
        };

        if targets.iter().any(|target| {
            matches!(
                target,
                StimTarget::Rec(offset) if *offset >= 0 || ((-*offset) as usize) > measurements_seen
            )
        }) {
            return false;
        }

        match name.as_str() {
            "MR" | "MRX" | "MRY" | "MRZ" => {
                has_reset_measurement = true;
                measurements_seen += targets.iter().filter_map(StimTarget::qubit_index).count();
                for qubit in targets.iter().filter_map(StimTarget::qubit_index) {
                    last_touch_was_reset_measurement.insert(qubit, true);
                }
            }
            "M" | "MX" | "MY" | "MZ" | "MXX" | "MYY" | "MZZ" | "MPP" | "SPP" | "SPP_DAG" => {
                return false;
            }
            _ => {
                for qubit in targets.iter().filter_map(StimTarget::qubit_index) {
                    last_touch_was_reset_measurement.insert(qubit, false);
                }
            }
        }
    }

    has_reset_measurement
        && !last_touch_was_reset_measurement.is_empty()
        && last_touch_was_reset_measurement.values().all(|was_reset| *was_reset)
}

#[cfg(test)]
mod tests {
    use super::supports_reset_periodic_body;
    use crate::compiled::{compile_circuit, CompiledBlock, CompiledRepeatRegion};
    use crate::parser::parse_lines;

    fn top_level_repeat_region<'a>(blocks: &'a [CompiledBlock]) -> &'a CompiledRepeatRegion {
        match blocks {
            [CompiledBlock::Repeat(region)] => region,
            _ => panic!("expected top-level repeat"),
        }
    }

    #[test]
    fn supports_reset_periodic_body_requires_reset_measurements() {
        let reset_repeat =
            compile_circuit(&parse_lines("REPEAT 8 {\n  X_ERROR(0.1) 0\n  MR 0\n}\n").unwrap())
                .unwrap();
        let plain_measure_repeat =
            compile_circuit(&parse_lines("REPEAT 8 {\n  X_ERROR(0.1) 0\n  M 0\n}\n").unwrap())
                .unwrap();

        let reset_region = top_level_repeat_region(reset_repeat.blocks.as_slice());
        let plain_region = top_level_repeat_region(plain_measure_repeat.blocks.as_slice());

        assert!(supports_reset_periodic_body(reset_region));
        assert!(!supports_reset_periodic_body(plain_region));
    }

    #[test]
    fn supports_reset_periodic_body_rejects_nested_repeat_in_body_source() {
        let nested_repeat =
            compile_circuit(&parse_lines("REPEAT 8 {\n  REPEAT 2 {\n    MR 0\n  }\n}\n").unwrap())
                .unwrap();

        let nested_region = top_level_repeat_region(nested_repeat.blocks.as_slice());

        assert!(!supports_reset_periodic_body(nested_region));
    }
}
