use crate::decoder_dataset::LOGICAL_FLIP_MARKER;
use crate::ir::{circuit_to_string, StimInstr, StimTarget};
use super::NoiseParams;

/// Generate a rotated surface code memory-X experiment circuit.
pub fn rotated_memory_x(distance: usize, rounds: usize, noise: f64) -> Vec<StimInstr> {
    rotated_memory_x_with_params(distance, rounds, NoiseParams::uniform(noise))
}

/// Generate a rotated surface code memory-Z experiment circuit.
pub fn rotated_memory_z(distance: usize, rounds: usize, noise: f64) -> Vec<StimInstr> {
    rotated_memory_z_with_params(distance, rounds, NoiseParams::uniform(noise))
}

/// Generate a rotated surface code memory-X experiment circuit with per-channel noise.
pub fn rotated_memory_x_with_params(distance: usize, rounds: usize, params: NoiseParams) -> Vec<StimInstr> {
    rotated_surface_code(distance, rounds, params, true)
}

/// Generate a rotated surface code memory-Z experiment circuit with per-channel noise.
pub fn rotated_memory_z_with_params(distance: usize, rounds: usize, params: NoiseParams) -> Vec<StimInstr> {
    rotated_surface_code(distance, rounds, params, false)
}

const Z_ORDER: [(i32, i32); 4] = [(1, 1), (1, -1), (-1, 1), (-1, -1)];
const X_ORDER: [(i32, i32); 4] = [(1, 1), (-1, 1), (1, -1), (-1, -1)];

/// Shared qubit layout for rotated surface-code memory circuits.
pub(crate) struct RotatedLayout {
    pub data_coords: Vec<(i32, i32)>,
    pub x_measure_coords: Vec<(i32, i32)>,
    pub z_measure_coords: Vec<(i32, i32)>,
    pub measure_coords: Vec<(i32, i32)>,
    pub all_coords: Vec<(i32, i32)>,
    pub data_qubits: Vec<u32>,
    pub x_measure_qubits: Vec<u32>,
    pub measure_qubits: Vec<u32>,
    pub x_observable: Vec<(i32, i32)>,
    pub z_observable: Vec<(i32, i32)>,
    pub cnot_layers: [Vec<u32>; 4],
}

pub(crate) fn coord_order(coords: &[(i32, i32)], target: (i32, i32)) -> usize {
    coords.iter().position(|&c| c == target).unwrap()
}

fn rotated_layout(d: usize) -> RotatedLayout {
    // --- Build coordinate sets ---
    let mut data_coords: Vec<(i32, i32)> = Vec::new();
    let mut x_observable: Vec<(i32, i32)> = Vec::new();
    let mut z_observable: Vec<(i32, i32)> = Vec::new();
    for xi in 0..d {
        for yi in 0..d {
            let cx = (xi as i32) * 2 + 1;
            let cy = (yi as i32) * 2 + 1;
            data_coords.push((cx, cy));
            if xi == 0 {
                x_observable.push((cx, cy));
            }
            if yi == 0 {
                z_observable.push((cx, cy));
            }
        }
    }
    data_coords.sort();

    let mut x_measure_coords: Vec<(i32, i32)> = Vec::new();
    let mut z_measure_coords: Vec<(i32, i32)> = Vec::new();
    for xi in 0..=(d as i32) {
        for yi in 0..=(d as i32) {
            let cx = xi * 2;
            let cy = yi * 2;
            let on_boundary_1 = xi == 0 || xi == d as i32;
            let on_boundary_2 = yi == 0 || yi == d as i32;
            let parity = (xi % 2) != (yi % 2);
            if on_boundary_1 && parity {
                continue;
            }
            if on_boundary_2 && !parity {
                continue;
            }
            if parity {
                x_measure_coords.push((cx, cy));
            } else {
                z_measure_coords.push((cx, cy));
            }
        }
    }
    x_measure_coords.sort();
    z_measure_coords.sort();

    let mut measure_coords: Vec<(i32, i32)> = Vec::new();
    measure_coords.extend_from_slice(&x_measure_coords);
    measure_coords.extend_from_slice(&z_measure_coords);
    measure_coords.sort();

    let mut all_coords: Vec<(i32, i32)> = Vec::new();
    all_coords.extend_from_slice(&data_coords);
    all_coords.extend_from_slice(&measure_coords);
    all_coords.sort();
    all_coords.dedup();

    let coord_to_idx = |c: (i32, i32)| -> u32 {
        all_coords.iter().position(|&x| x == c).unwrap() as u32
    };

    let data_qubits: Vec<u32> = data_coords.iter().map(|&c| coord_to_idx(c)).collect();
    let x_measure_qubits: Vec<u32> = x_measure_coords.iter().map(|&c| coord_to_idx(c)).collect();
    let measure_qubits: Vec<u32> = measure_coords.iter().map(|&c| coord_to_idx(c)).collect();

    let mut cnot_layers: [Vec<u32>; 4] = [vec![], vec![], vec![], vec![]];
    for k in 0..4 {
        for &mc in &x_measure_coords {
            let dc = (mc.0 + X_ORDER[k].0, mc.1 + X_ORDER[k].1);
            if data_coords.contains(&dc) {
                cnot_layers[k].push(coord_to_idx(mc));
                cnot_layers[k].push(coord_to_idx(dc));
            }
        }
        for &mc in &z_measure_coords {
            let dc = (mc.0 + Z_ORDER[k].0, mc.1 + Z_ORDER[k].1);
            if data_coords.contains(&dc) {
                cnot_layers[k].push(coord_to_idx(dc));
                cnot_layers[k].push(coord_to_idx(mc));
            }
        }
    }

    RotatedLayout {
        data_coords,
        x_measure_coords,
        z_measure_coords,
        measure_coords,
        all_coords,
        data_qubits,
        x_measure_qubits,
        measure_qubits,
        x_observable,
        z_observable,
        cnot_layers,
    }
}

fn rotated_surface_code(d: usize, rounds: usize, params: NoiseParams, is_memory_x: bool) -> Vec<StimInstr> {
    assert!(d >= 2, "distance must be >= 2");
    assert!(rounds >= 1, "rounds must be >= 1");

    let RotatedLayout {
        data_coords,
        x_measure_coords,
        z_measure_coords,
        measure_coords,
        all_coords,
        data_qubits,
        x_measure_qubits,
        measure_qubits,
        x_observable,
        z_observable,
        cnot_layers,
    } = rotated_layout(d);

    let chosen_measure_coords: &Vec<(i32, i32)> = if is_memory_x { &x_measure_coords } else { &z_measure_coords };
    let chosen_observable: &Vec<(i32, i32)> = if is_memory_x { &x_observable } else { &z_observable };

    let mut instrs: Vec<StimInstr> = Vec::new();

    // QUBIT_COORDS
    for &(cx, cy) in &all_coords {
        let q = coord_order(&all_coords, (cx, cy)) as u32;
        instrs.push(op("QUBIT_COORDS", &[cx as f64, cy as f64], &[StimTarget::Qubit(q)]));
    }

    // Reset data qubits in chosen basis
    let data_reset_op = if is_memory_x { "RX" } else { "R" };
    for &q in &data_qubits {
        instrs.push(op(data_reset_op, &[], &[StimTarget::Qubit(q)]));
    }
    if params.after_reset_flip_probability > 0.0 {
        for &q in &data_qubits {
            instrs.push(op("X_ERROR", &[params.after_reset_flip_probability], &[StimTarget::Qubit(q)]));
        }
    }

    // Reset all ancilla
    for &q in &measure_qubits {
        instrs.push(op("R", &[], &[StimTarget::Qubit(q)]));
    }
    if params.after_reset_flip_probability > 0.0 {
        for &q in &measure_qubits {
            instrs.push(op("X_ERROR", &[params.after_reset_flip_probability], &[StimTarget::Qubit(q)]));
        }
    }

    let measure_coord_to_order = |c: (i32, i32)| -> usize {
        measure_coords.iter().position(|&x| x == c).unwrap()
    };
    let data_coord_to_order = |c: (i32, i32)| -> usize {
        data_coords.iter().position(|&x| x == c).unwrap()
    };

    let n_measure = measure_qubits.len();
    let n_data = data_qubits.len();

    // Helper: emit one stabilizer round (H, 4 CX layers, H, MR)
    let emit_round = |instrs: &mut Vec<StimInstr>| {
        instrs.push(op("TICK", &[], &[]));

        // before_round_data_depolarization at start of round
        if params.before_round_data_depolarization > 0.0 {
            for &q in &data_qubits {
                instrs.push(op("DEPOLARIZE1", &[params.before_round_data_depolarization], &[StimTarget::Qubit(q)]));
            }
        }

        // H on X ancilla
        for &q in &x_measure_qubits {
            instrs.push(op("H", &[], &[StimTarget::Qubit(q)]));
        }
        emit_after_clifford_loss(
            instrs,
            params.after_clifford_loss_probability,
            &x_measure_qubits,
        );
        if params.after_clifford_depolarization > 0.0 {
            for &q in &x_measure_qubits {
                instrs.push(op(
                    "DEPOLARIZE1",
                    &[params.after_clifford_depolarization],
                    &[StimTarget::Qubit(q)],
                ));
            }
        }
        // 4 CNOT layers
        for k in 0..4 {
            instrs.push(op("TICK", &[], &[]));
            let targets: Vec<StimTarget> = cnot_layers[k].iter().map(|&q| StimTarget::Qubit(q)).collect();
            if !targets.is_empty() {
                instrs.push(op("CX", &[], &targets));
                emit_after_clifford_loss(
                    instrs,
                    params.after_clifford_loss_probability,
                    &cnot_layers[k],
                );
            }
            if params.after_clifford_depolarization > 0.0 && !cnot_layers[k].is_empty() {
                let pairs: Vec<StimTarget> = cnot_layers[k].iter().map(|&q| StimTarget::Qubit(q)).collect();
                instrs.push(op("DEPOLARIZE2", &[params.after_clifford_depolarization], &pairs));
            }
        }
        instrs.push(op("TICK", &[], &[]));
        // H on X ancilla
        for &q in &x_measure_qubits {
            instrs.push(op("H", &[], &[StimTarget::Qubit(q)]));
        }
        emit_after_clifford_loss(
            instrs,
            params.after_clifford_loss_probability,
            &x_measure_qubits,
        );
        if params.after_clifford_depolarization > 0.0 {
            for &q in &x_measure_qubits {
                instrs.push(op(
                    "DEPOLARIZE1",
                    &[params.after_clifford_depolarization],
                    &[StimTarget::Qubit(q)],
                ));
            }
        }
        instrs.push(op("TICK", &[], &[]));

        // before_measure_flip_probability: X_ERROR before MR
        if params.before_measure_flip_probability > 0.0 {
            for &q in &measure_qubits {
                instrs.push(op("X_ERROR", &[params.before_measure_flip_probability], &[StimTarget::Qubit(q)]));
            }
        }

        // MR all ancilla
        for &q in &measure_qubits {
            instrs.push(op("MR", &[], &[StimTarget::Qubit(q)]));
        }

        // after_reset_flip_probability: X_ERROR after MR (which includes reset)
        if params.after_reset_flip_probability > 0.0 {
            for &q in &measure_qubits {
                instrs.push(op("X_ERROR", &[params.after_reset_flip_probability], &[StimTarget::Qubit(q)]));
            }
        }
    };

    // First round (head cycle)
    emit_round(&mut instrs);
    // Detectors for first round: only chosen-basis ancilla
    for &mc in chosen_measure_coords.iter() {
        let order = measure_coord_to_order(mc);
        let rec_offset = -((n_measure - order) as i32);
        instrs.push(op(
            "DETECTOR",
            &[mc.0 as f64, mc.1 as f64, 0.0],
            &[StimTarget::Rec(rec_offset)],
        ));
    }

    // Subsequent rounds (body cycle, rounds-1 times)
    for _round in 1..rounds {
        instrs.push(op("SHIFT_COORDS", &[0.0, 0.0, 1.0], &[]));
        emit_round(&mut instrs);
        // Detectors: all ancilla, compare current to previous
        for &mc in &measure_coords {
            let order = measure_coord_to_order(mc);
            let k = (n_measure - order - 1) as i32;
            instrs.push(op(
                "DETECTOR",
                &[mc.0 as f64, mc.1 as f64, 0.0],
                &[StimTarget::Rec(-(k + 1)), StimTarget::Rec(-(k + 1 + n_measure as i32))],
            ));
        }
    }

    // Tail: measure data qubits in chosen basis
    instrs.push(op("TICK", &[], &[]));
    if params.before_measure_flip_probability > 0.0 {
        for &q in &data_qubits {
            instrs.push(op("X_ERROR", &[params.before_measure_flip_probability], &[StimTarget::Qubit(q)]));
        }
    }
    let data_meas_op = if is_memory_x { "MX" } else { "M" };
    for &q in &data_qubits {
        instrs.push(op(data_meas_op, &[], &[StimTarget::Qubit(q)]));
    }

    // Tail detectors
    for &mc in chosen_measure_coords.iter() {
        let mut det_targets: Vec<StimTarget> = Vec::new();
        for &delta in &Z_ORDER {
            let dc = (mc.0 + delta.0, mc.1 + delta.1);
            if let Some(dorder) = data_coords.iter().position(|&x| x == dc) {
                let offset = -((n_data - dorder) as i32);
                det_targets.push(StimTarget::Rec(offset));
            }
        }
        let morder = measure_coord_to_order(mc);
        let anc_offset = -((n_data + n_measure - morder) as i32);
        det_targets.push(StimTarget::Rec(anc_offset));
        det_targets.sort_by_key(|t| match t { StimTarget::Rec(r) => *r, _ => 0 });
        instrs.push(op(
            "DETECTOR",
            &[mc.0 as f64, mc.1 as f64, 1.0],
            &det_targets,
        ));
    }

    // Observable
    let obs_targets: Vec<StimTarget> = {
        let mut v: Vec<StimTarget> = chosen_observable.iter().map(|&c| {
            let dorder = data_coord_to_order(c);
            StimTarget::Rec(-((n_data - dorder) as i32))
        }).collect();
        v.sort_by_key(|t| match t { StimTarget::Rec(r) => *r, _ => 0 });
        v
    };
    instrs.push(op("OBSERVABLE_INCLUDE", &[0.0], &obs_targets));

    instrs
}

fn op(name: &str, args: &[f64], targets: &[StimTarget]) -> StimInstr {
    StimInstr::Op {
        name: name.to_string(),
        tag: None,
        args: args.to_vec(),
        targets: targets.to_vec(),
    }
}

fn emit_after_clifford_loss(instrs: &mut Vec<StimInstr>, probability: f64, qubits: &[u32]) {
    if probability <= 0.0 || qubits.is_empty() {
        return;
    }
    let targets: Vec<StimTarget> = qubits.iter().copied().map(StimTarget::Qubit).collect();
    instrs.push(op("LOSS", &[probability], &targets));
}

/// Per-channel noise and loss parameters for the loss-visible conventional
/// rotated-memory-Z circuit.
///
/// Each Pauli-noise field drives exactly the channel it is named after; there
/// is deliberately no "uniform" shortcut. Loss fields follow the native
/// Mid-SWAP generator's conventions: operation loss applies to resets and
/// single-qubit gates at the full rate and to two-qubit gates at half rate,
/// while measurement loss is sampled immediately before each loss-visible
/// readout.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RotatedMemoryZLossConfig {
    pub before_round_data_depolarization: f64,
    pub after_clifford_depolarization: f64,
    pub before_measure_flip_probability: f64,
    pub after_reset_flip_probability: f64,
    pub operation_loss_probability: f64,
    pub measurement_loss_probability: f64,
    pub after_clifford_loss_probability: f64,
}

/// Generate the conventional fixed-order rotated-memory-Z circuit in
/// loss-visible form.
///
/// The circuit keeps the fixed `rotated_memory_z` CNOT layer order in every
/// round (no alternating A/B schedule and no shuttles), emits loss-visible
/// `MRL`/`ML` readouts whose records interleave `loss_flag,value_bit`, and
/// places exactly one `# RSTIM_LOGICAL_FLIP_POINT` immediately after the data
/// reset so the circuit can drive `export_decoder_dataset --mode
/// measurements_blinded`.
pub fn rotated_memory_z_loss_visible(
    distance: usize,
    rounds: usize,
    config: RotatedMemoryZLossConfig,
) -> Result<String, String> {
    if distance < 2 {
        return Err(format!("distance must be at least 2, got {distance}"));
    }
    if rounds == 0 {
        return Err("rounds must be positive".to_string());
    }
    for (name, value) in [
        (
            "before_round_data_depolarization",
            config.before_round_data_depolarization,
        ),
        (
            "after_clifford_depolarization",
            config.after_clifford_depolarization,
        ),
        (
            "before_measure_flip_probability",
            config.before_measure_flip_probability,
        ),
        (
            "after_reset_flip_probability",
            config.after_reset_flip_probability,
        ),
        (
            "operation_loss_probability",
            config.operation_loss_probability,
        ),
        (
            "measurement_loss_probability",
            config.measurement_loss_probability,
        ),
        (
            "after_clifford_loss_probability",
            config.after_clifford_loss_probability,
        ),
    ] {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(format!(
                "{name} must be finite and in [0, 1], got {value}"
            ));
        }
    }
    Ok(LossVisibleMemoryZ::new(distance, rounds, config).build())
}

enum LossVisibleItem {
    Instruction(StimInstr),
    Comment(&'static str),
}

struct LossVisibleMemoryZ {
    config: RotatedMemoryZLossConfig,
    layout: RotatedLayout,
    rounds: usize,
    items: Vec<LossVisibleItem>,
}

impl LossVisibleMemoryZ {
    fn new(distance: usize, rounds: usize, config: RotatedMemoryZLossConfig) -> Self {
        Self {
            config,
            layout: rotated_layout(distance),
            rounds,
            items: Vec::new(),
        }
    }

    fn build(mut self) -> String {
        let n_measure = self.layout.measure_qubits.len();
        let n_data = self.layout.data_qubits.len();

        let all_coords = self.layout.all_coords.clone();
        for &(cx, cy) in &all_coords {
            let q = coord_order(&all_coords, (cx, cy)) as u32;
            self.emit("QUBIT_COORDS", &[cx as f64, cy as f64], &[q]);
        }

        // Data reset, then the logical-flip marker before any data noise or
        // loss so the blinded-dataset boundary stays ahead of the first LOSS
        // on the logical support.
        let data = self.layout.data_qubits.clone();
        let measure = self.layout.measure_qubits.clone();
        self.emit("R", &[], &data);
        self.comment(LOGICAL_FLIP_MARKER.trim_start_matches("# "));
        self.emit_noise("X_ERROR", self.config.after_reset_flip_probability, &data);
        self.emit_noise("LOSS", self.config.operation_loss_probability, &data);
        self.emit("R", &[], &measure);
        self.emit_noise("X_ERROR", self.config.after_reset_flip_probability, &measure);
        self.emit_noise("LOSS", self.config.operation_loss_probability, &measure);

        for round in 0..self.rounds {
            if round > 0 {
                self.emit("SHIFT_COORDS", &[0.0, 0.0, 1.0], &[]);
            }
            self.emit_round();
            if round == 0 {
                let z_measure_coords = self.layout.z_measure_coords.clone();
                for &mc in &z_measure_coords {
                    let order = coord_order(&self.layout.measure_coords, mc) as i32;
                    let value_offset = 2 * order + 1 - 2 * n_measure as i32;
                    self.emit_detector(mc, 0.0, &[value_offset]);
                }
            } else {
                let measure_coords = self.layout.measure_coords.clone();
                for &mc in &measure_coords {
                    let order = coord_order(&self.layout.measure_coords, mc) as i32;
                    let current = 2 * order + 1 - 2 * n_measure as i32;
                    let previous = current - 2 * n_measure as i32;
                    self.emit_detector(mc, 0.0, &[current, previous]);
                }
            }
        }

        // Tail: loss-visible data measurement.
        self.emit("TICK", &[], &[]);
        self.emit_noise("LOSS", self.config.measurement_loss_probability, &data);
        self.emit_noise(
            "X_ERROR",
            self.config.before_measure_flip_probability,
            &data,
        );
        self.emit("ML", &[], &data);

        let z_measure_coords = self.layout.z_measure_coords.clone();
        for &mc in &z_measure_coords {
            let mut offsets: Vec<i32> = Vec::new();
            for &delta in &Z_ORDER {
                let dc = (mc.0 + delta.0, mc.1 + delta.1);
                if let Some(dorder) = self.layout.data_coords.iter().position(|&x| x == dc) {
                    offsets.push(2 * dorder as i32 + 1 - 2 * n_data as i32);
                }
            }
            let morder = coord_order(&self.layout.measure_coords, mc) as i32;
            offsets.push(2 * morder + 1 - 2 * (n_data + n_measure) as i32);
            offsets.sort_unstable();
            self.emit_detector(mc, 1.0, &offsets);
        }

        let mut observable: Vec<i32> = self
            .layout
            .z_observable
            .iter()
            .map(|&c| {
                let dorder = coord_order(&self.layout.data_coords, c) as i32;
                2 * dorder + 1 - 2 * n_data as i32
            })
            .collect();
        observable.sort_unstable();
        self.emit_rec("OBSERVABLE_INCLUDE", &[0.0], &observable);

        let mut output = String::new();
        for item in &self.items {
            match item {
                LossVisibleItem::Instruction(instruction) => {
                    output.push_str(&circuit_to_string(std::slice::from_ref(instruction)));
                }
                LossVisibleItem::Comment(text) => {
                    output.push_str("# ");
                    output.push_str(text);
                    output.push('\n');
                }
            }
        }
        output
    }

    fn emit_round(&mut self) {
        let config = self.config;
        let data = self.layout.data_qubits.clone();
        let x_measure = self.layout.x_measure_qubits.clone();
        let measure = self.layout.measure_qubits.clone();
        let layers = self.layout.cnot_layers.clone();

        self.emit("TICK", &[], &[]);
        self.emit_noise(
            "DEPOLARIZE1",
            config.before_round_data_depolarization,
            &data,
        );

        self.emit("H", &[], &x_measure);
        self.emit_noise("LOSS", config.after_clifford_loss_probability, &x_measure);
        self.emit_noise("DEPOLARIZE1", config.after_clifford_depolarization, &x_measure);
        self.emit_noise("LOSS", config.operation_loss_probability, &x_measure);

        for layer in &layers {
            self.emit("TICK", &[], &[]);
            if layer.is_empty() {
                continue;
            }
            self.emit("CX", &[], layer);
            self.emit_noise("LOSS", config.after_clifford_loss_probability, layer);
            self.emit_noise("DEPOLARIZE2", config.after_clifford_depolarization, layer);
            self.emit_noise("LOSS", config.operation_loss_probability / 2.0, layer);
        }

        self.emit("TICK", &[], &[]);
        self.emit("H", &[], &x_measure);
        self.emit_noise("LOSS", config.after_clifford_loss_probability, &x_measure);
        self.emit_noise("DEPOLARIZE1", config.after_clifford_depolarization, &x_measure);
        self.emit_noise("LOSS", config.operation_loss_probability, &x_measure);

        self.emit("TICK", &[], &[]);
        self.emit_noise("LOSS", config.measurement_loss_probability, &measure);
        self.emit_noise(
            "X_ERROR",
            config.before_measure_flip_probability,
            &measure,
        );
        self.emit("MRL", &[], &measure);
        self.emit_noise("X_ERROR", config.after_reset_flip_probability, &measure);
        self.emit_noise("LOSS", config.operation_loss_probability, &measure);
    }

    fn emit_detector(&mut self, coord: (i32, i32), time: f64, offsets: &[i32]) {
        self.emit_rec(
            "DETECTOR",
            &[coord.0 as f64, coord.1 as f64, time],
            offsets,
        );
    }

    fn emit(&mut self, name: &str, args: &[f64], wires: &[u32]) {
        let targets: Vec<StimTarget> = wires.iter().copied().map(StimTarget::Qubit).collect();
        self.items.push(LossVisibleItem::Instruction(op(name, args, &targets)));
    }

    fn emit_rec(&mut self, name: &str, args: &[f64], offsets: &[i32]) {
        let targets: Vec<StimTarget> = offsets.iter().copied().map(StimTarget::Rec).collect();
        self.items.push(LossVisibleItem::Instruction(op(name, args, &targets)));
    }

    fn emit_noise(&mut self, name: &str, probability: f64, wires: &[u32]) {
        if probability > 0.0 && !wires.is_empty() {
            self.emit(name, &[probability], wires);
        }
    }

    fn comment(&mut self, text: &'static str) {
        self.items.push(LossVisibleItem::Comment(text));
    }
}


/// Generate an unrotated surface code memory-X experiment circuit.
pub fn unrotated_memory_x(distance: usize, rounds: usize, noise: f64) -> Vec<StimInstr> {
    unrotated_memory_x_with_params(distance, rounds, NoiseParams::uniform(noise))
}

/// Generate an unrotated surface code memory-Z experiment circuit.
pub fn unrotated_memory_z(distance: usize, rounds: usize, noise: f64) -> Vec<StimInstr> {
    unrotated_memory_z_with_params(distance, rounds, NoiseParams::uniform(noise))
}

/// Generate an unrotated surface code memory-X experiment circuit with per-channel noise.
pub fn unrotated_memory_x_with_params(distance: usize, rounds: usize, params: NoiseParams) -> Vec<StimInstr> {
    unrotated_surface_code(distance, rounds, params, true)
}

/// Generate an unrotated surface code memory-Z experiment circuit with per-channel noise.
pub fn unrotated_memory_z_with_params(distance: usize, rounds: usize, params: NoiseParams) -> Vec<StimInstr> {
    unrotated_surface_code(distance, rounds, params, false)
}

fn unrotated_surface_code(d: usize, rounds: usize, params: NoiseParams, is_memory_x: bool) -> Vec<StimInstr> {
    assert!(d >= 2, "distance must be >= 2");
    assert!(rounds >= 1, "rounds must be >= 1");

    let width = 2 * d - 1;

    let mut data_coords: Vec<(i32, i32)> = Vec::new();
    let mut x_measure_coords: Vec<(i32, i32)> = Vec::new();
    let mut z_measure_coords: Vec<(i32, i32)> = Vec::new();
    let mut x_observable: Vec<(i32, i32)> = Vec::new();
    let mut z_observable: Vec<(i32, i32)> = Vec::new();

    for y in 0..width {
        for x in 0..width {
            let xi = x as i32;
            let yi = y as i32;
            let parity = (x % 2) != (y % 2);
            if parity {
                if x % 2 == 0 {
                    z_measure_coords.push((xi, yi));
                } else {
                    x_measure_coords.push((xi, yi));
                }
            } else {
                data_coords.push((xi, yi));
                if x == 0 {
                    x_observable.push((xi, yi));
                }
                if y == 0 {
                    z_observable.push((xi, yi));
                }
            }
        }
    }

    let coord_to_idx = |c: (i32, i32)| -> u32 { (c.0 + c.1 * width as i32) as u32 };

    let data_qubits: Vec<u32> = data_coords.iter().map(|&c| coord_to_idx(c)).collect();
    let x_measure_qubits: Vec<u32> = x_measure_coords.iter().map(|&c| coord_to_idx(c)).collect();
    let _z_measure_qubits: Vec<u32> = z_measure_coords.iter().map(|&c| coord_to_idx(c)).collect();

    let mut measure_coords: Vec<(i32, i32)> = Vec::new();
    measure_coords.extend_from_slice(&x_measure_coords);
    measure_coords.extend_from_slice(&z_measure_coords);
    measure_coords.sort();

    let measure_qubits: Vec<u32> = measure_coords.iter().map(|&c| coord_to_idx(c)).collect();

    let mut all_coords: Vec<(i32, i32)> = Vec::new();
    all_coords.extend_from_slice(&data_coords);
    all_coords.extend_from_slice(&x_measure_coords);
    all_coords.extend_from_slice(&z_measure_coords);
    all_coords.sort();
    all_coords.dedup();

    let chosen_measure_coords: &Vec<(i32, i32)> = if is_memory_x { &x_measure_coords } else { &z_measure_coords };
    let chosen_observable: &Vec<(i32, i32)> = if is_memory_x { &x_observable } else { &z_observable };

    let interact_order: [(i32, i32); 4] = [(1, 0), (0, 1), (0, -1), (-1, 0)];

    let mut cnot_layers: [Vec<u32>; 4] = [vec![], vec![], vec![], vec![]];
    for k in 0..4 {
        for &mc in &x_measure_coords {
            let dc = (mc.0 + interact_order[k].0, mc.1 + interact_order[k].1);
            if data_coords.contains(&dc) {
                cnot_layers[k].push(coord_to_idx(mc));
                cnot_layers[k].push(coord_to_idx(dc));
            }
        }
        for &mc in &z_measure_coords {
            let dc = (mc.0 + interact_order[k].0, mc.1 + interact_order[k].1);
            if data_coords.contains(&dc) {
                cnot_layers[k].push(coord_to_idx(dc));
                cnot_layers[k].push(coord_to_idx(mc));
            }
        }
    }

    let measure_coord_to_order = |c: (i32, i32)| -> usize {
        measure_coords.iter().position(|&x| x == c).unwrap()
    };
    let data_coord_to_order = |c: (i32, i32)| -> usize {
        data_coords.iter().position(|&x| x == c).unwrap()
    };

    let n_measure = measure_qubits.len();
    let n_data = data_qubits.len();

    let mut instrs: Vec<StimInstr> = Vec::new();

    // QUBIT_COORDS
    for &(cx, cy) in &all_coords {
        let q = coord_to_idx((cx, cy));
        instrs.push(op("QUBIT_COORDS", &[cx as f64, cy as f64], &[StimTarget::Qubit(q)]));
    }

    // Reset data in chosen basis
    let data_reset_op = if is_memory_x { "RX" } else { "R" };
    for &q in &data_qubits {
        instrs.push(op(data_reset_op, &[], &[StimTarget::Qubit(q)]));
    }
    if params.after_reset_flip_probability > 0.0 {
        for &q in &data_qubits {
            instrs.push(op("X_ERROR", &[params.after_reset_flip_probability], &[StimTarget::Qubit(q)]));
        }
    }

    // Reset all ancilla
    for &q in &measure_qubits {
        instrs.push(op("R", &[], &[StimTarget::Qubit(q)]));
    }
    if params.after_reset_flip_probability > 0.0 {
        for &q in &measure_qubits {
            instrs.push(op("X_ERROR", &[params.after_reset_flip_probability], &[StimTarget::Qubit(q)]));
        }
    }

    // Helper: emit one stabilizer round
    let emit_round = |instrs: &mut Vec<StimInstr>| {
        instrs.push(op("TICK", &[], &[]));

        if params.before_round_data_depolarization > 0.0 {
            for &q in &data_qubits {
                instrs.push(op("DEPOLARIZE1", &[params.before_round_data_depolarization], &[StimTarget::Qubit(q)]));
            }
        }

        for &q in &x_measure_qubits {
            instrs.push(op("H", &[], &[StimTarget::Qubit(q)]));
        }
        emit_after_clifford_loss(
            instrs,
            params.after_clifford_loss_probability,
            &x_measure_qubits,
        );
        for k in 0..4 {
            instrs.push(op("TICK", &[], &[]));
            let targets: Vec<StimTarget> = cnot_layers[k].iter().map(|&q| StimTarget::Qubit(q)).collect();
            if !targets.is_empty() {
                instrs.push(op("CX", &[], &targets));
                emit_after_clifford_loss(
                    instrs,
                    params.after_clifford_loss_probability,
                    &cnot_layers[k],
                );
            }
            if params.after_clifford_depolarization > 0.0 && !cnot_layers[k].is_empty() {
                let pairs: Vec<StimTarget> = cnot_layers[k].iter().map(|&q| StimTarget::Qubit(q)).collect();
                instrs.push(op("DEPOLARIZE2", &[params.after_clifford_depolarization], &pairs));
            }
        }
        instrs.push(op("TICK", &[], &[]));
        for &q in &x_measure_qubits {
            instrs.push(op("H", &[], &[StimTarget::Qubit(q)]));
        }
        emit_after_clifford_loss(
            instrs,
            params.after_clifford_loss_probability,
            &x_measure_qubits,
        );
        instrs.push(op("TICK", &[], &[]));

        if params.before_measure_flip_probability > 0.0 {
            for &q in &measure_qubits {
                instrs.push(op("X_ERROR", &[params.before_measure_flip_probability], &[StimTarget::Qubit(q)]));
            }
        }

        for &q in &measure_qubits {
            instrs.push(op("MR", &[], &[StimTarget::Qubit(q)]));
        }

        if params.after_reset_flip_probability > 0.0 {
            for &q in &measure_qubits {
                instrs.push(op("X_ERROR", &[params.after_reset_flip_probability], &[StimTarget::Qubit(q)]));
            }
        }
    };

    // First round
    emit_round(&mut instrs);
    for &mc in chosen_measure_coords.iter() {
        let order = measure_coord_to_order(mc);
        let rec_offset = -((n_measure - order) as i32);
        instrs.push(op(
            "DETECTOR",
            &[mc.0 as f64, mc.1 as f64, 0.0],
            &[StimTarget::Rec(rec_offset)],
        ));
    }

    // Subsequent rounds
    for _round in 1..rounds {
        instrs.push(op("SHIFT_COORDS", &[0.0, 0.0, 1.0], &[]));
        emit_round(&mut instrs);
        for &mc in &measure_coords {
            let order = measure_coord_to_order(mc);
            let k = (n_measure - order - 1) as i32;
            instrs.push(op(
                "DETECTOR",
                &[mc.0 as f64, mc.1 as f64, 0.0],
                &[StimTarget::Rec(-(k + 1)), StimTarget::Rec(-(k + 1 + n_measure as i32))],
            ));
        }
    }

    // Tail: measure data in chosen basis
    instrs.push(op("TICK", &[], &[]));
    if params.before_round_data_depolarization > 0.0 {
        for &q in &data_qubits {
            instrs.push(op("DEPOLARIZE1", &[params.before_round_data_depolarization], &[StimTarget::Qubit(q)]));
        }
    }
    if params.before_measure_flip_probability > 0.0 {
        for &q in &data_qubits {
            instrs.push(op("X_ERROR", &[params.before_measure_flip_probability], &[StimTarget::Qubit(q)]));
        }
    }
    let data_meas_op = if is_memory_x { "MX" } else { "M" };
    for &q in &data_qubits {
        instrs.push(op(data_meas_op, &[], &[StimTarget::Qubit(q)]));
    }

    // Tail detectors
    for &mc in chosen_measure_coords.iter() {
        let mut det_targets: Vec<StimTarget> = Vec::new();
        for &delta in &interact_order {
            let dc = (mc.0 + delta.0, mc.1 + delta.1);
            if let Some(dorder) = data_coords.iter().position(|&x| x == dc) {
                let offset = -((n_data - dorder) as i32);
                det_targets.push(StimTarget::Rec(offset));
            }
        }
        let morder = measure_coord_to_order(mc);
        let anc_offset = -((n_data + n_measure - morder) as i32);
        det_targets.push(StimTarget::Rec(anc_offset));
        det_targets.sort_by_key(|t| match t { StimTarget::Rec(r) => *r, _ => 0 });
        instrs.push(op(
            "DETECTOR",
            &[mc.0 as f64, mc.1 as f64, 1.0],
            &det_targets,
        ));
    }

    // Observable
    let obs_targets: Vec<StimTarget> = {
        let mut v: Vec<StimTarget> = chosen_observable.iter().map(|&c| {
            let dorder = data_coord_to_order(c);
            StimTarget::Rec(-((n_data - dorder) as i32))
        }).collect();
        v.sort_by_key(|t| match t { StimTarget::Rec(r) => *r, _ => 0 });
        v
    };
    instrs.push(op("OBSERVABLE_INCLUDE", &[0.0], &obs_targets));

    instrs
}
