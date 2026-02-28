use crate::dem::{DemInstruction, DemTarget, DetectorErrorModel};

#[derive(Debug, Clone)]
pub struct ExplainedError {
    pub probability: f64,
    pub detectors: Vec<usize>,
    pub observables: Vec<usize>,
}

/// Find error mechanisms in `dem` that together explain the `fired` detectors.
/// Uses a greedy approach: repeatedly pick the error mechanism that covers
/// the most currently-unexplained fired detectors.
pub fn explain(dem: &DetectorErrorModel, fired: &[usize]) -> Vec<ExplainedError> {
    if fired.is_empty() {
        return vec![];
    }

    let errors = collect_errors(dem);
    let mut remaining: std::collections::HashSet<usize> = fired.iter().copied().collect();
    let mut result = Vec::new();

    while !remaining.is_empty() {
        let best = errors.iter().max_by_key(|e| {
            e.detectors.iter().filter(|d| remaining.contains(d)).count()
        });

        match best {
            None => break,
            Some(e) => {
                let covered = e.detectors.iter().filter(|d| remaining.contains(d)).count();
                if covered == 0 { break; }
                for d in &e.detectors {
                    remaining.remove(d);
                }
                result.push(e.clone());
            }
        }
    }

    result
}

fn collect_errors(dem: &DetectorErrorModel) -> Vec<ExplainedError> {
    let mut out = Vec::new();
    collect_errors_instrs(dem.instructions(), &mut out, 0);
    out
}

fn collect_errors_instrs(instrs: &[DemInstruction], out: &mut Vec<ExplainedError>, det_offset: usize) {
    for instr in instrs {
        match instr {
            DemInstruction::Error { probability, targets } => {
                let mut detectors = Vec::new();
                let mut observables = Vec::new();
                for t in targets {
                    match t {
                        DemTarget::Detector(d) => detectors.push(d + det_offset),
                        DemTarget::Observable(o) => observables.push(*o),
                        DemTarget::Separator => {}
                    }
                }
                out.push(ExplainedError { probability: *probability, detectors, observables });
            }
            DemInstruction::Repeat { count, body } => {
                let n_dets = body.num_detectors();
                let mut offset = det_offset;
                for _ in 0..*count {
                    collect_errors_instrs(body.instructions(), out, offset);
                    offset += n_dets;
                }
            }
            _ => {}
        }
    }
}
