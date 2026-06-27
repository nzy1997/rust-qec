use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

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

pub fn mixed_noise_rotated_memory_x_d3_r3() -> Vec<StimInstr> {
    let mut noise = crate::codegen::NoiseParams::uniform(0.01);
    noise.after_clifford_loss_probability = 0.01;
    let base = crate::codegen::surface_code::rotated_memory_x_with_params(3, 3, noise);
    let mut out = Vec::with_capacity(base.len() + 1);
    let insertion_index = final_tick_before_first_mx_index(&base)
        .expect("rotated_memory_x(3, 3, 0.0) should contain a final TICK before MX");

    for (index, instr) in base.into_iter().enumerate() {
        out.push(instr);
        if index == insertion_index {
            out.push(noise_op("Z_ERROR", &[3, 13, 15]));
        }
    }

    out
}

fn final_tick_before_first_mx_index(instrs: &[StimInstr]) -> Option<usize> {
    let first_mx = instrs.iter().position(|instr| {
        matches!(
            instr,
            StimInstr::Op { name, .. } if name == "MX"
        )
    })?;
    instrs[..first_mx].iter().rposition(|instr| {
        matches!(
            instr,
            StimInstr::Op { name, .. } if name == "TICK"
        )
    })
}

fn noise_op(name: &str, qubits: &[u32]) -> StimInstr {
    StimInstr::Op {
        name: name.to_string(),
        tag: None,
        args: vec![0.01],
        targets: qubits.iter().copied().map(StimTarget::Qubit).collect(),
    }
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

#[derive(Debug, Clone)]
pub struct CircuitSummary {
    pub qubits: usize,
    pub opcode_counts: BTreeMap<String, usize>,
    pub measurements: usize,
    pub detectors: usize,
    pub observables: usize,
    pub detector_target_arities: BTreeMap<usize, usize>,
    pub observable_target_arities: BTreeMap<usize, usize>,
    pub qubit_coords: BTreeSet<String>,
    pub detector_annotations: BTreeSet<String>,
    pub observable_includes: BTreeSet<String>,
}

impl PartialEq for CircuitSummary {
    fn eq(&self, other: &Self) -> bool {
        self.qubits == other.qubits
            && self.opcode_counts == other.opcode_counts
            && self.measurements == other.measurements
            && self.detectors == other.detectors
            && self.observables == other.observables
            && self.detector_target_arities == other.detector_target_arities
            && self.observable_target_arities == other.observable_target_arities
    }
}

impl Eq for CircuitSummary {}

pub fn structural_circuit_summary(instrs: &[StimInstr]) -> CircuitSummary {
    let mut summary = CircuitSummary {
        qubits: used_qubit_count(instrs),
        opcode_counts: BTreeMap::new(),
        measurements: stats::num_measurements(instrs),
        detectors: stats::num_detectors(instrs),
        observables: stats::num_observables(instrs),
        detector_target_arities: BTreeMap::new(),
        observable_target_arities: BTreeMap::new(),
        qubit_coords: BTreeSet::new(),
        detector_annotations: BTreeSet::new(),
        observable_includes: BTreeSet::new(),
    };
    accumulate_instrs(instrs, &mut summary);
    summary
}

fn used_qubit_count(instrs: &[StimInstr]) -> usize {
    let mut qubits = BTreeSet::new();
    collect_qubits(instrs, &mut qubits);
    qubits.len()
}

fn collect_qubits(instrs: &[StimInstr], qubits: &mut BTreeSet<u32>) {
    for instr in instrs {
        match instr {
            StimInstr::Op { targets, .. } => {
                for target in targets {
                    if let Some(qubit) = target.qubit_index() {
                        qubits.insert(qubit);
                    }
                }
            }
            StimInstr::Repeat { body, .. } => collect_qubits(body, qubits),
        }
    }
}

fn accumulate_instrs(instrs: &[StimInstr], summary: &mut CircuitSummary) {
    for instr in instrs {
        match instr {
            StimInstr::Op {
                name,
                args,
                targets,
                ..
            } => match name.as_str() {
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
                    let arity = targets
                        .iter()
                        .filter(|target| matches!(target, StimTarget::Rec(_)))
                        .count();
                    *summary.detector_target_arities.entry(arity).or_default() += 1;
                    summary.detector_annotations.insert(format!(
                        "DETECTOR({}) {}",
                        format_args(args),
                        format_targets(targets)
                    ));
                }
                "OBSERVABLE_INCLUDE" => {
                    let arity = targets
                        .iter()
                        .filter(|target| matches!(target, StimTarget::Rec(_)))
                        .count();
                    *summary.observable_target_arities.entry(arity).or_default() += 1;
                    summary.observable_includes.insert(format!(
                        "OBSERVABLE_INCLUDE({}) {}",
                        format_args(args),
                        format_targets(targets)
                    ));
                }
                "MR" => {
                    add_opcode_units(&mut summary.opcode_counts, "M", targets.len());
                    add_opcode_units(&mut summary.opcode_counts, "R", targets.len());
                }
                "MRX" => {
                    add_opcode_units(&mut summary.opcode_counts, "MX", targets.len());
                    add_opcode_units(&mut summary.opcode_counts, "RX", targets.len());
                }
                "MRY" => {
                    add_opcode_units(&mut summary.opcode_counts, "MY", targets.len());
                    add_opcode_units(&mut summary.opcode_counts, "RY", targets.len());
                }
                "MRZ" => {
                    add_opcode_units(&mut summary.opcode_counts, "MZ", targets.len());
                    add_opcode_units(&mut summary.opcode_counts, "R", targets.len());
                }
                "TICK" | "SHIFT_COORDS" => {}
                _ => {
                    let units = normalized_opcode_units(name, targets);
                    add_opcode_units(&mut summary.opcode_counts, name, units);
                }
            },
            StimInstr::Repeat { count, body } => {
                for _ in 0..*count {
                    accumulate_instrs(body, summary);
                }
            }
        }
    }
}

fn add_opcode_units(opcode_counts: &mut BTreeMap<String, usize>, name: &str, units: usize) {
    if units > 0 {
        *opcode_counts.entry(name.to_string()).or_default() += units;
    }
}

fn normalized_opcode_units(name: &str, targets: &[StimTarget]) -> usize {
    match name {
        "QUBIT_COORDS" | "DETECTOR" | "OBSERVABLE_INCLUDE" => 0,
        "CX" | "CY" | "CZ" | "SWAP" | "ISWAP" | "ISWAP_DAG" | "XCX" | "XCY" | "XCZ" | "YCX"
        | "YCY" | "YCZ" | "MXX" | "MYY" | "MZZ" => targets.len() / 2,
        "MPP" => targets
            .split(|target| matches!(target, StimTarget::Combiner))
            .filter(|group| !group.is_empty())
            .count(),
        _ => {
            if targets.is_empty() {
                1
            } else {
                targets.len()
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

pub fn median_duration_ns(values: &[Duration]) -> u128 {
    let mut nanos: Vec<u128> = values.iter().map(Duration::as_nanos).collect();
    nanos.sort_unstable();
    nanos[nanos.len() / 2]
}

pub fn render_markdown_table(rows: &[Vec<String>]) -> String {
    let mut out = String::from(
        "| Case | Gen | DEM | Max Rel Error | Stim Gen ms | rstim Gen ms | Stim DEM ms | rstim DEM ms | Gen Ratio | DEM Ratio |\n",
    );
    out.push_str("| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |\n");
    for row in rows {
        out.push('|');
        out.push(' ');
        out.push_str(&row.join(" | "));
        out.push_str(" |\n");
    }
    out
}
