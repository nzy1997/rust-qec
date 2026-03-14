use std::collections::{BTreeMap, BTreeSet};

use crate::dem::{DemInstruction, DemTarget, DetectorErrorModel};
use crate::ir::{PauliBasis, StimInstr, StimTarget};
use crate::stats;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShowcaseCase {
    pub code: &'static str,
    pub task: &'static str,
    pub distance: usize,
    pub rounds: usize,
}

impl ShowcaseCase {
    pub fn label(&self) -> String {
        format!(
            "{}/{} d={} r={}",
            self.code, self.task, self.distance, self.rounds
        )
    }
}

pub fn showcase_cases() -> Vec<ShowcaseCase> {
    vec![
        ShowcaseCase {
            code: "repetition_code",
            task: "memory",
            distance: 5,
            rounds: 5,
        },
        ShowcaseCase {
            code: "repetition_code",
            task: "memory",
            distance: 13,
            rounds: 13,
        },
        ShowcaseCase {
            code: "surface_code",
            task: "rotated_memory_x",
            distance: 5,
            rounds: 5,
        },
        ShowcaseCase {
            code: "surface_code",
            task: "rotated_memory_x",
            distance: 13,
            rounds: 13,
        },
        ShowcaseCase {
            code: "surface_code",
            task: "rotated_memory_z",
            distance: 5,
            rounds: 5,
        },
        ShowcaseCase {
            code: "surface_code",
            task: "rotated_memory_z",
            distance: 13,
            rounds: 13,
        },
    ]
}

pub fn strip_comment_preamble(text: &str) -> &str {
    let mut offset = 0usize;
    for line in text.split_inclusive('\n') {
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            offset += line.len();
            continue;
        }
        break;
    }
    &text[offset..]
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CircuitSummary {
    pub opcode_counts: BTreeMap<String, usize>,
    pub measurements: usize,
    pub detectors: usize,
    pub observables: usize,
    pub qubit_coords: BTreeSet<String>,
    pub detector_annotations: BTreeSet<String>,
    pub observable_includes: BTreeSet<String>,
}

pub fn structural_circuit_summary(instrs: &[StimInstr]) -> CircuitSummary {
    let mut summary = CircuitSummary {
        opcode_counts: BTreeMap::new(),
        measurements: stats::num_measurements(instrs),
        detectors: stats::num_detectors(instrs),
        observables: stats::num_observables(instrs),
        qubit_coords: BTreeSet::new(),
        detector_annotations: BTreeSet::new(),
        observable_includes: BTreeSet::new(),
    };
    accumulate_instrs(instrs, &mut summary);
    summary
}

fn accumulate_instrs(instrs: &[StimInstr], summary: &mut CircuitSummary) {
    for instr in instrs {
        match instr {
            StimInstr::Op {
                name,
                args,
                targets,
                ..
            } => {
                *summary.opcode_counts.entry(name.clone()).or_default() += 1;
                match name.as_str() {
                    "QUBIT_COORDS" => {
                        if let Some(q) = targets.first().and_then(StimTarget::qubit_index) {
                            summary.qubit_coords.insert(format!(
                                "QUBIT_COORDS({}) {}",
                                format_args(args),
                                q
                            ));
                        }
                    }
                    "DETECTOR" => {
                        summary.detector_annotations.insert(format!(
                            "DETECTOR({}) {}",
                            format_args(args),
                            format_targets(targets)
                        ));
                    }
                    "OBSERVABLE_INCLUDE" => {
                        summary.observable_includes.insert(format!(
                            "OBSERVABLE_INCLUDE({}) {}",
                            format_args(args),
                            format_targets(targets)
                        ));
                    }
                    _ => {}
                }
            }
            StimInstr::Repeat { count, body } => {
                *summary
                    .opcode_counts
                    .entry("REPEAT".to_string())
                    .or_default() += 1;
                for _ in 0..*count {
                    accumulate_instrs(body, summary);
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DemSummary {
    pub error_probabilities: BTreeMap<String, f64>,
    pub annotation_lines: Vec<String>,
}

pub fn dem_semantic_summary(dem: &DetectorErrorModel) -> DemSummary {
    let mut summary = DemSummary {
        error_probabilities: BTreeMap::new(),
        annotation_lines: Vec::new(),
    };
    accumulate_dem(dem.instructions(), 0, &mut summary);
    summary.annotation_lines.sort();
    summary
}

fn accumulate_dem(
    instrs: &[DemInstruction],
    detector_offset: usize,
    summary: &mut DemSummary,
) -> usize {
    let mut offset = detector_offset;
    for instr in instrs {
        match instr {
            DemInstruction::Error {
                probability,
                targets,
            } => {
                summary
                    .error_probabilities
                    .insert(format_dem_targets(targets, offset), *probability);
            }
            DemInstruction::Detector { index, coords } => {
                summary.annotation_lines.push(format!(
                    "detector({}) D{}",
                    format_args(coords),
                    index + offset
                ));
            }
            DemInstruction::LogicalObservable { index } => {
                summary
                    .annotation_lines
                    .push(format!("logical_observable L{}", index));
            }
            DemInstruction::ShiftDetectors {
                detector_offset,
                coord_offsets,
            } => {
                summary.annotation_lines.push(format!(
                    "shift_detectors({}) {}",
                    format_args(coord_offsets),
                    detector_offset
                ));
                offset += detector_offset;
            }
            DemInstruction::Repeat { count, body } => {
                for _ in 0..*count {
                    offset = accumulate_dem(body.instructions(), offset, summary);
                }
            }
        }
    }
    offset
}

fn format_dem_targets(targets: &[DemTarget], detector_offset: usize) -> String {
    let mut out = Vec::with_capacity(targets.len());
    for target in targets {
        out.push(match target {
            DemTarget::Detector(index) => format!("D{}", index + detector_offset),
            DemTarget::Observable(index) => format!("L{}", index),
            DemTarget::Separator => "^".to_string(),
        });
    }
    out.join(" ")
}

fn format_targets(targets: &[StimTarget]) -> String {
    let mut out = Vec::with_capacity(targets.len());
    for target in targets {
        out.push(match target {
            StimTarget::Qubit(q) => q.to_string(),
            StimTarget::QubitInv(q) => format!("!{}", q),
            StimTarget::Rec(r) => format!("rec[{r}]"),
            StimTarget::Pauli {
                qubit,
                basis,
                inverted,
            } => {
                let prefix = if *inverted { "!" } else { "" };
                let basis = match basis {
                    PauliBasis::X => "X",
                    PauliBasis::Y => "Y",
                    PauliBasis::Z => "Z",
                };
                format!("{prefix}{basis}{qubit}")
            }
            StimTarget::Combiner => "*".to_string(),
            StimTarget::Sweep(k) => format!("sweep[{k}]"),
        });
    }
    out.join(" ")
}

fn format_args(args: &[f64]) -> String {
    let mut parts = Vec::with_capacity(args.len());
    for arg in args {
        if *arg == (*arg as i64) as f64 {
            parts.push((*arg as i64).to_string());
        } else {
            parts.push(arg.to_string());
        }
    }
    parts.join(",")
}
