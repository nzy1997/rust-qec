use rand::rngs::StdRng;
use rand::{Error as RandError, RngCore, SeedableRng};
use rstim::ir::{StimInstr, StimTarget};
use rstim::parser::parse_lines;
use rstim::sampler::{SampleOptions, SampleOutputMode, SamplingBackend, sample_batch_with_options};
use rstim::sim::tableau::StabilizerState;

fn assert_sampled_outputs_are_possible(stim: &str, shots: usize) {
    let instrs = parse_lines(stim).unwrap();
    let mut rng = StdRng::seed_from_u64(0x431);
    let out = sample_batch_with_options(
        &instrs,
        shots,
        &mut rng,
        SampleOptions {
            backend: SamplingBackend::Interpreted,
            output_mode: SampleOutputMode::MeasurementsOnly,
            ..SampleOptions::default()
        },
    )
    .unwrap();

    for shot in 0..shots {
        let row: Vec<bool> = (0..out.measurements.num_major())
            .map(|m| out.measurements.get(m, shot))
            .collect();
        assert!(
            is_output_possible(&instrs, &row).unwrap(),
            "shot {shot} produced impossible row {row:?}"
        );
    }
}

fn is_output_possible(instrs: &[StimInstr], row: &[bool]) -> Result<bool, String> {
    let mut replay = PossibleOutputReplay::new(instrs, row);
    if !replay.run_instrs(instrs)? {
        return Ok(false);
    }
    Ok(replay.next_measurement == row.len())
}

#[test]
fn sampled_outputs_are_possible_for_entangling_circuits() {
    for stim in [
        "H 0\nCNOT 0 1\nM 0 1\n",
        "H 0\nCNOT 0 1\nMR 0\nM 1\n",
        "RX 0 1\nH 0\nCNOT 0 1\nMRX 0\nMX 1\n",
        "R 0 1 2\nR 3 4\nTICK\nH 0 1 2\nCNOT 0 3 1 4 1 3 2 4\nMR 3 4\nDETECTOR rec[-2]\nDETECTOR rec[-1]\nTICK\nM 0 1 2\nDETECTOR rec[-3] rec[-2] rec[-5]\nDETECTOR rec[-2] rec[-1] rec[-4]\nOBSERVABLE_INCLUDE(0) rec[-3]\n",
    ] {
        assert_sampled_outputs_are_possible(stim, 32);
    }
}

#[test]
fn impossible_output_is_rejected() {
    let instrs = parse_lines("H 0\nCNOT 0 1\nM 0\nM 1\n").unwrap();
    assert!(is_output_possible(&instrs, &[false, false]).unwrap());
    assert!(is_output_possible(&instrs, &[true, true]).unwrap());
    assert!(!is_output_possible(&instrs, &[false, true]).unwrap());
    assert!(!is_output_possible(&instrs, &[true, false]).unwrap());
}

#[test]
fn impossible_output_is_rejected_inside_repeat() {
    let instrs = parse_lines("REPEAT 1 {\n  H 0\n  CNOT 0 1\n  M 0\n  M 1\n}\n").unwrap();
    assert!(is_output_possible(&instrs, &[false, false]).unwrap());
    assert!(is_output_possible(&instrs, &[true, true]).unwrap());
    assert!(!is_output_possible(&instrs, &[false, true]).unwrap());
    assert!(!is_output_possible(&instrs, &[true, false]).unwrap());
}

#[derive(Clone, Copy)]
enum MeasureBasis {
    Z,
    X,
}

#[derive(Default)]
struct ForcedBitRng {
    bit: bool,
}

impl ForcedBitRng {
    fn force(&mut self, bit: bool) {
        self.bit = bit;
    }
}

impl RngCore for ForcedBitRng {
    fn next_u32(&mut self) -> u32 {
        if self.bit { u32::MAX } else { 0 }
    }

    fn next_u64(&mut self) -> u64 {
        if self.bit { u64::MAX } else { 0 }
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        dest.fill(if self.bit { u8::MAX } else { 0 });
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), RandError> {
        self.fill_bytes(dest);
        Ok(())
    }
}

struct PossibleOutputReplay<'a> {
    state: StabilizerState,
    row: &'a [bool],
    next_measurement: usize,
    rng: ForcedBitRng,
}

impl<'a> PossibleOutputReplay<'a> {
    fn new(instrs: &[StimInstr], row: &'a [bool]) -> Self {
        Self {
            state: StabilizerState::new(required_qubits(instrs)),
            row,
            next_measurement: 0,
            rng: ForcedBitRng::default(),
        }
    }

    fn run_instrs(&mut self, instrs: &[StimInstr]) -> Result<bool, String> {
        for instr in instrs {
            if !self.run_instr(instr)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn run_instr(&mut self, instr: &StimInstr) -> Result<bool, String> {
        match instr {
            StimInstr::Op { name, targets, .. } => self.run_op(name, targets),
            StimInstr::Repeat { count, body } => {
                for _ in 0..*count {
                    if !self.run_instrs(body)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
        }
    }

    fn run_op(&mut self, name: &str, targets: &[StimTarget]) -> Result<bool, String> {
        match name {
            "I" | "TICK" | "QUBIT_COORDS" | "SHIFT_COORDS" | "DETECTOR" | "OBSERVABLE_INCLUDE" => {
                Ok(true)
            }
            "H" => {
                for q in qubits(targets)? {
                    self.state.h(q);
                }
                Ok(true)
            }
            "X" => {
                for q in qubits(targets)? {
                    self.state.x_gate(q);
                }
                Ok(true)
            }
            "Z" => {
                for q in qubits(targets)? {
                    self.state.z_gate(q);
                }
                Ok(true)
            }
            "CX" | "CNOT" => {
                for (control, target) in qubit_pairs(targets)? {
                    self.state.cx(control, target);
                }
                Ok(true)
            }
            "CZ" => {
                for (a, b) in qubit_pairs(targets)? {
                    self.state.cz(a, b);
                }
                Ok(true)
            }
            "R" | "RZ" => {
                for q in qubits(targets)? {
                    self.state.reset_z_biased(q);
                }
                Ok(true)
            }
            "RX" => {
                for q in qubits(targets)? {
                    self.state.reset_x_biased(q);
                }
                Ok(true)
            }
            "M" | "MZ" => self.measure_each(targets, MeasureBasis::Z, false),
            "MX" => self.measure_each(targets, MeasureBasis::X, false),
            "MR" | "MRZ" => self.measure_each(targets, MeasureBasis::Z, true),
            "MRX" => self.measure_each(targets, MeasureBasis::X, true),
            other => Err(format!(
                "unsupported instruction in possible-output helper: {other}"
            )),
        }
    }

    fn measure_each(
        &mut self,
        targets: &[StimTarget],
        basis: MeasureBasis,
        reset: bool,
    ) -> Result<bool, String> {
        for (q, inverted) in measurement_targets(targets)? {
            let Some(&row_bit) = self.row.get(self.next_measurement) else {
                return Ok(false);
            };
            self.next_measurement += 1;
            let candidate_raw = row_bit ^ inverted;
            let possible = match basis {
                MeasureBasis::Z => self.measure_z_candidate(q, candidate_raw, reset)?,
                MeasureBasis::X => {
                    self.state.h(q);
                    let possible = self.measure_z_candidate(q, candidate_raw, reset)?;
                    self.state.h(q);
                    possible
                }
            };
            if !possible {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn measure_z_candidate(
        &mut self,
        q: usize,
        candidate_raw: bool,
        reset: bool,
    ) -> Result<bool, String> {
        self.rng.force(candidate_raw);
        let (measured_raw, was_random) = self.state.measure_z(q, &mut self.rng);
        let measured_raw = measured_raw == 1;
        if !was_random && measured_raw != candidate_raw {
            return Ok(false);
        }
        if reset && measured_raw {
            self.state.x_gate(q);
        }
        Ok(true)
    }
}

fn required_qubits(instrs: &[StimInstr]) -> usize {
    instrs
        .iter()
        .map(|instr| match instr {
            StimInstr::Op { targets, .. } => targets
                .iter()
                .filter_map(StimTarget::qubit_index)
                .max()
                .map(|q| q as usize + 1)
                .unwrap_or(0),
            StimInstr::Repeat { body, .. } => required_qubits(body),
        })
        .max()
        .unwrap_or(0)
}

fn qubits(targets: &[StimTarget]) -> Result<Vec<usize>, String> {
    let mut out = Vec::new();
    for target in targets {
        match target {
            StimTarget::Qubit(q) => out.push(*q as usize),
            StimTarget::Sweep(_) => {}
            StimTarget::QubitInv(_) => {
                return Err("inverted qubit target only valid for measurement".to_string());
            }
            _ => return Err("expected qubit target".to_string()),
        }
    }
    Ok(out)
}

fn measurement_targets(targets: &[StimTarget]) -> Result<Vec<(usize, bool)>, String> {
    let mut out = Vec::new();
    for target in targets {
        match target {
            StimTarget::Qubit(q) => out.push((*q as usize, false)),
            StimTarget::QubitInv(q) => out.push((*q as usize, true)),
            _ => return Err("expected measurement qubit target".to_string()),
        }
    }
    Ok(out)
}

fn qubit_pairs(targets: &[StimTarget]) -> Result<Vec<(usize, usize)>, String> {
    if targets.len() % 2 != 0 {
        return Err("odd number of targets".to_string());
    }
    let mut out = Vec::new();
    let mut iter = targets.iter();
    while let (Some(a), Some(b)) = (iter.next(), iter.next()) {
        if matches!(a, StimTarget::Sweep(_)) || matches!(b, StimTarget::Sweep(_)) {
            continue;
        }
        out.push((gate_qubit(a)?, gate_qubit(b)?));
    }
    Ok(out)
}

fn gate_qubit(target: &StimTarget) -> Result<usize, String> {
    match target {
        StimTarget::Qubit(q) => Ok(*q as usize),
        StimTarget::QubitInv(_) => {
            Err("inverted qubit target only valid for measurement".to_string())
        }
        StimTarget::Sweep(_) => Err("sweep[] target unexpected here".to_string()),
        _ => Err("expected qubit target".to_string()),
    }
}
