use std::collections::BTreeMap;

use rand::{Rng, RngCore};

use crate::coords::CoordState;
use crate::interactive_shot::{
    ChoiceKind, CircuitDigest, KeyedRng, NoiseEventId, NoiseOutcome, NoiseSiteId, Pauli, RandomKey,
};
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
    pub observable_events: Vec<ObservableEvent>,
    pub inapplicable_noise_events: Vec<NoiseApplicabilityEvent>,
    pub qubit_coords: std::collections::HashMap<u32, Vec<f64>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoiseApplicabilityEvent {
    pub op_path: Vec<usize>,
    pub repeat_iterations: Vec<u64>,
    pub target_slots: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservableEvent {
    pub op_path: Vec<usize>,
    pub repeat_iterations: Vec<u64>,
    pub sequence_index: usize,
    pub observable_index: u32,
    pub bit: bool,
}

impl Executor {
    pub fn from_instrs(instrs: Vec<StimInstr>) -> Result<Self, String> {
        crate::validation::validate_circuit(&instrs)?;
        Ok(Self { instrs })
    }

    pub fn run(&mut self, rng: &mut impl Rng) -> Result<ExecOutput, String> {
        let (out, _) = self.run_internal(rng, false, None)?;
        Ok(out)
    }

    pub fn run_with_sweep_bits(
        &mut self,
        rng: &mut impl Rng,
        sweep_bits: Option<&[bool]>,
    ) -> Result<ExecOutput, String> {
        let (out, _) = self.run_internal(rng, false, sweep_bits)?;
        Ok(out)
    }

    pub fn run_with_trace(
        &mut self,
        rng: &mut impl Rng,
    ) -> Result<(ExecOutput, SampleTrace), String> {
        self.run_internal(rng, true, None)
    }

    fn run_internal(
        &self,
        rng: &mut impl Rng,
        trace_enabled: bool,
        sweep_bits: Option<&[bool]>,
    ) -> Result<(ExecOutput, SampleTrace), String> {
        let mut randomness = ExecutionRandom::Sequential(rng);
        self.run_internal_with_randomness(&mut randomness, trace_enabled, sweep_bits)
    }

    pub fn run_with_choices(
        &self,
        config: InteractiveExecutionConfig<'_>,
    ) -> Result<(ExecOutput, SampleTrace), String> {
        let mut randomness = ExecutionRandom::interactive(config);
        self.run_internal_with_randomness(&mut randomness, true, None)
    }

    fn run_internal_with_randomness(
        &self,
        randomness: &mut ExecutionRandom<'_>,
        trace_enabled: bool,
        sweep_bits: Option<&[bool]>,
    ) -> Result<(ExecOutput, SampleTrace), String> {
        let n = max_qubit(&self.instrs)?;
        let mut exec = ExecutionState::new(n, trace_enabled);
        execute_instrs(
            &self.instrs,
            &ExecutionTraversalContext::default(),
            &mut exec,
            randomness,
            sweep_bits,
        )?;
        Ok(exec.into_output())
    }
}

#[derive(Debug, Clone, Copy)]
pub struct InteractiveExecutionConfig<'a> {
    pub circuit_digest: CircuitDigest,
    pub seed: u64,
    pub force_noiseless: bool,
    pub overrides: &'a BTreeMap<NoiseEventId, NoiseOutcome>,
}

struct InteractiveRandom<'a> {
    circuit_digest: CircuitDigest,
    force_noiseless: bool,
    overrides: &'a BTreeMap<NoiseEventId, NoiseOutcome>,
    keyed: KeyedRng,
}

enum ExecutionRandom<'a> {
    Sequential(&'a mut dyn RngCore),
    Interactive(InteractiveRandom<'a>),
}

impl<'a> ExecutionRandom<'a> {
    fn interactive(config: InteractiveExecutionConfig<'a>) -> Self {
        let initial_key = RandomKey {
            circuit_digest: config.circuit_digest,
            op_path: Vec::new(),
            repeat_iterations: Vec::new(),
            target_slots: Vec::new(),
            choice_kind: ChoiceKind::IntrinsicMeasurement,
            subchoice: 0,
        };
        Self::Interactive(InteractiveRandom {
            circuit_digest: config.circuit_digest,
            force_noiseless: config.force_noiseless,
            overrides: config.overrides,
            keyed: KeyedRng::new(config.seed, initial_key),
        })
    }

    fn set_key(
        &mut self,
        context: &ExecutionTraversalContext,
        target_slots: &[usize],
        choice_kind: ChoiceKind,
        subchoice: u16,
    ) {
        if let Self::Interactive(random) = self {
            random.keyed.set_key(RandomKey {
                circuit_digest: random.circuit_digest,
                op_path: context.op_path.clone(),
                repeat_iterations: context.repeat_iterations.clone(),
                target_slots: target_slots.to_vec(),
                choice_kind,
                subchoice,
            });
        }
    }

    fn bernoulli(
        &mut self,
        context: &ExecutionTraversalContext,
        target_slots: &[usize],
        choice_kind: ChoiceKind,
        subchoice: u16,
        probability: f64,
        declared_noise: bool,
    ) -> bool {
        self.unit_interval(
            context,
            target_slots,
            choice_kind,
            subchoice,
            declared_noise,
        ) < probability
    }

    fn unit_interval(
        &mut self,
        context: &ExecutionTraversalContext,
        target_slots: &[usize],
        choice_kind: ChoiceKind,
        subchoice: u16,
        declared_noise: bool,
    ) -> f64 {
        if declared_noise
            && matches!(
                self,
                Self::Interactive(InteractiveRandom {
                    force_noiseless: true,
                    ..
                })
            )
        {
            return 1.0;
        }
        self.set_key(context, target_slots, choice_kind, subchoice);
        self.r#gen::<f64>()
    }

    fn prepare_uniform(
        &mut self,
        context: &ExecutionTraversalContext,
        target_slots: &[usize],
        choice_kind: ChoiceKind,
        subchoice: u16,
    ) {
        self.set_key(context, target_slots, choice_kind, subchoice);
    }

    fn uniform_i32(
        &mut self,
        context: &ExecutionTraversalContext,
        target_slots: &[usize],
        choice_kind: ChoiceKind,
        subchoice: u16,
        upper: i32,
    ) -> i32 {
        self.prepare_uniform(context, target_slots, choice_kind, subchoice);
        self.gen_range(0..upper)
    }

    fn uniform_u32(
        &mut self,
        context: &ExecutionTraversalContext,
        target_slots: &[usize],
        choice_kind: ChoiceKind,
        subchoice: u16,
        upper: u32,
    ) -> u32 {
        assert!(upper > 0, "uniform upper bound must be positive");
        self.prepare_uniform(context, target_slots, choice_kind, subchoice);
        match self {
            // Preserve the established native seeded sequence for non-interactive callers.
            Self::Sequential(rng) => rng.gen_range(0..upper as usize) as u32,
            Self::Interactive(random) => bounded_u32(&mut random.keyed, upper),
        }
    }

    fn uniform_u8(
        &mut self,
        context: &ExecutionTraversalContext,
        target_slots: &[usize],
        choice_kind: ChoiceKind,
        subchoice: u16,
        upper: u8,
    ) -> u8 {
        self.prepare_uniform(context, target_slots, choice_kind, subchoice);
        self.gen_range(0..upper)
    }

    fn prepare_intrinsic(
        &mut self,
        context: &ExecutionTraversalContext,
        target_slots: &[usize],
        subchoice: u16,
    ) {
        self.set_key(
            context,
            target_slots,
            ChoiceKind::IntrinsicMeasurement,
            subchoice,
        );
    }

    fn override_outcome(
        &self,
        context: &ExecutionTraversalContext,
        target_slots: &[usize],
    ) -> Option<NoiseOutcome> {
        let Self::Interactive(random) = self else {
            return None;
        };
        let id = NoiseEventId {
            site: NoiseSiteId {
                circuit_digest: random.circuit_digest,
                op_path: context.op_path.clone(),
                target_slots: target_slots.to_vec(),
            },
            repeat_iterations: context.repeat_iterations.clone(),
        };
        random.overrides.get(&id).copied()
    }
}

fn bounded_u32(rng: &mut impl RngCore, upper: u32) -> u32 {
    let threshold = upper.wrapping_neg() % upper;
    loop {
        let value = rng.next_u32();
        if value >= threshold {
            return value % upper;
        }
    }
}

impl RngCore for ExecutionRandom<'_> {
    fn next_u32(&mut self) -> u32 {
        match self {
            Self::Sequential(rng) => rng.next_u32(),
            Self::Interactive(random) => random.keyed.next_u32(),
        }
    }

    fn next_u64(&mut self) -> u64 {
        match self {
            Self::Sequential(rng) => rng.next_u64(),
            Self::Interactive(random) => random.keyed.next_u64(),
        }
    }

    fn fill_bytes(&mut self, destination: &mut [u8]) {
        match self {
            Self::Sequential(rng) => rng.fill_bytes(destination),
            Self::Interactive(random) => random.keyed.fill_bytes(destination),
        }
    }

    fn try_fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), rand::Error> {
        match self {
            Self::Sequential(rng) => rng.try_fill_bytes(destination),
            Self::Interactive(random) => random.keyed.try_fill_bytes(destination),
        }
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
    observable_events: Vec<ObservableEvent>,
    inapplicable_noise_events: Vec<NoiseApplicabilityEvent>,
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
            observable_events: Vec::new(),
            inapplicable_noise_events: Vec::new(),
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
                observable_events: self.observable_events,
                inapplicable_noise_events: self.inapplicable_noise_events,
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

    fn record_inapplicable_noise_event(
        &mut self,
        context: &ExecutionTraversalContext,
        target_slots: &[usize],
    ) {
        if self.trace_enabled {
            self.inapplicable_noise_events
                .push(NoiseApplicabilityEvent {
                    op_path: context.op_path.clone(),
                    repeat_iterations: context.repeat_iterations.clone(),
                    target_slots: target_slots.to_vec(),
                });
        }
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

    fn record_observable_event(
        &mut self,
        context: &ExecutionTraversalContext,
        sequence_index: usize,
        observable_index: u32,
        bit: bool,
    ) {
        if !self.trace_enabled {
            return;
        }
        self.observable_events.push(ObservableEvent {
            op_path: context.op_path.clone(),
            repeat_iterations: context.repeat_iterations.clone(),
            sequence_index,
            observable_index,
            bit,
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

fn execute_instrs(
    instrs: &[StimInstr],
    context: &ExecutionTraversalContext,
    exec: &mut ExecutionState,
    rng: &mut ExecutionRandom<'_>,
    sweep_bits: Option<&[bool]>,
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
                execute_op(name, args, targets, &op_context, exec, rng, sweep_bits)?;
            }
            StimInstr::Repeat { count, body } => {
                for iteration in 0..*count {
                    let repeat_context = context.with_repeat_iteration(op_index, iteration);
                    execute_instrs(body, &repeat_context, exec, rng, sweep_bits)?;
                }
            }
        }
    }
    Ok(())
}

fn execute_op(
    name: &str,
    args: &[f64],
    targets: &[StimTarget],
    context: &ExecutionTraversalContext,
    exec: &mut ExecutionState,
    rng: &mut ExecutionRandom<'_>,
    sweep_bits: Option<&[bool]>,
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
            apply_runtime_controlled_pairs(
                &mut exec.state,
                &exec.lost,
                &exec.recorder,
                targets,
                sweep_bits,
                |state, c, t| state.cx(c, t),
                |state, q| state.x_gate(q),
            )?;
        }
        "CY" | "ZCY" => {
            apply_runtime_controlled_pairs(
                &mut exec.state,
                &exec.lost,
                &exec.recorder,
                targets,
                sweep_bits,
                |state, c, t| state.cy(c, t),
                |state, q| state.y_gate(q),
            )?;
        }
        "CZ" | "ZCZ" => {
            apply_runtime_controlled_pairs(
                &mut exec.state,
                &exec.lost,
                &exec.recorder,
                targets,
                sweep_bits,
                |state, c, t| state.cz(c, t),
                |state, q| state.z_gate(q),
            )?;
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
                rng.prepare_intrinsic(context, &[target_slot], 0);
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
                rng.prepare_intrinsic(context, &[target_slot], 0);
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
                rng.prepare_intrinsic(context, &[target_slot], 0);
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
                rng.prepare_intrinsic(context, &[target_slot], 0);
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
                rng.prepare_intrinsic(context, &[target_slot], 0);
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
                rng.prepare_intrinsic(context, &[target_slot], 0);
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
                rng.prepare_intrinsic(context, &[target_slot], 0);
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
                rng.prepare_intrinsic(context, &[target_slot], 0);
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
                rng.prepare_intrinsic(context, &[target_slot], 0);
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
                rng.prepare_intrinsic(context, &[target_slot], 0);
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
                rng.prepare_intrinsic(context, &[target_slot], 0);
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
                rng.prepare_intrinsic(context, &[target_slot], 0);
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
            for (target_slot, t) in targets.iter().enumerate() {
                let q = expect_qubit(t)?;
                let mut bit = q != 0;
                let flipped = p > 0.0
                    && rng.bernoulli(
                        context,
                        &[target_slot],
                        ChoiceKind::MeasurementFlip,
                        0,
                        p,
                        true,
                    );
                if flipped {
                    bit = !bit;
                    exec.record_noise_event(
                        context,
                        name,
                        vec![target_slot],
                        vec![q as u32],
                        "flip",
                    );
                }
                exec.recorder.push(bit);
                exec.record_measurement_event(
                    context,
                    name,
                    target_slot,
                    q as u32,
                    exec.recorder.len(),
                    MeasurementOutcome {
                        bit,
                        loss_cause: false,
                    },
                    MeasurementComponent::Value,
                );
            }
        }
        "R" | "RZ" => {
            for (target_slot, q) in qubit_slots(targets)? {
                exec.lost[q] = false;
                rng.prepare_intrinsic(context, &[target_slot], 0);
                exec.state.reset_z(q, rng);
            }
        }
        "RX" => {
            for (target_slot, q) in qubit_slots(targets)? {
                exec.lost[q] = false;
                rng.prepare_intrinsic(context, &[target_slot], 0);
                exec.state.reset_x(q, rng);
            }
        }
        "RY" => {
            for (target_slot, q) in qubit_slots(targets)? {
                exec.lost[q] = false;
                rng.prepare_intrinsic(context, &[target_slot], 0);
                exec.state.reset_y(q, rng);
            }
        }
        "LOSS" => {
            let p = args.first().copied().unwrap_or(0.0);
            for (target_slot, q) in qubit_slots(targets)? {
                let target_slots = [target_slot];
                let outcome = rng
                    .override_outcome(context, &target_slots)
                    .unwrap_or_else(|| {
                        if rng.bernoulli(
                            context,
                            &target_slots,
                            ChoiceKind::NoiseOccurrence,
                            0,
                            p,
                            true,
                        ) {
                            NoiseOutcome::Lost
                        } else {
                            NoiseOutcome::Identity
                        }
                    });
                if outcome == NoiseOutcome::Lost {
                    exec.lost[q] = true;
                    exec.record_noise_event(context, name, vec![target_slot], vec![q as u32], "L");
                } else if outcome != NoiseOutcome::Identity {
                    return Err(format!("invalid override {} for LOSS", outcome.label()));
                }
            }
        }
        "X_ERROR" => {
            let p = args.first().copied().unwrap_or(0.0);
            for (target_slot, q) in qubit_slots(targets)? {
                let target_slots = [target_slot];
                if exec.lost[q] {
                    exec.record_inapplicable_noise_event(context, &target_slots);
                    if let Some(outcome) = rng.override_outcome(context, &target_slots)
                        && !matches!(outcome, NoiseOutcome::Identity | NoiseOutcome::X)
                    {
                        return Err(format!("invalid override {} for X_ERROR", outcome.label()));
                    }
                    continue;
                }
                let outcome = rng
                    .override_outcome(context, &target_slots)
                    .unwrap_or_else(|| {
                        if rng.bernoulli(
                            context,
                            &target_slots,
                            ChoiceKind::NoiseOccurrence,
                            0,
                            p,
                            true,
                        ) {
                            NoiseOutcome::X
                        } else {
                            NoiseOutcome::Identity
                        }
                    });
                if outcome == NoiseOutcome::X {
                    exec.state.x_gate(q);
                    exec.record_noise_event(context, name, vec![target_slot], vec![q as u32], "X");
                } else if !matches!(outcome, NoiseOutcome::Identity | NoiseOutcome::X) {
                    return Err(format!("invalid override {} for X_ERROR", outcome.label()));
                }
            }
        }
        "Y_ERROR" => {
            let p = args.first().copied().unwrap_or(0.0);
            for (target_slot, q) in qubit_slots(targets)? {
                let target_slots = [target_slot];
                if exec.lost[q] {
                    exec.record_inapplicable_noise_event(context, &target_slots);
                    if let Some(outcome) = rng.override_outcome(context, &target_slots)
                        && !matches!(outcome, NoiseOutcome::Identity | NoiseOutcome::Y)
                    {
                        return Err(format!("invalid override {} for Y_ERROR", outcome.label()));
                    }
                    continue;
                }
                let outcome = rng
                    .override_outcome(context, &target_slots)
                    .unwrap_or_else(|| {
                        if rng.bernoulli(
                            context,
                            &target_slots,
                            ChoiceKind::NoiseOccurrence,
                            0,
                            p,
                            true,
                        ) {
                            NoiseOutcome::Y
                        } else {
                            NoiseOutcome::Identity
                        }
                    });
                if outcome == NoiseOutcome::Y {
                    exec.state.y_gate(q);
                    exec.record_noise_event(context, name, vec![target_slot], vec![q as u32], "Y");
                } else if !matches!(outcome, NoiseOutcome::Identity | NoiseOutcome::Y) {
                    return Err(format!("invalid override {} for Y_ERROR", outcome.label()));
                }
            }
        }
        "Z_ERROR" => {
            let p = args.first().copied().unwrap_or(0.0);
            for (target_slot, q) in qubit_slots(targets)? {
                let target_slots = [target_slot];
                if exec.lost[q] {
                    exec.record_inapplicable_noise_event(context, &target_slots);
                    if let Some(outcome) = rng.override_outcome(context, &target_slots)
                        && !matches!(outcome, NoiseOutcome::Identity | NoiseOutcome::Z)
                    {
                        return Err(format!("invalid override {} for Z_ERROR", outcome.label()));
                    }
                    continue;
                }
                let outcome = rng
                    .override_outcome(context, &target_slots)
                    .unwrap_or_else(|| {
                        if rng.bernoulli(
                            context,
                            &target_slots,
                            ChoiceKind::NoiseOccurrence,
                            0,
                            p,
                            true,
                        ) {
                            NoiseOutcome::Z
                        } else {
                            NoiseOutcome::Identity
                        }
                    });
                if outcome == NoiseOutcome::Z {
                    exec.state.z_gate(q);
                    exec.record_noise_event(context, name, vec![target_slot], vec![q as u32], "Z");
                } else if !matches!(outcome, NoiseOutcome::Identity | NoiseOutcome::Z) {
                    return Err(format!("invalid override {} for Z_ERROR", outcome.label()));
                }
            }
        }
        "DEPOLARIZE1" => {
            let p = args.first().copied().unwrap_or(0.0);
            for (target_slot, q) in qubit_slots(targets)? {
                let target_slots = [target_slot];
                if exec.lost[q] {
                    exec.record_inapplicable_noise_event(context, &target_slots);
                    if let Some(outcome) = rng.override_outcome(context, &target_slots)
                        && !matches!(
                            outcome,
                            NoiseOutcome::Identity
                                | NoiseOutcome::X
                                | NoiseOutcome::Y
                                | NoiseOutcome::Z
                        )
                    {
                        return Err(format!(
                            "invalid override {} for DEPOLARIZE1",
                            outcome.label()
                        ));
                    }
                    continue;
                }
                let outcome = rng
                    .override_outcome(context, &target_slots)
                    .unwrap_or_else(|| {
                        if !rng.bernoulli(
                            context,
                            &target_slots,
                            ChoiceKind::NoiseOccurrence,
                            0,
                            p,
                            true,
                        ) {
                            NoiseOutcome::Identity
                        } else {
                            match rng.uniform_i32(
                                context,
                                &target_slots,
                                ChoiceKind::NoiseBranch,
                                0,
                                3,
                            ) {
                                0 => NoiseOutcome::X,
                                1 => NoiseOutcome::Y,
                                _ => NoiseOutcome::Z,
                            }
                        }
                    });
                let branch = match outcome {
                    NoiseOutcome::Identity => None,
                    NoiseOutcome::X => Some({
                        exec.state.x_gate(q);
                        "X"
                    }),
                    NoiseOutcome::Y => Some({
                        exec.state.y_gate(q);
                        "Y"
                    }),
                    NoiseOutcome::Z => Some({
                        exec.state.z_gate(q);
                        "Z"
                    }),
                    _ => {
                        return Err(format!(
                            "invalid override {} for DEPOLARIZE1",
                            outcome.label()
                        ));
                    }
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
        "DEPOLARIZE2" => {
            let p = args.first().copied().unwrap_or(0.0);
            for ((slot_a, a), (slot_b, b)) in qubit_pair_slots(targets)? {
                let target_slots = [slot_a, slot_b];
                if exec.lost[a] || exec.lost[b] {
                    exec.record_inapplicable_noise_event(context, &target_slots);
                    if let Some(outcome) = rng.override_outcome(context, &target_slots)
                        && !matches!(
                            outcome,
                            NoiseOutcome::Identity | NoiseOutcome::PauliPair { .. }
                        )
                    {
                        return Err(format!(
                            "invalid override {} for DEPOLARIZE2",
                            outcome.label()
                        ));
                    }
                    continue;
                }
                let outcome = rng
                    .override_outcome(context, &target_slots)
                    .unwrap_or_else(|| {
                        if !rng.bernoulli(
                            context,
                            &target_slots,
                            ChoiceKind::NoiseOccurrence,
                            0,
                            p,
                            true,
                        ) {
                            NoiseOutcome::Identity
                        } else {
                            let (pa, pb) = two_qubit_pauli(rng.uniform_u32(
                                context,
                                &target_slots,
                                ChoiceKind::NoiseBranch,
                                0,
                                15,
                            ) as usize);
                            NoiseOutcome::PauliPair {
                                first: pauli_from_code(pa),
                                second: pauli_from_code(pb),
                            }
                        }
                    });
                let pair = match outcome {
                    NoiseOutcome::Identity => None,
                    NoiseOutcome::PauliPair { first, second } => {
                        Some((pauli_code(first), pauli_code(second)))
                    }
                    _ => {
                        return Err(format!(
                            "invalid override {} for DEPOLARIZE2",
                            outcome.label()
                        ));
                    }
                };
                if let Some((pa, pb)) = pair {
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
            let sequence_index = exec.observables.len();
            exec.observables.push((index, bit));
            exec.record_observable_event(context, sequence_index, index, bit);
        }
        "MXX" => {
            let p = args.first().copied().unwrap_or(0.0);
            pair_measure(exec, name, targets, PauliBasis::X, p, context, rng)?;
        }
        "MYY" => {
            let p = args.first().copied().unwrap_or(0.0);
            pair_measure(exec, name, targets, PauliBasis::Y, p, context, rng)?;
        }
        "MZZ" => {
            let p = args.first().copied().unwrap_or(0.0);
            pair_measure(exec, name, targets, PauliBasis::Z, p, context, rng)?;
        }
        "MPP" => {
            let p = args.first().copied().unwrap_or(0.0);
            let products = split_pauli_products(targets)?;
            for (product_index, product) in products.iter().enumerate() {
                let target_slots = [product_index];
                let mut bit = if product.terms.iter().any(|(q, _)| exec.lost[*q]) {
                    true
                } else {
                    rng.prepare_intrinsic(context, &target_slots, 0);
                    measure_pauli_product(&mut exec.state, &product.terms, product.inverted, rng)
                };
                let flipped = p > 0.0
                    && rng.bernoulli(
                        context,
                        &target_slots,
                        ChoiceKind::MeasurementFlip,
                        0,
                        p,
                        true,
                    );
                if flipped {
                    bit = !bit;
                    exec.record_noise_event(
                        context,
                        name,
                        target_slots.to_vec(),
                        product
                            .terms
                            .iter()
                            .map(|(qubit, _)| *qubit as u32)
                            .collect(),
                        "flip",
                    );
                }
                exec.recorder.push(bit);
                let loss_cause = product.terms.iter().any(|(q, _)| exec.lost[*q]);
                let target_qubit = product.terms.first().map_or(0, |(q, _)| *q as u32);
                exec.record_measurement_event(
                    context,
                    name,
                    product_index,
                    target_qubit,
                    exec.recorder.len(),
                    MeasurementOutcome { bit, loss_cause },
                    MeasurementComponent::Value,
                );
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
                    exec.record_inapplicable_noise_event(context, &[target_slot]);
                    continue;
                }
                let r =
                    rng.unit_interval(context, &[target_slot], ChoiceKind::NoiseBranch, 0, true);
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
            for ((slot_a, a), (slot_b, b)) in qubit_pair_slots(targets)? {
                if exec.lost[a] || exec.lost[b] {
                    exec.record_inapplicable_noise_event(context, &[slot_a, slot_b]);
                    continue;
                }
                let r =
                    rng.unit_interval(context, &[slot_a, slot_b], ChoiceKind::NoiseBranch, 0, true);
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
            for (target_slot, q) in qubit_slots(targets)? {
                let heralded = p > 0.0
                    && rng.bernoulli(context, &[target_slot], ChoiceKind::Herald, 0, p, true);
                if heralded {
                    let branch = match rng.uniform_u8(
                        context,
                        &[target_slot],
                        ChoiceKind::NoiseBranch,
                        0,
                        4,
                    ) {
                        1 => {
                            exec.state.x_gate(q);
                            Some("X")
                        }
                        2 => {
                            exec.state.y_gate(q);
                            Some("Y")
                        }
                        3 => {
                            exec.state.z_gate(q);
                            Some("Z")
                        }
                        _ => None,
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
                exec.recorder.push(heralded);
                exec.record_measurement_event(
                    context,
                    name,
                    target_slot,
                    q as u32,
                    exec.recorder.len(),
                    MeasurementOutcome {
                        bit: heralded,
                        loss_cause: false,
                    },
                    MeasurementComponent::Value,
                );
            }
        }
        "HERALDED_PAULI_CHANNEL_1" => {
            let pi = args.first().copied().unwrap_or(0.0);
            let px = args.get(1).copied().unwrap_or(0.0);
            let py = args.get(2).copied().unwrap_or(0.0);
            let pz = args.get(3).copied().unwrap_or(0.0);
            let total = pi + px + py + pz;
            for (target_slot, q) in qubit_slots(targets)? {
                let r = rng.unit_interval(context, &[target_slot], ChoiceKind::Herald, 0, true);
                let heralded = r < total;
                if heralded {
                    let inner = r;
                    let branch = if inner < pi {
                        None
                    } else if inner < pi + px {
                        exec.state.x_gate(q);
                        Some("X")
                    } else if inner < pi + px + py {
                        exec.state.y_gate(q);
                        Some("Y")
                    } else {
                        exec.state.z_gate(q);
                        Some("Z")
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
                exec.recorder.push(heralded);
                exec.record_measurement_event(
                    context,
                    name,
                    target_slot,
                    q as u32,
                    exec.recorder.len(),
                    MeasurementOutcome {
                        bit: heralded,
                        loss_cause: false,
                    },
                    MeasurementComponent::Value,
                );
            }
        }
        "CORRELATED_ERROR" | "E" => {
            let p = args.first().copied().unwrap_or(0.0);
            let payload = correlated_trace_payload(targets)?;
            let target_slots = payload
                .as_ref()
                .map(|(slots, _, _)| slots.as_slice())
                .unwrap_or(&[]);
            if p > 0.0
                && rng.bernoulli(
                    context,
                    target_slots,
                    ChoiceKind::CorrelatedOccurrence,
                    0,
                    p,
                    true,
                )
            {
                apply_pauli_targets(&mut exec.state, targets)?;
                if let Some((target_slots, target_qubits, label)) = payload {
                    exec.record_noise_event(context, name, target_slots, target_qubits, &label);
                }
                exec.last_correlated_error_occurred = true;
            } else {
                exec.last_correlated_error_occurred = false;
            }
        }
        "ELSE_CORRELATED_ERROR" => {
            if exec.last_correlated_error_occurred {
                let payload = correlated_trace_payload(targets)?;
                let target_slots = payload
                    .as_ref()
                    .map(|(slots, _, _)| slots.as_slice())
                    .unwrap_or(&[]);
                exec.record_inapplicable_noise_event(context, target_slots);
            } else {
                let p = args.first().copied().unwrap_or(0.0);
                let payload = correlated_trace_payload(targets)?;
                let target_slots = payload
                    .as_ref()
                    .map(|(slots, _, _)| slots.as_slice())
                    .unwrap_or(&[]);
                if p > 0.0
                    && rng.bernoulli(
                        context,
                        target_slots,
                        ChoiceKind::CorrelatedOccurrence,
                        0,
                        p,
                        true,
                    )
                {
                    apply_pauli_targets(&mut exec.state, targets)?;
                    if let Some((target_slots, target_qubits, label)) = payload {
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
            apply_reference_controlled_pairs(state, lost, measurements, name, targets, sweep_bits)?;
        }
        "CY" | "ZCY" => {
            apply_reference_controlled_pairs(state, lost, measurements, name, targets, sweep_bits)?;
        }
        "CZ" | "ZCZ" => {
            apply_reference_controlled_pairs(state, lost, measurements, name, targets, sweep_bits)?;
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
    lost: &[bool],
    measurements: &[bool],
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
            (StimTarget::Qubit(c), StimTarget::Qubit(t)) => {
                let c = *c as usize;
                let t = *t as usize;
                if lost[c] || lost[t] {
                    continue;
                }
                match name {
                    "CX" | "CNOT" | "ZCX" => state.cx(c, t),
                    "CY" | "ZCY" => state.cy(c, t),
                    "CZ" | "ZCZ" => state.cz(c, t),
                    _ => return Err(format!("unsupported reference pair op {name}")),
                }
            }
            (StimTarget::Rec(offset), StimTarget::Qubit(q)) => {
                let q = *q as usize;
                if lost[q] {
                    continue;
                }
                let active = rec_from_measurements(measurements, *offset)?;
                if active {
                    apply_reference_feedback_gate(state, name, q)?;
                }
            }
            (StimTarget::Sweep(k), StimTarget::Qubit(q)) => {
                let q = *q as usize;
                if lost[q] {
                    continue;
                }
                let active = sweep_bits
                    .and_then(|bits| bits.get(*k as usize))
                    .copied()
                    .unwrap_or(false);
                if active {
                    apply_reference_feedback_gate(state, name, q)?;
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

fn rec_from_measurements(measurements: &[bool], offset: i32) -> Result<bool, String> {
    if offset >= 0 {
        return Err("rec out of range".to_string());
    }
    let idx = measurements.len() as i32 + offset;
    if idx < 0 {
        return Err("rec out of range".to_string());
    }
    measurements
        .get(idx as usize)
        .copied()
        .ok_or_else(|| "rec out of range".to_string())
}

fn apply_reference_feedback_gate(
    state: &mut StabilizerState,
    name: &str,
    q: usize,
) -> Result<(), String> {
    match name {
        "CX" | "CNOT" | "ZCX" => state.x_gate(q),
        "CY" | "ZCY" => state.y_gate(q),
        "CZ" | "ZCZ" => state.z_gate(q),
        _ => return Err(format!("unsupported reference pair op {name}")),
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

fn apply_runtime_controlled_pairs<FPair, FSingle>(
    state: &mut StabilizerState,
    lost: &[bool],
    recorder: &Recorder,
    targets: &[StimTarget],
    sweep_bits: Option<&[bool]>,
    mut apply_pair: FPair,
    mut apply_single: FSingle,
) -> Result<(), String>
where
    FPair: FnMut(&mut StabilizerState, usize, usize),
    FSingle: FnMut(&mut StabilizerState, usize),
{
    if targets.len() % 2 != 0 {
        return Err("odd number of targets".to_string());
    }
    let mut it = targets.iter();
    while let (Some(control), Some(target)) = (it.next(), it.next()) {
        match (control, target) {
            (StimTarget::Qubit(c), StimTarget::Qubit(t)) => {
                let c = *c as usize;
                let t = *t as usize;
                if !lost[c] && !lost[t] {
                    apply_pair(state, c, t);
                }
            }
            (StimTarget::Rec(offset), StimTarget::Qubit(t)) => {
                let t = *t as usize;
                if lost[t] {
                    continue;
                }
                let active = recorder.rec(*offset).ok_or("rec out of range")?;
                if active {
                    apply_single(state, t);
                }
            }
            (StimTarget::Sweep(k), StimTarget::Qubit(t)) => {
                let t = *t as usize;
                if lost[t] {
                    continue;
                }
                let active = sweep_bits
                    .and_then(|bits| bits.get(*k as usize))
                    .copied()
                    .unwrap_or(false);
                if active {
                    apply_single(state, t);
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

fn qubit_pair_slots(
    targets: &[StimTarget],
) -> Result<Vec<((usize, usize), (usize, usize))>, String> {
    if targets.len() % 2 != 0 {
        return Err("odd number of targets".to_string());
    }
    let mut out = Vec::new();
    let mut iterator = targets.iter().enumerate();
    while let (Some((slot_a, a)), Some((slot_b, b))) = (iterator.next(), iterator.next()) {
        if matches!(a, StimTarget::Sweep(_)) || matches!(b, StimTarget::Sweep(_)) {
            continue;
        }
        out.push(((slot_a, expect_qubit(a)?), (slot_b, expect_qubit(b)?)));
    }
    Ok(out)
}

#[cfg(test)]
fn present_qubit_pair_slots(
    targets: &[StimTarget],
    lost: &[bool],
) -> Result<Vec<((usize, usize), (usize, usize))>, String> {
    Ok(qubit_pair_slots(targets)?
        .into_iter()
        .filter(|((_, a), (_, b))| !lost[*a] && !lost[*b])
        .collect())
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

fn pauli_code(pauli: Pauli) -> u8 {
    match pauli {
        Pauli::I => 0,
        Pauli::X => 1,
        Pauli::Y => 2,
        Pauli::Z => 3,
    }
}

fn pauli_from_code(code: u8) -> Pauli {
    match code {
        0 => Pauli::I,
        1 => Pauli::X,
        2 => Pauli::Y,
        3 => Pauli::Z,
        _ => unreachable!("two-qubit Pauli code must be in 0..=3"),
    }
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
    exec: &mut ExecutionState,
    instr_name: &str,
    targets: &[StimTarget],
    basis: PauliBasis,
    noise_p: f64,
    context: &ExecutionTraversalContext,
    rng: &mut ExecutionRandom<'_>,
) -> Result<(), String> {
    let pairs = qubits_with_inversion_pairs(targets)?;
    for (pair_index, ((a, inv_a), (b, _inv_b))) in pairs.into_iter().enumerate() {
        let target_slots = [pair_index * 2, pair_index * 2 + 1];
        let loss_cause = exec.lost[a] || exec.lost[b];
        let mut bit = if loss_cause {
            true ^ inv_a
        } else {
            let terms = vec![(a, basis), (b, basis)];
            rng.prepare_intrinsic(context, &target_slots, 0);
            measure_pauli_product(&mut exec.state, &terms, inv_a, rng)
        };
        let flipped = noise_p > 0.0
            && rng.bernoulli(
                context,
                &target_slots,
                ChoiceKind::MeasurementFlip,
                0,
                noise_p,
                true,
            );
        if flipped {
            bit = !bit;
            exec.record_noise_event(
                context,
                instr_name,
                target_slots.to_vec(),
                vec![a as u32, b as u32],
                "flip",
            );
        }
        exec.recorder.push(bit);
        exec.record_measurement_event(
            context,
            instr_name,
            target_slots[0],
            a as u32,
            exec.recorder.len(),
            MeasurementOutcome { bit, loss_cause },
            MeasurementComponent::Value,
        );
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

    fn execute_op_with_script(
        name: &str,
        args: &[f64],
        targets: &[StimTarget],
        context: &ExecutionTraversalContext,
        exec: &mut ExecutionState,
        values: Vec<u64>,
    ) -> Result<(), String> {
        let mut script = ScriptRng::new(values);
        let mut randomness = ExecutionRandom::Sequential(&mut script);
        execute_op(name, args, targets, context, exec, &mut randomness, None)
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
        let mut traced =
            Executor::from_instrs(parse_lines("H 0\nREPEAT 1 {\n  M 0\n}\n").unwrap()).unwrap();
        let _ = traced
            .run_with_trace(&mut ScriptRng::new(vec![0, 0]))
            .unwrap();

        let ctx = ExecutionTraversalContext::default();
        let mut exec = ExecutionState::new(4, true);

        execute_op_with_script(
            "DEPOLARIZE1",
            &[1.0],
            &[StimTarget::Qubit(0)],
            &ctx,
            &mut exec,
            vec![0, 0],
        )
        .unwrap();
        execute_op_with_script(
            "DEPOLARIZE1",
            &[1.0],
            &[StimTarget::Qubit(1)],
            &ctx,
            &mut exec,
            vec![0, 1],
        )
        .unwrap();
        execute_op_with_script(
            "DEPOLARIZE1",
            &[1.0],
            &[StimTarget::Qubit(2)],
            &ctx,
            &mut exec,
            vec![0, 2],
        )
        .unwrap();

        exec.lost[3] = true;
        execute_op_with_script(
            "PAULI_CHANNEL_1",
            &[1.0, 0.0, 0.0],
            &[StimTarget::Qubit(3)],
            &ctx,
            &mut exec,
            vec![0],
        )
        .unwrap();
        execute_op_with_script(
            "PAULI_CHANNEL_1",
            &[1.0, 0.0, 0.0],
            &[StimTarget::Qubit(0)],
            &ctx,
            &mut exec,
            vec![0],
        )
        .unwrap();
        execute_op_with_script(
            "PAULI_CHANNEL_1",
            &[0.0, 1.0, 0.0],
            &[StimTarget::Qubit(1)],
            &ctx,
            &mut exec,
            vec![0],
        )
        .unwrap();
        execute_op_with_script(
            "PAULI_CHANNEL_1",
            &[0.0, 0.0, 1.0],
            &[StimTarget::Qubit(2)],
            &ctx,
            &mut exec,
            vec![0],
        )
        .unwrap();
        execute_op_with_script(
            "PAULI_CHANNEL_1",
            &[0.0, 0.0, 0.0],
            &[StimTarget::Qubit(2)],
            &ctx,
            &mut exec,
            vec![0],
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

        assert!(execute_op_with_script("BOGUS", &[], &[], &ctx, &mut exec, vec![0],).is_err());

        assert_eq!(
            qubits(&[StimTarget::Qubit(0), StimTarget::Sweep(1)]).unwrap(),
            vec![0]
        );
        assert!(
            qubits(&[StimTarget::Pauli {
                qubit: 0,
                basis: PauliBasis::X,
                inverted: false,
            }])
            .is_err()
        );

        assert_eq!(
            qubit_slots(&[StimTarget::Qubit(0), StimTarget::Sweep(1)]).unwrap(),
            vec![(0, 0)]
        );
        assert!(qubit_slots(&[StimTarget::QubitInv(0)]).is_err());

        assert_eq!(
            qubits_with_inversion(&[StimTarget::Qubit(0), StimTarget::Sweep(1)]).unwrap(),
            vec![(0, false)]
        );
        assert!(
            qubits_with_inversion(&[StimTarget::Pauli {
                qubit: 0,
                basis: PauliBasis::X,
                inverted: false,
            }])
            .is_err()
        );

        assert_eq!(
            qubits_with_inversion_slots(&[StimTarget::Sweep(0), StimTarget::QubitInv(1)]).unwrap(),
            vec![(1, 1, true)]
        );
        assert!(
            qubits_with_inversion_slots(&[StimTarget::Pauli {
                qubit: 0,
                basis: PauliBasis::X,
                inverted: false,
            }])
            .is_err()
        );

        assert!(qubits_with_inversion_pairs(&[StimTarget::Qubit(0)]).is_err());

        assert_eq!(expect_qubit(&StimTarget::Qubit(0)).unwrap(), 0);
        assert!(expect_qubit(&StimTarget::QubitInv(0)).is_err());
        assert!(expect_qubit(&StimTarget::Sweep(0)).is_err());
        assert!(
            expect_qubit(&StimTarget::Pauli {
                qubit: 0,
                basis: PauliBasis::X,
                inverted: false,
            })
            .is_err()
        );

        assert!(qubit_pairs(&[StimTarget::Qubit(0)]).is_err());
        assert_eq!(
            qubit_pairs(&[StimTarget::Sweep(0), StimTarget::Qubit(1)]).unwrap(),
            Vec::<(usize, usize)>::new()
        );

        assert!(
            present_qubit_pairs(
                &[StimTarget::Qubit(0), StimTarget::Qubit(1)],
                &[false, true]
            )
            .map(|pairs| pairs.is_empty())
            .unwrap()
        );
        assert!(present_qubit_pair_slots(&[StimTarget::Qubit(0)], &[false]).is_err());
        assert_eq!(
            present_qubit_pair_slots(
                &[StimTarget::Sweep(0), StimTarget::Qubit(1)],
                &[false, false]
            )
            .unwrap(),
            Vec::<((usize, usize), (usize, usize))>::new()
        );

        let recorder = Recorder::default();
        assert!(xor_recs(&recorder, &[StimTarget::Qubit(0)]).is_err());
        assert!(apply_pauli_targets(&mut exec.state, &[StimTarget::Qubit(0)]).is_err());
        assert_eq!(
            correlated_trace_payload(&[StimTarget::Qubit(0)]).unwrap(),
            None
        );
        assert_eq!(pauli_pair_label(4, 4), "??");
        apply_pauli(&mut exec.state, 0, 4);
        assert!(measure_pauli_product(
            &mut exec.state,
            &[],
            true,
            &mut ScriptRng::new(vec![0])
        ));
        apply_spp(&mut exec.state, &[], false, false);
    }

    #[test]
    fn reference_controlled_pairs_cover_sweep_and_error_paths() {
        let mut state = StabilizerState::new(2);
        let lost = vec![false, false];
        let measurements = Vec::new();
        let sweep_bits = [true];

        assert!(
            apply_reference_controlled_pairs(
                &mut state,
                &lost,
                &measurements,
                "CX",
                &[StimTarget::Qubit(0)],
                None,
            )
            .is_err()
        );

        assert!(
            apply_reference_controlled_pairs(
                &mut state,
                &lost,
                &measurements,
                "BAD",
                &[StimTarget::Qubit(0), StimTarget::Qubit(1)],
                None,
            )
            .is_err()
        );

        assert!(
            apply_reference_controlled_pairs(
                &mut state,
                &lost,
                &measurements,
                "BAD",
                &[StimTarget::Sweep(0), StimTarget::Qubit(1)],
                Some(&sweep_bits),
            )
            .is_err()
        );

        assert!(
            apply_reference_controlled_pairs(
                &mut state,
                &lost,
                &measurements,
                "CX",
                &[StimTarget::Qubit(0), StimTarget::Sweep(0)],
                Some(&sweep_bits),
            )
            .is_err()
        );

        assert!(
            apply_reference_controlled_pairs(
                &mut state,
                &lost,
                &measurements,
                "CX",
                &[
                    StimTarget::Pauli {
                        qubit: 0,
                        basis: PauliBasis::X,
                        inverted: false,
                    },
                    StimTarget::Qubit(1)
                ],
                None,
            )
            .is_err()
        );

        assert!(
            apply_reference_controlled_pairs(
                &mut state,
                &lost,
                &measurements,
                "CX",
                &[
                    StimTarget::Qubit(0),
                    StimTarget::Qubit(1),
                    StimTarget::Qubit(0)
                ],
                None,
            )
            .is_err()
        );
    }
}
