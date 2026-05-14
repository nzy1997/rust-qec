use rand::Rng;

use crate::coords::CoordState;
use crate::ir::{PauliBasis, StimInstr, StimTarget};
use crate::recorder::Recorder;
use crate::sample_trace::{
    DetectorEvent, MeasurementComponent, MeasurementEvent, NoiseEvent, SampleTrace,
};
use crate::sim::tableau::StabilizerState;

pub struct Executor {
    instrs: Vec<StimInstr>,
}

pub struct ExecOutput {
    pub measurements: Vec<bool>,
    pub detectors: Vec<bool>,
    pub detector_coords: Vec<Vec<f64>>,
    pub observables: Vec<(u32, bool)>,
    pub qubit_coords: std::collections::HashMap<u32, Vec<f64>>,
}

impl Executor {
    pub fn from_instrs(instrs: Vec<StimInstr>) -> Result<Self, String> {
        Ok(Self { instrs })
    }

    pub fn run(&mut self, rng: &mut impl Rng) -> Result<ExecOutput, String> {
        let (out, _) = self.run_internal(rng, false)?;
        Ok(out)
    }

    pub fn run_with_trace(
        &mut self,
        rng: &mut impl Rng,
    ) -> Result<(ExecOutput, SampleTrace), String> {
        self.run_internal(rng, true)
    }

    fn run_internal(
        &self,
        rng: &mut impl Rng,
        trace_enabled: bool,
    ) -> Result<(ExecOutput, SampleTrace), String> {
        let n = max_qubit(&self.instrs)?;
        let mut exec = ExecutionState::new(n, trace_enabled);
        execute_instrs(
            &self.instrs,
            &ExecutionTraversalContext::default(),
            &mut exec,
            rng,
        )?;
        Ok(exec.into_output())
    }
}

#[derive(Default, Debug, Clone)]
struct ExecutionTraversalContext {
    op_path: Vec<usize>,
    repeat_iterations: Vec<u64>,
}

impl ExecutionTraversalContext {
    fn with_op_index(&self, op_index: usize) -> Self {
        let mut op_path = self.op_path.clone();
        op_path.push(op_index);
        Self {
            op_path,
            repeat_iterations: self.repeat_iterations.clone(),
        }
    }

    fn with_repeat_iteration(&self, repeat_op_index: usize, iteration: u64) -> Self {
        let mut op_path = self.op_path.clone();
        op_path.push(repeat_op_index);
        let mut repeat_iterations = self.repeat_iterations.clone();
        repeat_iterations.push(iteration);
        Self {
            op_path,
            repeat_iterations,
        }
    }
}

struct ExecutionState {
    state: StabilizerState,
    lost: Vec<bool>,
    recorder: Recorder,
    detectors: Vec<bool>,
    detector_coords: Vec<Vec<f64>>,
    observables: Vec<(u32, bool)>,
    coords: CoordState,
    last_correlated_error_occurred: bool,
    trace_enabled: bool,
    trace: SampleTrace,
}

impl ExecutionState {
    fn new(num_qubits: usize, trace_enabled: bool) -> Self {
        Self {
            state: StabilizerState::new(num_qubits),
            lost: vec![false; num_qubits],
            recorder: Recorder::default(),
            detectors: Vec::new(),
            detector_coords: Vec::new(),
            observables: Vec::new(),
            coords: CoordState::default(),
            last_correlated_error_occurred: false,
            trace_enabled,
            trace: SampleTrace {
                noise_events: Vec::new(),
                measurement_events: Vec::new(),
                detector_events: Vec::new(),
            },
        }
    }

    fn into_output(self) -> (ExecOutput, SampleTrace) {
        (
            ExecOutput {
                measurements: recorder_bits(self.recorder),
                detectors: self.detectors,
                detector_coords: self.detector_coords,
                observables: self.observables,
                qubit_coords: self.coords.qubit_coords,
            },
            self.trace,
        )
    }

    fn record_noise_event(
        &mut self,
        context: &ExecutionTraversalContext,
        instr_name: &str,
        target_slots: Vec<usize>,
        target_qubits: Vec<u32>,
        branch_label: &str,
    ) {
        if !self.trace_enabled {
            return;
        }
        self.trace.noise_events.push(NoiseEvent {
            op_path: context.op_path.clone(),
            repeat_iterations: context.repeat_iterations.clone(),
            instr_name: instr_name.to_string(),
            target_slots,
            target_qubits,
            occurred: true,
            branch_label: Some(branch_label.to_string()),
        });
    }

    fn record_measurement_event(
        &mut self,
        context: &ExecutionTraversalContext,
        instr_name: &str,
        target_slot: usize,
        target_qubit: u32,
        measurement_index: usize,
        outcome: MeasurementOutcome,
        component: MeasurementComponent,
    ) {
        if !self.trace_enabled {
            return;
        }
        self.trace.measurement_events.push(MeasurementEvent {
            op_path: context.op_path.clone(),
            repeat_iterations: context.repeat_iterations.clone(),
            target_slot,
            target_qubit,
            instr_name: instr_name.to_string(),
            measurement_index,
            bit: outcome.bit,
            loss_cause: outcome.loss_cause,
            component,
        });
    }

    fn record_detector_event(
        &mut self,
        context: &ExecutionTraversalContext,
        detector_index: usize,
        flipped: bool,
    ) {
        if !self.trace_enabled {
            return;
        }
        self.trace.detector_events.push(DetectorEvent {
            op_path: context.op_path.clone(),
            repeat_iterations: context.repeat_iterations.clone(),
            detector_index,
            flipped,
        });
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MeasurementOutcome {
    bit: bool,
    loss_cause: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LossVisibleMeasurementOutcome {
    loss_flag: bool,
    value: MeasurementOutcome,
}

fn execute_instrs<R: Rng>(
    instrs: &[StimInstr],
    context: &ExecutionTraversalContext,
    exec: &mut ExecutionState,
    rng: &mut R,
) -> Result<(), String> {
    for (op_index, instr) in instrs.iter().enumerate() {
        match instr {
            StimInstr::Op {
                name,
                args,
                targets,
                ..
            } => {
                let op_context = context.with_op_index(op_index);
                execute_op(name, args, targets, &op_context, exec, rng)?;
            }
            StimInstr::Repeat { count, body } => {
                for iteration in 0..*count {
                    let repeat_context = context.with_repeat_iteration(op_index, iteration);
                    execute_instrs(body, &repeat_context, exec, rng)?;
                }
            }
        }
    }
    Ok(())
}

fn execute_op<R: Rng>(
    name: &str,
    args: &[f64],
    targets: &[StimTarget],
    context: &ExecutionTraversalContext,
    exec: &mut ExecutionState,
    rng: &mut R,
) -> Result<(), String> {
    match name {
        "I" | "I_ERROR" | "II_ERROR" => {}
        "H" => {
            let lost = &exec.lost;
            let state = &mut exec.state;
            for_each_present_qubit(targets, lost, |q| state.h(q))?;
        }
        "H_XY" => {
            let lost = &exec.lost;
            let state = &mut exec.state;
            for_each_present_qubit(targets, lost, |q| state.h_xy(q))?;
        }
        "H_YZ" => {
            let lost = &exec.lost;
            let state = &mut exec.state;
            for_each_present_qubit(targets, lost, |q| state.h_yz(q))?;
        }
        "S" | "SQRT_Z" => {
            let lost = &exec.lost;
            let state = &mut exec.state;
            for_each_present_qubit(targets, lost, |q| state.s(q))?;
        }
        "S_DAG" | "SQRT_Z_DAG" => {
            let lost = &exec.lost;
            let state = &mut exec.state;
            for_each_present_qubit(targets, lost, |q| state.s_dag(q))?;
        }
        "SQRT_X" => {
            let lost = &exec.lost;
            let state = &mut exec.state;
            for_each_present_qubit(targets, lost, |q| state.sqrt_x(q))?;
        }
        "SQRT_X_DAG" => {
            let lost = &exec.lost;
            let state = &mut exec.state;
            for_each_present_qubit(targets, lost, |q| state.sqrt_x_dag(q))?;
        }
        "SQRT_Y" => {
            let lost = &exec.lost;
            let state = &mut exec.state;
            for_each_present_qubit(targets, lost, |q| state.sqrt_y(q))?;
        }
        "SQRT_Y_DAG" => {
            let lost = &exec.lost;
            let state = &mut exec.state;
            for_each_present_qubit(targets, lost, |q| state.sqrt_y_dag(q))?;
        }
        "X" => {
            let lost = &exec.lost;
            let state = &mut exec.state;
            for_each_present_qubit(targets, lost, |q| state.x_gate(q))?;
        }
        "Y" => {
            let lost = &exec.lost;
            let state = &mut exec.state;
            for_each_present_qubit(targets, lost, |q| state.y_gate(q))?;
        }
        "Z" => {
            let lost = &exec.lost;
            let state = &mut exec.state;
            for_each_present_qubit(targets, lost, |q| state.z_gate(q))?;
        }
        "C_XYZ" => {
            let lost = &exec.lost;
            let state = &mut exec.state;
            for_each_present_qubit(targets, lost, |q| state.c_xyz(q))?;
        }
        "C_ZYX" => {
            let lost = &exec.lost;
            let state = &mut exec.state;
            for_each_present_qubit(targets, lost, |q| state.c_zyx(q))?;
        }
        "C_NXYZ" => {
            let lost = &exec.lost;
            let state = &mut exec.state;
            for_each_present_qubit(targets, lost, |q| state.c_nxyz(q))?;
        }
        "C_NZYX" => {
            let lost = &exec.lost;
            let state = &mut exec.state;
            for_each_present_qubit(targets, lost, |q| state.c_nzyx(q))?;
        }
        "C_XNYZ" => {
            let lost = &exec.lost;
            let state = &mut exec.state;
            for_each_present_qubit(targets, lost, |q| state.c_xnyz(q))?;
        }
        "C_XYNZ" => {
            let lost = &exec.lost;
            let state = &mut exec.state;
            for_each_present_qubit(targets, lost, |q| state.c_xynz(q))?;
        }
        "C_ZNYX" => {
            let lost = &exec.lost;
            let state = &mut exec.state;
            for_each_present_qubit(targets, lost, |q| state.c_znyx(q))?;
        }
        "C_ZYNX" => {
            let lost = &exec.lost;
            let state = &mut exec.state;
            for_each_present_qubit(targets, lost, |q| state.c_zynx(q))?;
        }
        "H_NXY" => {
            let lost = &exec.lost;
            let state = &mut exec.state;
            for_each_present_qubit(targets, lost, |q| state.h_nxy(q))?;
        }
        "H_NXZ" => {
            let lost = &exec.lost;
            let state = &mut exec.state;
            for_each_present_qubit(targets, lost, |q| state.h_nxz(q))?;
        }
        "H_NYZ" => {
            let lost = &exec.lost;
            let state = &mut exec.state;
            for_each_present_qubit(targets, lost, |q| state.h_nyz(q))?;
        }
        "CX" | "CNOT" | "ZCX" => {
            let pairs = present_qubit_pairs(targets, &exec.lost)?;
            for (c, t) in pairs {
                exec.state.cx(c, t);
            }
        }
        "CY" | "ZCY" => {
            let pairs = present_qubit_pairs(targets, &exec.lost)?;
            for (c, t) in pairs {
                exec.state.cy(c, t);
            }
        }
        "CZ" | "ZCZ" => {
            let pairs = present_qubit_pairs(targets, &exec.lost)?;
            for (c, t) in pairs {
                exec.state.cz(c, t);
            }
        }
        "XCX" => {
            let pairs = present_qubit_pairs(targets, &exec.lost)?;
            for (a, b) in pairs {
                exec.state.xcx(a, b);
            }
        }
        "XCY" => {
            let pairs = present_qubit_pairs(targets, &exec.lost)?;
            for (a, b) in pairs {
                exec.state.xcy(a, b);
            }
        }
        "XCZ" => {
            let pairs = present_qubit_pairs(targets, &exec.lost)?;
            for (a, b) in pairs {
                exec.state.xcz(a, b);
            }
        }
        "YCX" => {
            let pairs = present_qubit_pairs(targets, &exec.lost)?;
            for (a, b) in pairs {
                exec.state.ycx(a, b);
            }
        }
        "YCY" => {
            let pairs = present_qubit_pairs(targets, &exec.lost)?;
            for (a, b) in pairs {
                exec.state.ycy(a, b);
            }
        }
        "YCZ" => {
            let pairs = present_qubit_pairs(targets, &exec.lost)?;
            for (a, b) in pairs {
                exec.state.ycz(a, b);
            }
        }
        "SWAP" => {
            let pairs = present_qubit_pairs(targets, &exec.lost)?;
            for (a, b) in pairs {
                exec.state.swap(a, b);
            }
        }
        "ISWAP" => {
            let pairs = present_qubit_pairs(targets, &exec.lost)?;
            for (a, b) in pairs {
                exec.state.iswap(a, b);
            }
        }
        "ISWAP_DAG" => {
            let pairs = present_qubit_pairs(targets, &exec.lost)?;
            for (a, b) in pairs {
                exec.state.iswap_dag(a, b);
            }
        }
        "CXSWAP" => {
            let pairs = present_qubit_pairs(targets, &exec.lost)?;
            for (a, b) in pairs {
                exec.state.cxswap(a, b);
            }
        }
        "SWAPCX" => {
            let pairs = present_qubit_pairs(targets, &exec.lost)?;
            for (a, b) in pairs {
                exec.state.swapcx(a, b);
            }
        }
        "CZSWAP" => {
            let pairs = present_qubit_pairs(targets, &exec.lost)?;
            for (a, b) in pairs {
                exec.state.czswap(a, b);
            }
        }
        "M" | "MZ" => {
            for (target_slot, q, inv) in qubits_with_inversion_slots(targets)? {
                let outcome =
                    record_measure_z(&mut exec.state, &exec.lost, q, inv, rng, &mut exec.recorder);
                exec.record_measurement_event(
                    context,
                    name,
                    target_slot,
                    q as u32,
                    exec.recorder.len(),
                    outcome,
                    MeasurementComponent::Value,
                );
            }
        }
        "MX" => {
            for (target_slot, q, inv) in qubits_with_inversion_slots(targets)? {
                let outcome =
                    record_measure_x(&mut exec.state, &exec.lost, q, inv, rng, &mut exec.recorder);
                exec.record_measurement_event(
                    context,
                    name,
                    target_slot,
                    q as u32,
                    exec.recorder.len(),
                    outcome,
                    MeasurementComponent::Value,
                );
            }
        }
        "MY" => {
            for (target_slot, q, inv) in qubits_with_inversion_slots(targets)? {
                let outcome =
                    record_measure_y(&mut exec.state, &exec.lost, q, inv, rng, &mut exec.recorder);
                exec.record_measurement_event(
                    context,
                    name,
                    target_slot,
                    q as u32,
                    exec.recorder.len(),
                    outcome,
                    MeasurementComponent::Value,
                );
            }
        }
        "MR" | "MRZ" => {
            for (target_slot, q, inv) in qubits_with_inversion_slots(targets)? {
                let outcome = measure_reset_z(
                    &mut exec.state,
                    &mut exec.lost,
                    q,
                    inv,
                    rng,
                    &mut exec.recorder,
                );
                exec.record_measurement_event(
                    context,
                    name,
                    target_slot,
                    q as u32,
                    exec.recorder.len(),
                    outcome,
                    MeasurementComponent::Value,
                );
            }
        }
        "MRX" => {
            for (target_slot, q, inv) in qubits_with_inversion_slots(targets)? {
                let outcome = measure_reset_x(
                    &mut exec.state,
                    &mut exec.lost,
                    q,
                    inv,
                    rng,
                    &mut exec.recorder,
                );
                exec.record_measurement_event(
                    context,
                    name,
                    target_slot,
                    q as u32,
                    exec.recorder.len(),
                    outcome,
                    MeasurementComponent::Value,
                );
            }
        }
        "MRY" => {
            for (target_slot, q, inv) in qubits_with_inversion_slots(targets)? {
                let outcome = measure_reset_y(
                    &mut exec.state,
                    &mut exec.lost,
                    q,
                    inv,
                    rng,
                    &mut exec.recorder,
                );
                exec.record_measurement_event(
                    context,
                    name,
                    target_slot,
                    q as u32,
                    exec.recorder.len(),
                    outcome,
                    MeasurementComponent::Value,
                );
            }
        }
        "ML" | "MZL" => {
            for (target_slot, q, inv) in qubits_with_inversion_slots(targets)? {
                let base_index = exec.recorder.len();
                let outcome = record_loss_visible_measure_z(
                    &mut exec.state,
                    &exec.lost,
                    q,
                    inv,
                    rng,
                    &mut exec.recorder,
                );
                exec.record_measurement_event(
                    context,
                    name,
                    target_slot,
                    q as u32,
                    base_index + 1,
                    MeasurementOutcome {
                        bit: outcome.loss_flag,
                        loss_cause: false,
                    },
                    MeasurementComponent::LossFlag,
                );
                exec.record_measurement_event(
                    context,
                    name,
                    target_slot,
                    q as u32,
                    base_index + 2,
                    outcome.value,
                    MeasurementComponent::Value,
                );
            }
        }
        "MXL" => {
            for (target_slot, q, inv) in qubits_with_inversion_slots(targets)? {
                let base_index = exec.recorder.len();
                let outcome = record_loss_visible_measure_x(
                    &mut exec.state,
                    &exec.lost,
                    q,
                    inv,
                    rng,
                    &mut exec.recorder,
                );
                exec.record_measurement_event(
                    context,
                    name,
                    target_slot,
                    q as u32,
                    base_index + 1,
                    MeasurementOutcome {
                        bit: outcome.loss_flag,
                        loss_cause: false,
                    },
                    MeasurementComponent::LossFlag,
                );
                exec.record_measurement_event(
                    context,
                    name,
                    target_slot,
                    q as u32,
                    base_index + 2,
                    outcome.value,
                    MeasurementComponent::Value,
                );
            }
        }
        "MYL" => {
            for (target_slot, q, inv) in qubits_with_inversion_slots(targets)? {
                let base_index = exec.recorder.len();
                let outcome = record_loss_visible_measure_y(
                    &mut exec.state,
                    &exec.lost,
                    q,
                    inv,
                    rng,
                    &mut exec.recorder,
                );
                exec.record_measurement_event(
                    context,
                    name,
                    target_slot,
                    q as u32,
                    base_index + 1,
                    MeasurementOutcome {
                        bit: outcome.loss_flag,
                        loss_cause: false,
                    },
                    MeasurementComponent::LossFlag,
                );
                exec.record_measurement_event(
                    context,
                    name,
                    target_slot,
                    q as u32,
                    base_index + 2,
                    outcome.value,
                    MeasurementComponent::Value,
                );
            }
        }
        "MRL" | "MRZL" => {
            for (target_slot, q, inv) in qubits_with_inversion_slots(targets)? {
                let base_index = exec.recorder.len();
                let outcome = measure_reset_loss_visible_z(
                    &mut exec.state,
                    &mut exec.lost,
                    q,
                    inv,
                    rng,
                    &mut exec.recorder,
                );
                exec.record_measurement_event(
                    context,
                    name,
                    target_slot,
                    q as u32,
                    base_index + 1,
                    MeasurementOutcome {
                        bit: outcome.loss_flag,
                        loss_cause: false,
                    },
                    MeasurementComponent::LossFlag,
                );
                exec.record_measurement_event(
                    context,
                    name,
                    target_slot,
                    q as u32,
                    base_index + 2,
                    outcome.value,
                    MeasurementComponent::Value,
                );
            }
        }
        "MRXL" => {
            for (target_slot, q, inv) in qubits_with_inversion_slots(targets)? {
                let base_index = exec.recorder.len();
                let outcome = measure_reset_loss_visible_x(
                    &mut exec.state,
                    &mut exec.lost,
                    q,
                    inv,
                    rng,
                    &mut exec.recorder,
                );
                exec.record_measurement_event(
                    context,
                    name,
                    target_slot,
                    q as u32,
                    base_index + 1,
                    MeasurementOutcome {
                        bit: outcome.loss_flag,
                        loss_cause: false,
                    },
                    MeasurementComponent::LossFlag,
                );
                exec.record_measurement_event(
                    context,
                    name,
                    target_slot,
                    q as u32,
                    base_index + 2,
                    outcome.value,
                    MeasurementComponent::Value,
                );
            }
        }
        "MRYL" => {
            for (target_slot, q, inv) in qubits_with_inversion_slots(targets)? {
                let base_index = exec.recorder.len();
                let outcome = measure_reset_loss_visible_y(
                    &mut exec.state,
                    &mut exec.lost,
                    q,
                    inv,
                    rng,
                    &mut exec.recorder,
                );
                exec.record_measurement_event(
                    context,
                    name,
                    target_slot,
                    q as u32,
                    base_index + 1,
                    MeasurementOutcome {
                        bit: outcome.loss_flag,
                        loss_cause: false,
                    },
                    MeasurementComponent::LossFlag,
                );
                exec.record_measurement_event(
                    context,
                    name,
                    target_slot,
                    q as u32,
                    base_index + 2,
                    outcome.value,
                    MeasurementComponent::Value,
                );
            }
        }
        "MPAD" => {
            let p = args.first().copied().unwrap_or(0.0);
            for t in targets {
                let q = expect_qubit(t)?;
                let mut bit = q != 0;
                if p > 0.0 && rng.r#gen::<f64>() < p {
                    bit = !bit;
                }
                exec.recorder.push(bit);
            }
        }
        "R" | "RZ" => {
            for q in qubits(targets)? {
                exec.lost[q] = false;
                exec.state.reset_z(q, rng);
            }
        }
        "RX" => {
            for q in qubits(targets)? {
                exec.lost[q] = false;
                exec.state.reset_x(q, rng);
            }
        }
        "RY" => {
            for q in qubits(targets)? {
                exec.lost[q] = false;
                exec.state.reset_y(q, rng);
            }
        }
        "LOSS" => {
            let p = args.first().copied().unwrap_or(0.0);
            for (target_slot, q) in qubit_slots(targets)? {
                if rng.r#gen::<f64>() < p {
                    exec.lost[q] = true;
                    exec.record_noise_event(context, name, vec![target_slot], vec![q as u32], "L");
                }
            }
        }
        "X_ERROR" => {
            let p = args.first().copied().unwrap_or(0.0);
            for (target_slot, q) in qubit_slots(targets)? {
                if !exec.lost[q] && rng.r#gen::<f64>() < p {
                    exec.state.x_gate(q);
                    exec.record_noise_event(context, name, vec![target_slot], vec![q as u32], "X");
                }
            }
        }
        "Y_ERROR" => {
            let p = args.first().copied().unwrap_or(0.0);
            for (target_slot, q) in qubit_slots(targets)? {
                if !exec.lost[q] && rng.r#gen::<f64>() < p {
                    exec.state.y_gate(q);
                    exec.record_noise_event(context, name, vec![target_slot], vec![q as u32], "Y");
                }
            }
        }
        "Z_ERROR" => {
            let p = args.first().copied().unwrap_or(0.0);
            for (target_slot, q) in qubit_slots(targets)? {
                if !exec.lost[q] && rng.r#gen::<f64>() < p {
                    exec.state.z_gate(q);
                    exec.record_noise_event(context, name, vec![target_slot], vec![q as u32], "Z");
                }
            }
        }
        "DEPOLARIZE1" => {
            let p = args.first().copied().unwrap_or(0.0);
            for (target_slot, q) in qubit_slots(targets)? {
                if !exec.lost[q] && rng.r#gen::<f64>() < p {
                    let branch = match rng.gen_range(0..3) {
                        0 => {
                            exec.state.x_gate(q);
                            "X"
                        }
                        1 => {
                            exec.state.y_gate(q);
                            "Y"
                        }
                        _ => {
                            exec.state.z_gate(q);
                            "Z"
                        }
                    };
                    exec.record_noise_event(
                        context,
                        name,
                        vec![target_slot],
                        vec![q as u32],
                        branch,
                    );
                }
            }
        }
        "DEPOLARIZE2" => {
            let p = args.first().copied().unwrap_or(0.0);
            for ((slot_a, a), (slot_b, b)) in present_qubit_pair_slots(targets, &exec.lost)? {
                if rng.r#gen::<f64>() < p {
                    let (pa, pb) = two_qubit_pauli(rng.gen_range(0..15));
                    apply_pauli(&mut exec.state, a, pa);
                    apply_pauli(&mut exec.state, b, pb);
                    let label = pauli_pair_label(pa, pb);
                    exec.record_noise_event(
                        context,
                        name,
                        vec![slot_a, slot_b],
                        vec![a as u32, b as u32],
                        &label,
                    );
                }
            }
        }
        "QUBIT_COORDS" => {
            let coords_vec = exec.coords.apply_offset(args);
            for t in targets {
                if let StimTarget::Qubit(q) = t {
                    exec.coords.qubit_coords.insert(*q, coords_vec.clone());
                } else {
                    return Err("QUBIT_COORDS expects qubit targets".to_string());
                }
            }
        }
        "SHIFT_COORDS" => {
            exec.coords.shift(args);
        }
        "TICK" => {
            exec.coords.tick += 1;
        }
        "DETECTOR" => {
            let bit = xor_recs(&exec.recorder, targets)?;
            let detector_index = exec.detectors.len();
            exec.detectors.push(bit);
            let det_coords = exec.coords.apply_offset(args);
            exec.detector_coords.push(det_coords);
            exec.record_detector_event(context, detector_index, bit);
        }
        "OBSERVABLE_INCLUDE" => {
            let index = args.first().copied().unwrap_or(0.0) as u32;
            let bit = xor_recs(&exec.recorder, targets)?;
            exec.observables.push((index, bit));
        }
        "MXX" => {
            let p = args.first().copied().unwrap_or(0.0);
            pair_measure(
                &mut exec.state,
                &exec.lost,
                targets,
                PauliBasis::X,
                p,
                rng,
                &mut exec.recorder,
            )?;
        }
        "MYY" => {
            let p = args.first().copied().unwrap_or(0.0);
            pair_measure(
                &mut exec.state,
                &exec.lost,
                targets,
                PauliBasis::Y,
                p,
                rng,
                &mut exec.recorder,
            )?;
        }
        "MZZ" => {
            let p = args.first().copied().unwrap_or(0.0);
            pair_measure(
                &mut exec.state,
                &exec.lost,
                targets,
                PauliBasis::Z,
                p,
                rng,
                &mut exec.recorder,
            )?;
        }
        "MPP" => {
            let p = args.first().copied().unwrap_or(0.0);
            let products = split_pauli_products(targets)?;
            for product in &products {
                let mut bit = if product.terms.iter().any(|(q, _)| exec.lost[*q]) {
                    true
                } else {
                    measure_pauli_product(&mut exec.state, &product.terms, product.inverted, rng)
                };
                if p > 0.0 && rng.r#gen::<f64>() < p {
                    bit = !bit;
                }
                exec.recorder.push(bit);
            }
        }
        "SPP" => {
            let products = split_pauli_products(targets)?;
            for product in &products {
                apply_spp(&mut exec.state, &product.terms, product.inverted, false);
            }
        }
        "SPP_DAG" => {
            let products = split_pauli_products(targets)?;
            for product in &products {
                apply_spp(&mut exec.state, &product.terms, product.inverted, true);
            }
        }
        "PAULI_CHANNEL_1" => {
            let px = args.first().copied().unwrap_or(0.0);
            let py = args.get(1).copied().unwrap_or(0.0);
            let pz = args.get(2).copied().unwrap_or(0.0);
            for (target_slot, q) in qubit_slots(targets)? {
                if exec.lost[q] {
                    continue;
                }
                let r: f64 = rng.r#gen();
                let branch = if r < px {
                    exec.state.x_gate(q);
                    Some("X")
                } else if r < px + py {
                    exec.state.y_gate(q);
                    Some("Y")
                } else if r < px + py + pz {
                    exec.state.z_gate(q);
                    Some("Z")
                } else {
                    None
                };
                if let Some(branch) = branch {
                    exec.record_noise_event(
                        context,
                        name,
                        vec![target_slot],
                        vec![q as u32],
                        branch,
                    );
                }
            }
        }
        "PAULI_CHANNEL_2" => {
            let probs: Vec<f64> = (0..15)
                .map(|i| args.get(i).copied().unwrap_or(0.0))
                .collect();
            let paulis: [(u8, u8); 15] = [
                (0, 1),
                (0, 2),
                (0, 3),
                (1, 0),
                (1, 1),
                (1, 2),
                (1, 3),
                (2, 0),
                (2, 1),
                (2, 2),
                (2, 3),
                (3, 0),
                (3, 1),
                (3, 2),
                (3, 3),
            ];
            for ((slot_a, a), (slot_b, b)) in present_qubit_pair_slots(targets, &exec.lost)? {
                let r: f64 = rng.r#gen();
                let mut cumulative = 0.0;
                let mut chosen = None;
                for (i, &(pa, pb)) in paulis.iter().enumerate() {
                    cumulative += probs[i];
                    if r < cumulative {
                        chosen = Some((pa, pb));
                        break;
                    }
                }
                if let Some((pa, pb)) = chosen {
                    apply_pauli(&mut exec.state, a, pa);
                    apply_pauli(&mut exec.state, b, pb);
                    let label = pauli_pair_label(pa, pb);
                    exec.record_noise_event(
                        context,
                        name,
                        vec![slot_a, slot_b],
                        vec![a as u32, b as u32],
                        &label,
                    );
                }
            }
        }
        "HERALDED_ERASE" => {
            let p = args.first().copied().unwrap_or(0.0);
            for q in qubits(targets)? {
                if p > 0.0 && rng.r#gen::<f64>() < p {
                    exec.recorder.push(true);
                    match rng.gen_range(0u8..4) {
                        1 => exec.state.x_gate(q),
                        2 => exec.state.y_gate(q),
                        3 => exec.state.z_gate(q),
                        _ => {}
                    }
                } else {
                    exec.recorder.push(false);
                }
            }
        }
        "HERALDED_PAULI_CHANNEL_1" => {
            let pi = args.first().copied().unwrap_or(0.0);
            let px = args.get(1).copied().unwrap_or(0.0);
            let py = args.get(2).copied().unwrap_or(0.0);
            let pz = args.get(3).copied().unwrap_or(0.0);
            let total = pi + px + py + pz;
            for q in qubits(targets)? {
                let r: f64 = rng.r#gen();
                if r < total {
                    exec.recorder.push(true);
                    let inner = r;
                    if inner < pi {
                    } else if inner < pi + px {
                        exec.state.x_gate(q);
                    } else if inner < pi + px + py {
                        exec.state.y_gate(q);
                    } else {
                        exec.state.z_gate(q);
                    }
                } else {
                    exec.recorder.push(false);
                }
            }
        }
        "CORRELATED_ERROR" | "E" => {
            let p = args.first().copied().unwrap_or(0.0);
            if p > 0.0 && rng.r#gen::<f64>() < p {
                apply_pauli_targets(&mut exec.state, targets)?;
                if let Some((target_slots, target_qubits, label)) =
                    correlated_trace_payload(targets)?
                {
                    exec.record_noise_event(context, name, target_slots, target_qubits, &label);
                }
                exec.last_correlated_error_occurred = true;
            } else {
                exec.last_correlated_error_occurred = false;
            }
        }
        "ELSE_CORRELATED_ERROR" => {
            if !exec.last_correlated_error_occurred {
                let p = args.first().copied().unwrap_or(0.0);
                if p > 0.0 && rng.r#gen::<f64>() < p {
                    apply_pauli_targets(&mut exec.state, targets)?;
                    if let Some((target_slots, target_qubits, label)) =
                        correlated_trace_payload(targets)?
                    {
                        exec.record_noise_event(context, name, target_slots, target_qubits, &label);
                    }
                    exec.last_correlated_error_occurred = true;
                }
            }
        }
        _ => return Err(format!("unsupported instruction {}", name)),
    }
    Ok(())
}

// --- reference_sample: noiseless biased simulation ---

pub fn reference_sample(instrs: &[StimInstr]) -> Result<Vec<bool>, String> {
    reference_sample_with_sweep_bits(instrs, None)
}

pub fn reference_sample_with_sweep_bits(
    instrs: &[StimInstr],
    sweep_bits: Option<&[bool]>,
) -> Result<Vec<bool>, String> {
    let n = max_qubit(instrs)?;
    let mut state = StabilizerState::new(n);
    let mut lost = vec![false; n];
    let mut measurements = Vec::new();
    ref_sample_instrs(&mut state, &mut lost, &mut measurements, instrs, sweep_bits)?;
    Ok(measurements)
}

fn ref_sample_instrs(
    state: &mut StabilizerState,
    lost: &mut [bool],
    measurements: &mut Vec<bool>,
    instrs: &[StimInstr],
    sweep_bits: Option<&[bool]>,
) -> Result<(), String> {
    for instr in instrs {
        match instr {
            StimInstr::Op {
                name,
                args,
                targets,
                ..
            } => {
                ref_sample_op(
                    state,
                    lost,
                    measurements,
                    name.as_str(),
                    args,
                    targets,
                    sweep_bits,
                )?;
            }
            StimInstr::Repeat { count, body } => {
                for _ in 0..*count {
                    ref_sample_instrs(state, lost, measurements, body, sweep_bits)?;
                }
            }
        }
    }
    Ok(())
}

fn ref_sample_op(
    state: &mut StabilizerState,
    lost: &mut [bool],
    measurements: &mut Vec<bool>,
    name: &str,
    _args: &[f64],
    targets: &[StimTarget],
    sweep_bits: Option<&[bool]>,
) -> Result<(), String> {
    match name {
        // Identity
        "I" => {}

        // Single-qubit Cliffords
        "H" => for_each_present_qubit(targets, lost, |q| state.h(q))?,
        "H_XY" => for_each_present_qubit(targets, lost, |q| state.h_xy(q))?,
        "H_YZ" => for_each_present_qubit(targets, lost, |q| state.h_yz(q))?,
        "S" | "SQRT_Z" => for_each_present_qubit(targets, lost, |q| state.s(q))?,
        "S_DAG" | "SQRT_Z_DAG" => for_each_present_qubit(targets, lost, |q| state.s_dag(q))?,
        "SQRT_X" => for_each_present_qubit(targets, lost, |q| state.sqrt_x(q))?,
        "SQRT_X_DAG" => for_each_present_qubit(targets, lost, |q| state.sqrt_x_dag(q))?,
        "SQRT_Y" => for_each_present_qubit(targets, lost, |q| state.sqrt_y(q))?,
        "SQRT_Y_DAG" => for_each_present_qubit(targets, lost, |q| state.sqrt_y_dag(q))?,
        "X" => for_each_present_qubit(targets, lost, |q| state.x_gate(q))?,
        "Y" => for_each_present_qubit(targets, lost, |q| state.y_gate(q))?,
        "Z" => for_each_present_qubit(targets, lost, |q| state.z_gate(q))?,
        "C_XYZ" => for_each_present_qubit(targets, lost, |q| state.c_xyz(q))?,
        "C_ZYX" => for_each_present_qubit(targets, lost, |q| state.c_zyx(q))?,
        "C_NXYZ" => for_each_present_qubit(targets, lost, |q| state.c_nxyz(q))?,
        "C_NZYX" => for_each_present_qubit(targets, lost, |q| state.c_nzyx(q))?,
        "C_XNYZ" => for_each_present_qubit(targets, lost, |q| state.c_xnyz(q))?,
        "C_XYNZ" => for_each_present_qubit(targets, lost, |q| state.c_xynz(q))?,
        "C_ZNYX" => for_each_present_qubit(targets, lost, |q| state.c_znyx(q))?,
        "C_ZYNX" => for_each_present_qubit(targets, lost, |q| state.c_zynx(q))?,
        "H_NXY" => for_each_present_qubit(targets, lost, |q| state.h_nxy(q))?,
        "H_NXZ" => for_each_present_qubit(targets, lost, |q| state.h_nxz(q))?,
        "H_NYZ" => for_each_present_qubit(targets, lost, |q| state.h_nyz(q))?,

        // Two-qubit Cliffords
        "CX" | "CNOT" | "ZCX" => {
            apply_reference_controlled_pairs(state, name, targets, sweep_bits)?;
        }
        "CY" | "ZCY" => {
            apply_reference_controlled_pairs(state, name, targets, sweep_bits)?;
        }
        "CZ" | "ZCZ" => {
            apply_reference_controlled_pairs(state, name, targets, sweep_bits)?;
        }
        "XCX" => {
            for (a, b) in present_qubit_pairs(targets, lost)? {
                state.xcx(a, b);
            }
        }
        "XCY" => {
            for (a, b) in present_qubit_pairs(targets, lost)? {
                state.xcy(a, b);
            }
        }
        "XCZ" => {
            for (a, b) in present_qubit_pairs(targets, lost)? {
                state.xcz(a, b);
            }
        }
        "YCX" => {
            for (a, b) in present_qubit_pairs(targets, lost)? {
                state.ycx(a, b);
            }
        }
        "YCY" => {
            for (a, b) in present_qubit_pairs(targets, lost)? {
                state.ycy(a, b);
            }
        }
        "YCZ" => {
            for (a, b) in present_qubit_pairs(targets, lost)? {
                state.ycz(a, b);
            }
        }
        "SWAP" => {
            for (a, b) in present_qubit_pairs(targets, lost)? {
                state.swap(a, b);
            }
        }
        "ISWAP" => {
            for (a, b) in present_qubit_pairs(targets, lost)? {
                state.iswap(a, b);
            }
        }
        "ISWAP_DAG" => {
            for (a, b) in present_qubit_pairs(targets, lost)? {
                state.iswap_dag(a, b);
            }
        }
        "CXSWAP" => {
            for (a, b) in present_qubit_pairs(targets, lost)? {
                state.cxswap(a, b);
            }
        }
        "SWAPCX" => {
            for (a, b) in present_qubit_pairs(targets, lost)? {
                state.swapcx(a, b);
            }
        }
        "CZSWAP" => {
            for (a, b) in present_qubit_pairs(targets, lost)? {
                state.czswap(a, b);
            }
        }

        // Measurements (biased)
        "M" | "MZ" => {
            for (q, inv) in qubits_with_inversion(targets)? {
                record_reference_measure_z(state, lost, q, inv, measurements);
            }
        }
        "MX" => {
            for (q, inv) in qubits_with_inversion(targets)? {
                record_reference_measure_x(state, lost, q, inv, measurements);
            }
        }
        "MY" => {
            for (q, inv) in qubits_with_inversion(targets)? {
                record_reference_measure_y(state, lost, q, inv, measurements);
            }
        }
        "MR" | "MRZ" => {
            for (q, inv) in qubits_with_inversion(targets)? {
                reference_measure_reset_z(state, lost, q, inv, measurements);
            }
        }
        "MRX" => {
            for (q, inv) in qubits_with_inversion(targets)? {
                reference_measure_reset_x(state, lost, q, inv, measurements);
            }
        }
        "MRY" => {
            for (q, inv) in qubits_with_inversion(targets)? {
                reference_measure_reset_y(state, lost, q, inv, measurements);
            }
        }
        "ML" | "MZL" => {
            for (q, inv) in qubits_with_inversion(targets)? {
                record_reference_loss_visible_measure_z(state, lost, q, inv, measurements);
            }
        }
        "MXL" => {
            for (q, inv) in qubits_with_inversion(targets)? {
                record_reference_loss_visible_measure_x(state, lost, q, inv, measurements);
            }
        }
        "MYL" => {
            for (q, inv) in qubits_with_inversion(targets)? {
                record_reference_loss_visible_measure_y(state, lost, q, inv, measurements);
            }
        }
        "MRL" | "MRZL" => {
            for (q, inv) in qubits_with_inversion(targets)? {
                reference_measure_reset_loss_visible_z(state, lost, q, inv, measurements);
            }
        }
        "MRXL" => {
            for (q, inv) in qubits_with_inversion(targets)? {
                reference_measure_reset_loss_visible_x(state, lost, q, inv, measurements);
            }
        }
        "MRYL" => {
            for (q, inv) in qubits_with_inversion(targets)? {
                reference_measure_reset_loss_visible_y(state, lost, q, inv, measurements);
            }
        }
        "MPAD" => {
            for t in targets {
                let q = expect_qubit(t)?;
                measurements.push(q != 0);
            }
        }

        // Resets (biased)
        "R" | "RZ" => {
            for q in qubits(targets)? {
                lost[q] = false;
                state.reset_z_biased(q);
            }
        }
        "RX" => {
            for q in qubits(targets)? {
                lost[q] = false;
                state.reset_x_biased(q);
            }
        }
        "RY" => {
            for q in qubits(targets)? {
                lost[q] = false;
                state.reset_y_biased(q);
            }
        }
        "LOSS" => for _q in qubits(targets)? {},

        // Multi-qubit Pauli measurements (biased)
        "MPP" => {
            let products = split_pauli_products(targets)?;
            for product in &products {
                let bit = reference_measure_pauli_product_biased(
                    state,
                    lost,
                    &product.terms,
                    product.inverted,
                );
                measurements.push(bit);
            }
        }
        "MXX" => {
            let pairs = qubits_with_inversion_pairs(targets)?;
            for ((a, inv_a), (b, _)) in pairs {
                let bit = reference_measure_pair_biased(state, lost, a, b, PauliBasis::X, inv_a);
                measurements.push(bit);
            }
        }
        "MYY" => {
            let pairs = qubits_with_inversion_pairs(targets)?;
            for ((a, inv_a), (b, _)) in pairs {
                let bit = reference_measure_pair_biased(state, lost, a, b, PauliBasis::Y, inv_a);
                measurements.push(bit);
            }
        }
        "MZZ" => {
            let pairs = qubits_with_inversion_pairs(targets)?;
            for ((a, inv_a), (b, _)) in pairs {
                let bit = reference_measure_pair_biased(state, lost, a, b, PauliBasis::Z, inv_a);
                measurements.push(bit);
            }
        }

        // SPP gates (no measurements, same logic as executor)
        "SPP" => {
            let products = split_pauli_products(targets)?;
            for product in &products {
                apply_spp(state, &product.terms, product.inverted, false);
            }
        }
        "SPP_DAG" => {
            let products = split_pauli_products(targets)?;
            for product in &products {
                apply_spp(state, &product.terms, product.inverted, true);
            }
        }

        // Heralded channels: push false per target (no herald in noiseless)
        "HERALDED_ERASE" | "HERALDED_PAULI_CHANNEL_1" => {
            for _q in qubits(targets)? {
                measurements.push(false);
            }
        }

        // Noise instructions: skip
        "X_ERROR"
        | "Z_ERROR"
        | "Y_ERROR"
        | "DEPOLARIZE1"
        | "DEPOLARIZE2"
        | "CORRELATED_ERROR"
        | "E"
        | "ELSE_CORRELATED_ERROR"
        | "PAULI_CHANNEL_1"
        | "PAULI_CHANNEL_2"
        | "I_ERROR"
        | "II_ERROR" => {}

        // Metadata: skip
        "TICK" | "QUBIT_COORDS" | "SHIFT_COORDS" | "DETECTOR" | "OBSERVABLE_INCLUDE" => {}

        _ => {
            return Err(format!(
                "reference_sample: unsupported instruction {}",
                name
            ));
        }
    }
    Ok(())
}

fn apply_reference_controlled_pairs(
    state: &mut StabilizerState,
    name: &str,
    targets: &[StimTarget],
    sweep_bits: Option<&[bool]>,
) -> Result<(), String> {
    if targets.len() % 2 != 0 {
        return Err("odd number of targets".to_string());
    }
    let mut it = targets.iter();
    while let (Some(a), Some(b)) = (it.next(), it.next()) {
        match (a, b) {
            (StimTarget::Qubit(c), StimTarget::Qubit(t)) => match name {
                "CX" | "CNOT" | "ZCX" => state.cx(*c as usize, *t as usize),
                "CY" | "ZCY" => state.cy(*c as usize, *t as usize),
                "CZ" | "ZCZ" => state.cz(*c as usize, *t as usize),
                _ => return Err(format!("unsupported reference pair op {name}")),
            },
            (StimTarget::Sweep(k), StimTarget::Qubit(q)) => {
                let active = sweep_bits
                    .and_then(|bits| bits.get(*k as usize))
                    .copied()
                    .unwrap_or(false);
                if active {
                    match name {
                        "CX" | "CNOT" | "ZCX" => state.x_gate(*q as usize),
                        "CY" | "ZCY" => state.y_gate(*q as usize),
                        "CZ" | "ZCZ" => state.z_gate(*q as usize),
                        _ => return Err(format!("unsupported reference pair op {name}")),
                    }
                }
            }
            (StimTarget::Sweep(_), _) | (_, StimTarget::Sweep(_)) => {
                return Err("unsupported sweep target placement".to_string());
            }
            _ => return Err("expected qubit target in pair".to_string()),
        }
    }
    Ok(())
}

fn measure_pauli_product_biased(
    state: &mut StabilizerState,
    terms: &[(usize, PauliBasis)],
    inverted: bool,
) -> bool {
    if terms.is_empty() {
        return inverted;
    }

    for &(q, basis) in terms {
        match basis {
            PauliBasis::X => state.h(q),
            PauliBasis::Y => state.h_yz(q),
            PauliBasis::Z => {}
        }
    }

    let anchor = terms.last().unwrap().0;
    let non_anchor: Vec<usize> = terms
        .iter()
        .map(|&(q, _)| q)
        .filter(|&q| q != anchor)
        .collect();
    for &q in &non_anchor {
        state.cx(q, anchor);
    }

    let bit = state.measure_z_biased(anchor);
    let result = (bit == 1) ^ inverted;

    for &q in non_anchor.iter().rev() {
        state.cx(q, anchor);
    }

    for &(q, basis) in terms {
        match basis {
            PauliBasis::X => state.h(q),
            PauliBasis::Y => state.h_yz(q),
            PauliBasis::Z => {}
        }
    }

    result
}

fn reference_measure_pauli_product_biased(
    state: &mut StabilizerState,
    lost: &[bool],
    terms: &[(usize, PauliBasis)],
    inverted: bool,
) -> bool {
    if terms.iter().any(|(q, _)| lost[*q]) {
        true
    } else {
        measure_pauli_product_biased(state, terms, inverted)
    }
}

fn reference_measure_pair_biased(
    state: &mut StabilizerState,
    lost: &[bool],
    a: usize,
    b: usize,
    basis: PauliBasis,
    inverted: bool,
) -> bool {
    if lost[a] || lost[b] {
        true ^ inverted
    } else {
        let terms = [(a, basis), (b, basis)];
        measure_pauli_product_biased(state, &terms, inverted)
    }
}

fn recorder_bits(r: Recorder) -> Vec<bool> {
    let mut out = Vec::new();
    for i in 1..=r.len() {
        out.push(r.rec(-(i as i32)).unwrap());
    }
    out.reverse();
    out
}

pub(crate) fn max_qubit(instrs: &[StimInstr]) -> Result<usize, String> {
    let mut max_q: Option<u32> = None;
    for i in instrs {
        match i {
            StimInstr::Op { targets, .. } => {
                for t in targets {
                    match t {
                        StimTarget::Qubit(q) | StimTarget::QubitInv(q) => {
                            max_q = Some(max_q.map_or(*q, |m| m.max(*q)));
                        }
                        StimTarget::Pauli { qubit: q, .. } => {
                            max_q = Some(max_q.map_or(*q, |m| m.max(*q)));
                        }
                        StimTarget::Combiner | StimTarget::Rec(_) | StimTarget::Sweep(_) => {}
                    }
                }
            }
            StimInstr::Repeat { body, .. } => {
                let inner = max_qubit(body)? as u32;
                max_q = Some(max_q.map_or(inner, |m| m.max(inner)));
            }
        }
    }
    Ok(max_q.map(|m| (m as usize) + 1).unwrap_or(0))
}

fn qubits(targets: &[StimTarget]) -> Result<Vec<usize>, String> {
    let mut out = Vec::new();
    for t in targets {
        match t {
            StimTarget::Qubit(q) => out.push(*q as usize),
            StimTarget::Sweep(_) => {} // skip: treated as always-0 (no-op)
            _ => out.push(expect_qubit(t)?),
        }
    }
    Ok(out)
}

fn qubit_slots(targets: &[StimTarget]) -> Result<Vec<(usize, usize)>, String> {
    let mut out = Vec::new();
    for (slot, t) in targets.iter().enumerate() {
        match t {
            StimTarget::Qubit(q) => out.push((slot, *q as usize)),
            StimTarget::Sweep(_) => {}
            _ => return Err("expected qubit target".to_string()),
        }
    }
    Ok(out)
}

fn qubits_with_inversion(targets: &[StimTarget]) -> Result<Vec<(usize, bool)>, String> {
    let mut out = Vec::new();
    for t in targets {
        match t {
            StimTarget::Qubit(q) => out.push((*q as usize, false)),
            StimTarget::QubitInv(q) => out.push((*q as usize, true)),
            StimTarget::Sweep(_) => {} // skip: treated as always-0 (no-op)
            _ => return Err("expected qubit target".to_string()),
        }
    }
    Ok(out)
}

fn qubits_with_inversion_slots(
    targets: &[StimTarget],
) -> Result<Vec<(usize, usize, bool)>, String> {
    let mut out = Vec::new();
    for (slot, t) in targets.iter().enumerate() {
        match t {
            StimTarget::Qubit(q) => out.push((slot, *q as usize, false)),
            StimTarget::QubitInv(q) => out.push((slot, *q as usize, true)),
            StimTarget::Sweep(_) => {}
            _ => return Err("expected qubit target".to_string()),
        }
    }
    Ok(out)
}

fn qubits_with_inversion_pairs(
    targets: &[StimTarget],
) -> Result<Vec<((usize, bool), (usize, bool))>, String> {
    let flat = qubits_with_inversion(targets)?;
    if flat.len() % 2 != 0 {
        return Err("odd number of targets for pair measurement".to_string());
    }
    Ok(flat.chunks(2).map(|c| (c[0], c[1])).collect())
}

fn for_each_present_qubit<F: FnMut(usize)>(
    targets: &[StimTarget],
    lost: &[bool],
    mut f: F,
) -> Result<(), String> {
    for t in targets {
        let q = expect_qubit(t)?;
        if !lost[q] {
            f(q);
        }
    }
    Ok(())
}

fn expect_qubit(t: &StimTarget) -> Result<usize, String> {
    match t {
        StimTarget::Qubit(q) => Ok(*q as usize),
        StimTarget::QubitInv(_) => {
            Err("inverted qubit target only valid for measurement".to_string())
        }
        StimTarget::Sweep(_) => Err("sweep[] target unexpected here".to_string()),
        _ => Err("expected qubit target".to_string()),
    }
}

fn qubit_pairs(targets: &[StimTarget]) -> Result<Vec<(usize, usize)>, String> {
    if targets.len() % 2 != 0 {
        return Err("odd number of targets".to_string());
    }
    let mut out = Vec::new();
    let mut it = targets.iter();
    while let (Some(a), Some(b)) = (it.next(), it.next()) {
        // Skip any pair that contains a sweep target (sweep=0 means gate is no-op)
        if matches!(a, StimTarget::Sweep(_)) || matches!(b, StimTarget::Sweep(_)) {
            continue;
        }
        out.push((expect_qubit(a)?, expect_qubit(b)?));
    }
    Ok(out)
}

fn present_qubit_pairs(
    targets: &[StimTarget],
    lost: &[bool],
) -> Result<Vec<(usize, usize)>, String> {
    Ok(qubit_pairs(targets)?
        .into_iter()
        .filter(|(a, b)| !lost[*a] && !lost[*b])
        .collect())
}

fn present_qubit_pair_slots(
    targets: &[StimTarget],
    lost: &[bool],
) -> Result<Vec<((usize, usize), (usize, usize))>, String> {
    if targets.len() % 2 != 0 {
        return Err("odd number of targets".to_string());
    }
    let mut out = Vec::new();
    let mut it = targets.iter().enumerate();
    while let (Some((slot_a, a)), Some((slot_b, b))) = (it.next(), it.next()) {
        if matches!(a, StimTarget::Sweep(_)) || matches!(b, StimTarget::Sweep(_)) {
            continue;
        }
        let a = expect_qubit(a)?;
        let b = expect_qubit(b)?;
        if !lost[a] && !lost[b] {
            out.push(((slot_a, a), (slot_b, b)));
        }
    }
    Ok(out)
}

fn xor_recs(r: &Recorder, targets: &[StimTarget]) -> Result<bool, String> {
    let mut acc = false;
    for t in targets {
        match t {
            StimTarget::Rec(o) => {
                let bit = r.rec(*o).ok_or("rec out of range")?;
                acc ^= bit;
            }
            _ => return Err("detector target must be rec".to_string()),
        }
    }
    Ok(acc)
}

fn apply_pauli_targets(state: &mut StabilizerState, targets: &[StimTarget]) -> Result<(), String> {
    for t in targets {
        match t {
            StimTarget::Pauli { qubit, basis, .. } => {
                let q = *qubit as usize;
                match basis {
                    PauliBasis::X => state.x_gate(q),
                    PauliBasis::Y => state.y_gate(q),
                    PauliBasis::Z => state.z_gate(q),
                }
            }
            _ => return Err("CORRELATED_ERROR targets must be Pauli".to_string()),
        }
    }
    Ok(())
}

fn correlated_trace_payload(
    targets: &[StimTarget],
) -> Result<Option<(Vec<usize>, Vec<u32>, String)>, String> {
    let mut target_slots = Vec::new();
    let mut target_qubits = Vec::new();
    let mut label = String::new();
    for (slot, target) in targets.iter().enumerate() {
        match target {
            StimTarget::Pauli { qubit, basis, .. } => {
                target_slots.push(slot);
                target_qubits.push(*qubit);
                label.push(match basis {
                    PauliBasis::X => 'X',
                    PauliBasis::Y => 'Y',
                    PauliBasis::Z => 'Z',
                });
            }
            _ => return Ok(None),
        }
    }
    Ok(Some((target_slots, target_qubits, label)))
}

fn pauli_pair_label(pa: u8, pb: u8) -> String {
    let mut label = String::new();
    label.push(match pa {
        0 => 'I',
        1 => 'X',
        2 => 'Y',
        3 => 'Z',
        _ => '?',
    });
    label.push(match pb {
        0 => 'I',
        1 => 'X',
        2 => 'Y',
        3 => 'Z',
        _ => '?',
    });
    label
}

fn apply_pauli(state: &mut StabilizerState, q: usize, p: u8) {
    match p {
        0 => {}
        1 => state.x_gate(q),
        2 => state.y_gate(q),
        3 => state.z_gate(q),
        _ => {}
    }
}

fn record_measure_z(
    state: &mut StabilizerState,
    lost: &[bool],
    q: usize,
    inv: bool,
    rng: &mut impl Rng,
    recorder: &mut Recorder,
) -> MeasurementOutcome {
    if lost[q] {
        let bit = true ^ inv;
        recorder.push(bit);
        return MeasurementOutcome {
            bit,
            loss_cause: true,
        };
    }
    let (bit, _) = state.measure_z(q, rng);
    let bit = (bit == 1) ^ inv;
    recorder.push(bit);
    MeasurementOutcome {
        bit,
        loss_cause: false,
    }
}

fn record_measure_x(
    state: &mut StabilizerState,
    lost: &[bool],
    q: usize,
    inv: bool,
    rng: &mut impl Rng,
    recorder: &mut Recorder,
) -> MeasurementOutcome {
    if lost[q] {
        let bit = true ^ inv;
        recorder.push(bit);
        return MeasurementOutcome {
            bit,
            loss_cause: true,
        };
    }
    state.h(q);
    let (bit, _) = state.measure_z(q, rng);
    state.h(q);
    let bit = (bit == 1) ^ inv;
    recorder.push(bit);
    MeasurementOutcome {
        bit,
        loss_cause: false,
    }
}

fn record_measure_y(
    state: &mut StabilizerState,
    lost: &[bool],
    q: usize,
    inv: bool,
    rng: &mut impl Rng,
    recorder: &mut Recorder,
) -> MeasurementOutcome {
    if lost[q] {
        let bit = true ^ inv;
        recorder.push(bit);
        return MeasurementOutcome {
            bit,
            loss_cause: true,
        };
    }
    state.s_dag(q);
    state.h(q);
    let (bit, _) = state.measure_z(q, rng);
    state.h(q);
    state.s(q);
    let bit = (bit == 1) ^ inv;
    recorder.push(bit);
    MeasurementOutcome {
        bit,
        loss_cause: false,
    }
}

fn measure_reset_z(
    state: &mut StabilizerState,
    lost: &mut [bool],
    q: usize,
    inv: bool,
    rng: &mut impl Rng,
    recorder: &mut Recorder,
) -> MeasurementOutcome {
    if lost[q] {
        let bit = true ^ inv;
        recorder.push(bit);
        lost[q] = false;
        state.reset_z(q, rng);
        return MeasurementOutcome {
            bit,
            loss_cause: true,
        };
    }
    let (raw_bit, _) = state.measure_z(q, rng);
    let bit = (raw_bit == 1) ^ inv;
    recorder.push(bit);
    if raw_bit == 1 {
        state.x_gate(q);
    }
    MeasurementOutcome {
        bit,
        loss_cause: false,
    }
}

fn measure_reset_x(
    state: &mut StabilizerState,
    lost: &mut [bool],
    q: usize,
    inv: bool,
    rng: &mut impl Rng,
    recorder: &mut Recorder,
) -> MeasurementOutcome {
    if lost[q] {
        let bit = true ^ inv;
        recorder.push(bit);
        lost[q] = false;
        state.reset_x(q, rng);
        return MeasurementOutcome {
            bit,
            loss_cause: true,
        };
    }
    state.h(q);
    let (raw_bit, _) = state.measure_z(q, rng);
    let bit = (raw_bit == 1) ^ inv;
    recorder.push(bit);
    if raw_bit == 1 {
        state.x_gate(q);
    }
    state.h(q);
    MeasurementOutcome {
        bit,
        loss_cause: false,
    }
}

fn measure_reset_y(
    state: &mut StabilizerState,
    lost: &mut [bool],
    q: usize,
    inv: bool,
    rng: &mut impl Rng,
    recorder: &mut Recorder,
) -> MeasurementOutcome {
    if lost[q] {
        let bit = true ^ inv;
        recorder.push(bit);
        lost[q] = false;
        state.reset_y(q, rng);
        return MeasurementOutcome {
            bit,
            loss_cause: true,
        };
    }
    state.s_dag(q);
    state.h(q);
    let (raw_bit, _) = state.measure_z(q, rng);
    let bit = (raw_bit == 1) ^ inv;
    recorder.push(bit);
    if raw_bit == 1 {
        state.x_gate(q);
    }
    state.h(q);
    state.s(q);
    MeasurementOutcome {
        bit,
        loss_cause: false,
    }
}

fn record_loss_visible_measure_z(
    state: &mut StabilizerState,
    lost: &[bool],
    q: usize,
    inv: bool,
    rng: &mut impl Rng,
    recorder: &mut Recorder,
) -> LossVisibleMeasurementOutcome {
    let loss_flag = lost[q];
    recorder.push(loss_flag);
    let value = record_measure_z(state, lost, q, inv, rng, recorder);
    LossVisibleMeasurementOutcome { loss_flag, value }
}

fn record_loss_visible_measure_x(
    state: &mut StabilizerState,
    lost: &[bool],
    q: usize,
    inv: bool,
    rng: &mut impl Rng,
    recorder: &mut Recorder,
) -> LossVisibleMeasurementOutcome {
    let loss_flag = lost[q];
    recorder.push(loss_flag);
    let value = record_measure_x(state, lost, q, inv, rng, recorder);
    LossVisibleMeasurementOutcome { loss_flag, value }
}

fn record_loss_visible_measure_y(
    state: &mut StabilizerState,
    lost: &[bool],
    q: usize,
    inv: bool,
    rng: &mut impl Rng,
    recorder: &mut Recorder,
) -> LossVisibleMeasurementOutcome {
    let loss_flag = lost[q];
    recorder.push(loss_flag);
    let value = record_measure_y(state, lost, q, inv, rng, recorder);
    LossVisibleMeasurementOutcome { loss_flag, value }
}

fn measure_reset_loss_visible_z(
    state: &mut StabilizerState,
    lost: &mut [bool],
    q: usize,
    inv: bool,
    rng: &mut impl Rng,
    recorder: &mut Recorder,
) -> LossVisibleMeasurementOutcome {
    let loss_flag = lost[q];
    recorder.push(loss_flag);
    let value = measure_reset_z(state, lost, q, inv, rng, recorder);
    LossVisibleMeasurementOutcome { loss_flag, value }
}

fn measure_reset_loss_visible_x(
    state: &mut StabilizerState,
    lost: &mut [bool],
    q: usize,
    inv: bool,
    rng: &mut impl Rng,
    recorder: &mut Recorder,
) -> LossVisibleMeasurementOutcome {
    let loss_flag = lost[q];
    recorder.push(loss_flag);
    let value = measure_reset_x(state, lost, q, inv, rng, recorder);
    LossVisibleMeasurementOutcome { loss_flag, value }
}

fn measure_reset_loss_visible_y(
    state: &mut StabilizerState,
    lost: &mut [bool],
    q: usize,
    inv: bool,
    rng: &mut impl Rng,
    recorder: &mut Recorder,
) -> LossVisibleMeasurementOutcome {
    let loss_flag = lost[q];
    recorder.push(loss_flag);
    let value = measure_reset_y(state, lost, q, inv, rng, recorder);
    LossVisibleMeasurementOutcome { loss_flag, value }
}

fn record_reference_measure_z(
    state: &mut StabilizerState,
    lost: &[bool],
    q: usize,
    inv: bool,
    measurements: &mut Vec<bool>,
) {
    if lost[q] {
        measurements.push(true ^ inv);
        return;
    }
    let bit = state.measure_z_biased(q);
    measurements.push((bit == 1) ^ inv);
}

fn record_reference_measure_x(
    state: &mut StabilizerState,
    lost: &[bool],
    q: usize,
    inv: bool,
    measurements: &mut Vec<bool>,
) {
    if lost[q] {
        measurements.push(true ^ inv);
        return;
    }
    state.h(q);
    let bit = state.measure_z_biased(q);
    state.h(q);
    measurements.push((bit == 1) ^ inv);
}

fn record_reference_measure_y(
    state: &mut StabilizerState,
    lost: &[bool],
    q: usize,
    inv: bool,
    measurements: &mut Vec<bool>,
) {
    if lost[q] {
        measurements.push(true ^ inv);
        return;
    }
    state.s_dag(q);
    state.h(q);
    let bit = state.measure_z_biased(q);
    state.h(q);
    state.s(q);
    measurements.push((bit == 1) ^ inv);
}

fn reference_measure_reset_z(
    state: &mut StabilizerState,
    lost: &mut [bool],
    q: usize,
    inv: bool,
    measurements: &mut Vec<bool>,
) {
    if lost[q] {
        measurements.push(true ^ inv);
        lost[q] = false;
        state.reset_z_biased(q);
        return;
    }
    let bit = state.measure_z_biased(q);
    measurements.push((bit == 1) ^ inv);
    if bit == 1 {
        state.x_gate(q);
    }
}

fn reference_measure_reset_x(
    state: &mut StabilizerState,
    lost: &mut [bool],
    q: usize,
    inv: bool,
    measurements: &mut Vec<bool>,
) {
    if lost[q] {
        measurements.push(true ^ inv);
        lost[q] = false;
        state.reset_x_biased(q);
        return;
    }
    state.h(q);
    let bit = state.measure_z_biased(q);
    measurements.push((bit == 1) ^ inv);
    if bit == 1 {
        state.x_gate(q);
    }
    state.h(q);
}

fn reference_measure_reset_y(
    state: &mut StabilizerState,
    lost: &mut [bool],
    q: usize,
    inv: bool,
    measurements: &mut Vec<bool>,
) {
    if lost[q] {
        measurements.push(true ^ inv);
        lost[q] = false;
        state.reset_y_biased(q);
        return;
    }
    state.s_dag(q);
    state.h(q);
    let bit = state.measure_z_biased(q);
    measurements.push((bit == 1) ^ inv);
    if bit == 1 {
        state.x_gate(q);
    }
    state.h(q);
    state.s(q);
}

fn record_reference_loss_visible_measure_z(
    state: &mut StabilizerState,
    lost: &[bool],
    q: usize,
    inv: bool,
    measurements: &mut Vec<bool>,
) {
    measurements.push(lost[q]);
    record_reference_measure_z(state, lost, q, inv, measurements);
}

fn record_reference_loss_visible_measure_x(
    state: &mut StabilizerState,
    lost: &[bool],
    q: usize,
    inv: bool,
    measurements: &mut Vec<bool>,
) {
    measurements.push(lost[q]);
    record_reference_measure_x(state, lost, q, inv, measurements);
}

fn record_reference_loss_visible_measure_y(
    state: &mut StabilizerState,
    lost: &[bool],
    q: usize,
    inv: bool,
    measurements: &mut Vec<bool>,
) {
    measurements.push(lost[q]);
    record_reference_measure_y(state, lost, q, inv, measurements);
}

fn reference_measure_reset_loss_visible_z(
    state: &mut StabilizerState,
    lost: &mut [bool],
    q: usize,
    inv: bool,
    measurements: &mut Vec<bool>,
) {
    measurements.push(lost[q]);
    reference_measure_reset_z(state, lost, q, inv, measurements);
}

fn reference_measure_reset_loss_visible_x(
    state: &mut StabilizerState,
    lost: &mut [bool],
    q: usize,
    inv: bool,
    measurements: &mut Vec<bool>,
) {
    measurements.push(lost[q]);
    reference_measure_reset_x(state, lost, q, inv, measurements);
}

fn reference_measure_reset_loss_visible_y(
    state: &mut StabilizerState,
    lost: &mut [bool],
    q: usize,
    inv: bool,
    measurements: &mut Vec<bool>,
) {
    measurements.push(lost[q]);
    reference_measure_reset_y(state, lost, q, inv, measurements);
}

fn two_qubit_pauli(r: usize) -> (u8, u8) {
    // Map 0..14 to 15 non-identity pairs from {I,X,Y,Z}^2 \ {II}
    let mut idx = 0usize;
    for a in 0..4 {
        for b in 0..4 {
            if a == 0 && b == 0 {
                continue;
            }
            if idx == r {
                return (a as u8, b as u8);
            }
            idx += 1;
        }
    }
    (0, 0)
}

struct PauliProduct {
    terms: Vec<(usize, PauliBasis)>,
    inverted: bool,
}

fn split_pauli_products(targets: &[StimTarget]) -> Result<Vec<PauliProduct>, String> {
    let mut products = Vec::new();
    let mut current_terms: Vec<(usize, PauliBasis)> = Vec::new();
    let mut inverted = false;
    let mut after_combiner = false;

    for target in targets {
        match target {
            StimTarget::Pauli {
                qubit,
                basis,
                inverted: inv,
            } => {
                if !after_combiner && !current_terms.is_empty() {
                    products.push(PauliProduct {
                        terms: std::mem::take(&mut current_terms),
                        inverted,
                    });
                    inverted = false;
                }
                if current_terms.is_empty() && *inv {
                    inverted = true;
                }
                current_terms.push((*qubit as usize, *basis));
                after_combiner = false;
            }
            StimTarget::Combiner => {
                after_combiner = true;
            }
            _ => return Err("MPP targets must be Pauli targets".to_string()),
        }
    }
    if !current_terms.is_empty() {
        products.push(PauliProduct {
            terms: current_terms,
            inverted,
        });
    }
    Ok(products)
}

fn measure_pauli_product(
    state: &mut StabilizerState,
    terms: &[(usize, PauliBasis)],
    inverted: bool,
    rng: &mut impl Rng,
) -> bool {
    if terms.is_empty() {
        return inverted;
    }

    // Basis change: X→Z via H, Y→Z via H_YZ
    for &(q, basis) in terms {
        match basis {
            PauliBasis::X => state.h(q),
            PauliBasis::Y => state.h_yz(q),
            PauliBasis::Z => {}
        }
    }

    // CX fold: chain all qubits' Z parity onto anchor (last qubit)
    let anchor = terms.last().unwrap().0;
    let non_anchor: Vec<usize> = terms
        .iter()
        .map(|&(q, _)| q)
        .filter(|&q| q != anchor)
        .collect();
    for &q in &non_anchor {
        state.cx(q, anchor);
    }

    // Measure anchor in Z basis
    let (bit, _) = state.measure_z(anchor, rng);
    let result = (bit == 1) ^ inverted;

    // Uncompute CX (reverse order, CX is self-inverse)
    for &q in non_anchor.iter().rev() {
        state.cx(q, anchor);
    }

    // Undo basis change (H and H_YZ are self-inverse)
    for &(q, basis) in terms {
        match basis {
            PauliBasis::X => state.h(q),
            PauliBasis::Y => state.h_yz(q),
            PauliBasis::Z => {}
        }
    }

    result
}

fn apply_spp(
    state: &mut StabilizerState,
    terms: &[(usize, PauliBasis)],
    inverted: bool,
    dag: bool,
) {
    if terms.is_empty() {
        return;
    }

    for &(q, basis) in terms {
        match basis {
            PauliBasis::X => state.h(q),
            PauliBasis::Y => state.h_yz(q),
            PauliBasis::Z => {}
        }
    }

    let anchor = terms.last().unwrap().0;
    let non_anchor: Vec<usize> = terms
        .iter()
        .map(|&(q, _)| q)
        .filter(|&q| q != anchor)
        .collect();
    for &q in &non_anchor {
        state.cx(q, anchor);
    }

    if dag ^ inverted {
        state.s_dag(anchor);
    } else {
        state.s(anchor);
    }

    for &q in non_anchor.iter().rev() {
        state.cx(q, anchor);
    }

    for &(q, basis) in terms {
        match basis {
            PauliBasis::X => state.h(q),
            PauliBasis::Y => state.h_yz(q),
            PauliBasis::Z => {}
        }
    }
}

fn pair_measure(
    state: &mut StabilizerState,
    lost: &[bool],
    targets: &[StimTarget],
    basis: PauliBasis,
    noise_p: f64,
    rng: &mut impl Rng,
    recorder: &mut Recorder,
) -> Result<(), String> {
    let pairs = qubits_with_inversion_pairs(targets)?;
    for ((a, inv_a), (b, _inv_b)) in pairs {
        let mut bit = if lost[a] || lost[b] {
            true ^ inv_a
        } else {
            let terms = vec![(a, basis), (b, basis)];
            measure_pauli_product(state, &terms, inv_a, rng)
        };
        if noise_p > 0.0 && rng.r#gen::<f64>() < noise_p {
            bit = !bit;
        }
        recorder.push(bit);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_lines;
    use rand::RngCore;
    use std::collections::VecDeque;

    struct ScriptRng {
        values: VecDeque<u64>,
    }

    impl ScriptRng {
        fn new(values: Vec<u64>) -> Self {
            Self {
                values: values.into(),
            }
        }

        fn next_value(&mut self) -> u64 {
            self.values.pop_front().unwrap_or(0)
        }
    }

    impl RngCore for ScriptRng {
        fn next_u32(&mut self) -> u32 {
            self.next_value() as u32
        }

        fn next_u64(&mut self) -> u64 {
            self.next_value()
        }

        fn fill_bytes(&mut self, dest: &mut [u8]) {
            for chunk in dest.chunks_mut(8) {
                let bytes = self.next_u64().to_ne_bytes();
                let len = chunk.len();
                chunk.copy_from_slice(&bytes[..len]);
            }
        }

        fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand::Error> {
            self.fill_bytes(dest);
            Ok(())
        }
    }

    #[test]
    fn reference_measurement_helpers_return_one_for_lost_qubits() {
        let mut state = StabilizerState::new(1);
        let lost = vec![true];
        let mut measurements = Vec::new();

        record_reference_measure_z(&mut state, &lost, 0, false, &mut measurements);
        record_reference_measure_x(&mut state, &lost, 0, true, &mut measurements);
        record_reference_measure_y(&mut state, &lost, 0, false, &mut measurements);

        assert_eq!(measurements, vec![true, false, true]);
    }

    #[test]
    fn reference_measure_reset_helpers_clear_loss_and_reset_state() {
        let mut state = StabilizerState::new(3);
        let mut lost = vec![true, true, true];
        let mut measurements = Vec::new();

        reference_measure_reset_z(&mut state, &mut lost, 0, false, &mut measurements);
        reference_measure_reset_x(&mut state, &mut lost, 1, true, &mut measurements);
        reference_measure_reset_y(&mut state, &mut lost, 2, false, &mut measurements);

        assert_eq!(measurements, vec![true, false, true]);
        assert_eq!(lost, vec![false, false, false]);

        let mut post_reset_measurements = Vec::new();
        record_reference_measure_z(&mut state, &lost, 0, false, &mut post_reset_measurements);
        record_reference_measure_x(&mut state, &lost, 1, false, &mut post_reset_measurements);
        record_reference_measure_y(&mut state, &lost, 2, false, &mut post_reset_measurements);
        assert_eq!(post_reset_measurements, vec![false, false, false]);
    }

    #[test]
    fn reference_measure_reset_x_and_y_correct_one_outcomes() {
        let mut state = StabilizerState::new(2);
        let mut lost = vec![false, false];
        let mut measurements = Vec::new();

        state.h(0);
        state.z_gate(0);
        reference_measure_reset_x(&mut state, &mut lost, 0, false, &mut measurements);

        state.h(1);
        state.s_dag(1);
        reference_measure_reset_y(&mut state, &mut lost, 1, false, &mut measurements);

        assert_eq!(measurements, vec![true, true]);

        let mut post_reset_measurements = Vec::new();
        record_reference_measure_x(&mut state, &lost, 0, false, &mut post_reset_measurements);
        record_reference_measure_y(&mut state, &lost, 1, false, &mut post_reset_measurements);
        assert_eq!(post_reset_measurements, vec![false, false]);
    }

    #[test]
    fn reference_multi_pauli_measurements_report_loss() {
        let mut state = StabilizerState::new(2);
        let lost = vec![true, false];

        assert!(reference_measure_pauli_product_biased(
            &mut state,
            &lost,
            &[(0, PauliBasis::X), (1, PauliBasis::Y)],
            true,
        ));
        assert!(reference_measure_pair_biased(
            &mut state,
            &lost,
            0,
            1,
            PauliBasis::X,
            false,
        ));
        assert!(!reference_measure_pair_biased(
            &mut state,
            &lost,
            0,
            1,
            PauliBasis::Y,
            true,
        ));
        assert!(reference_measure_pair_biased(
            &mut state,
            &lost,
            0,
            1,
            PauliBasis::Z,
            false,
        ));
    }

    #[test]
    fn executor_helpers_cover_branchy_paths_and_errors() {
        let mut traced = Executor::from_instrs(parse_lines("H 0\nREPEAT 1 {\n  M 0\n}\n").unwrap())
            .unwrap();
        let _ = traced.run_with_trace(&mut ScriptRng::new(vec![0, 0])).unwrap();

        let ctx = ExecutionTraversalContext::default();
        let mut exec = ExecutionState::new(4, true);

        execute_op(
            "DEPOLARIZE1",
            &[1.0],
            &[StimTarget::Qubit(0)],
            &ctx,
            &mut exec,
            &mut ScriptRng::new(vec![0, 0]),
        )
        .unwrap();
        execute_op(
            "DEPOLARIZE1",
            &[1.0],
            &[StimTarget::Qubit(1)],
            &ctx,
            &mut exec,
            &mut ScriptRng::new(vec![0, 1]),
        )
        .unwrap();
        execute_op(
            "DEPOLARIZE1",
            &[1.0],
            &[StimTarget::Qubit(2)],
            &ctx,
            &mut exec,
            &mut ScriptRng::new(vec![0, 2]),
        )
        .unwrap();

        exec.lost[3] = true;
        execute_op(
            "PAULI_CHANNEL_1",
            &[1.0, 0.0, 0.0],
            &[StimTarget::Qubit(3)],
            &ctx,
            &mut exec,
            &mut ScriptRng::new(vec![0]),
        )
        .unwrap();
        execute_op(
            "PAULI_CHANNEL_1",
            &[1.0, 0.0, 0.0],
            &[StimTarget::Qubit(0)],
            &ctx,
            &mut exec,
            &mut ScriptRng::new(vec![0]),
        )
        .unwrap();
        execute_op(
            "PAULI_CHANNEL_1",
            &[0.0, 1.0, 0.0],
            &[StimTarget::Qubit(1)],
            &ctx,
            &mut exec,
            &mut ScriptRng::new(vec![0]),
        )
        .unwrap();
        execute_op(
            "PAULI_CHANNEL_1",
            &[0.0, 0.0, 1.0],
            &[StimTarget::Qubit(2)],
            &ctx,
            &mut exec,
            &mut ScriptRng::new(vec![0]),
        )
        .unwrap();
        execute_op(
            "PAULI_CHANNEL_1",
            &[0.0, 0.0, 0.0],
            &[StimTarget::Qubit(2)],
            &ctx,
            &mut exec,
            &mut ScriptRng::new(vec![0]),
        )
        .unwrap();

        let branches = exec
            .trace
            .noise_events
            .iter()
            .map(|e| e.branch_label.as_deref())
            .collect::<Vec<_>>();
        assert_eq!(branches.len(), 6);
        assert!(branches.contains(&Some("X")));
        assert!(branches.contains(&Some("Y")));
        assert!(branches.contains(&Some("Z")));

        assert!(execute_op("BOGUS", &[], &[], &ctx, &mut exec, &mut ScriptRng::new(vec![0])).is_err());

        assert_eq!(qubits(&[StimTarget::Qubit(0), StimTarget::Sweep(1)]).unwrap(), vec![0]);
        assert!(qubits(&[StimTarget::Pauli {
            qubit: 0,
            basis: PauliBasis::X,
            inverted: false,
        }])
        .is_err());

        assert_eq!(
            qubit_slots(&[StimTarget::Qubit(0), StimTarget::Sweep(1)]).unwrap(),
            vec![(0, 0)]
        );
        assert!(qubit_slots(&[StimTarget::QubitInv(0)]).is_err());

        assert_eq!(
            qubits_with_inversion(&[StimTarget::Qubit(0), StimTarget::Sweep(1)]).unwrap(),
            vec![(0, false)]
        );
        assert!(qubits_with_inversion(&[StimTarget::Pauli {
            qubit: 0,
            basis: PauliBasis::X,
            inverted: false,
        }])
        .is_err());

        assert_eq!(
            qubits_with_inversion_slots(&[StimTarget::Sweep(0), StimTarget::QubitInv(1)]).unwrap(),
            vec![(1, 1, true)]
        );
        assert!(qubits_with_inversion_slots(&[StimTarget::Pauli {
            qubit: 0,
            basis: PauliBasis::X,
            inverted: false,
        }])
        .is_err());

        assert!(qubits_with_inversion_pairs(&[StimTarget::Qubit(0)]).is_err());

        assert_eq!(expect_qubit(&StimTarget::Qubit(0)).unwrap(), 0);
        assert!(expect_qubit(&StimTarget::QubitInv(0)).is_err());
        assert!(expect_qubit(&StimTarget::Sweep(0)).is_err());
        assert!(expect_qubit(&StimTarget::Pauli {
            qubit: 0,
            basis: PauliBasis::X,
            inverted: false,
        })
        .is_err());

        assert!(qubit_pairs(&[StimTarget::Qubit(0)]).is_err());
        assert_eq!(
            qubit_pairs(&[StimTarget::Sweep(0), StimTarget::Qubit(1)]).unwrap(),
            Vec::<(usize, usize)>::new()
        );

        assert!(present_qubit_pairs(&[StimTarget::Qubit(0), StimTarget::Qubit(1)], &[false, true])
            .map(|pairs| pairs.is_empty())
            .unwrap());
        assert!(present_qubit_pair_slots(&[StimTarget::Qubit(0)] , &[false]).is_err());
        assert_eq!(
            present_qubit_pair_slots(&[StimTarget::Sweep(0), StimTarget::Qubit(1)], &[false, false])
                .unwrap(),
            Vec::<((usize, usize), (usize, usize))>::new()
        );

        let recorder = Recorder::default();
        assert!(xor_recs(&recorder, &[StimTarget::Qubit(0)]).is_err());
        assert!(apply_pauli_targets(
            &mut exec.state,
            &[StimTarget::Qubit(0)]
        )
        .is_err());
        assert_eq!(
            correlated_trace_payload(&[StimTarget::Qubit(0)]).unwrap(),
            None
        );
        assert_eq!(pauli_pair_label(4, 4), "??");
        apply_pauli(&mut exec.state, 0, 4);
        assert!(measure_pauli_product(&mut exec.state, &[], true, &mut ScriptRng::new(vec![0])));
        apply_spp(&mut exec.state, &[], false, false);
    }

    #[test]
    fn reference_controlled_pairs_cover_sweep_and_error_paths() {
        let mut state = StabilizerState::new(2);
        let sweep_bits = [true];

        assert!(apply_reference_controlled_pairs(
            &mut state,
            "CX",
            &[StimTarget::Qubit(0)],
            None,
        )
        .is_err());

        assert!(apply_reference_controlled_pairs(
            &mut state,
            "BAD",
            &[StimTarget::Qubit(0), StimTarget::Qubit(1)],
            None,
        )
        .is_err());

        assert!(apply_reference_controlled_pairs(
            &mut state,
            "BAD",
            &[StimTarget::Sweep(0), StimTarget::Qubit(1)],
            Some(&sweep_bits),
        )
        .is_err());

        assert!(apply_reference_controlled_pairs(
            &mut state,
            "CX",
            &[StimTarget::Qubit(0), StimTarget::Sweep(0)],
            Some(&sweep_bits),
        )
        .is_err());

        assert!(apply_reference_controlled_pairs(
            &mut state,
            "CX",
            &[StimTarget::Pauli {
                qubit: 0,
                basis: PauliBasis::X,
                inverted: false,
            }, StimTarget::Qubit(1)],
            None,
        )
        .is_err());

        assert!(apply_reference_controlled_pairs(
            &mut state,
            "CX",
            &[StimTarget::Qubit(0), StimTarget::Qubit(1), StimTarget::Qubit(0)],
            None,
        )
        .is_err());
    }
}
