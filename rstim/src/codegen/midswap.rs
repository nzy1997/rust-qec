use std::collections::HashMap;
use std::fmt;

#[cfg(test)]
use crate::decoder_dataset::LOGICAL_FLIP_MARKER;
use crate::ir::{circuit_to_string, StimInstr, StimTarget};

const CNOT_SCHEDULE: &str = "paper_alternating_ab";

#[derive(Debug, Clone, Copy, PartialEq)]
/// Configuration for the flat Mid-SWAP rotated-memory-Z generator.
///
/// ```
/// use rstim::codegen::{MidSwapConfig, rotated_memory_z_midswap};
///
/// let config = MidSwapConfig {
///     distance: 3,
///     rounds: 2,
///     before_round_data_depolarization: 0.001,
///     before_round_data_loss_probability: 0.0,
///     after_clifford_depolarization: 0.001,
///     before_measure_flip_probability: 0.001,
///     after_reset_flip_probability: 0.001,
///     operation_loss_probability: 0.0,
///     measurement_loss_probability: 0.0,
/// };
///
/// let circuit = rotated_memory_z_midswap(config).unwrap();
/// assert!(!circuit.is_empty());
/// ```
///
/// ```compile_fail,E0560
/// use rstim::codegen::MidSwapConfig;
///
/// // Old API: this does not compile against the current release line.
/// let config = MidSwapConfig {
///     distance: 3,
///     rounds: 2,
///     before_round_data_depolarization: 0.001,
///     before_round_data_loss_probability: 0.0,
///     after_clifford_depolarization: 0.001,
///     before_measure_flip_probability: 0.001,
///     after_reset_flip_probability: 0.001,
///     operation_loss_probability: 0.0,
///     measurement_loss_probability: 0.0,
///     pauli_probability: 0.001,
/// };
/// ```
pub struct MidSwapConfig {
    pub distance: usize,
    pub rounds: usize,
    pub before_round_data_depolarization: f64,
    pub before_round_data_loss_probability: f64,
    pub after_clifford_depolarization: f64,
    pub before_measure_flip_probability: f64,
    pub after_reset_flip_probability: f64,
    pub operation_loss_probability: f64,
    pub measurement_loss_probability: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MidSwapError {
    InvalidDistance(usize),
    InvalidRounds,
    InvalidProbability { name: &'static str, value: String },
    CircuitTooLarge,
    InternalLayout(String),
}

impl fmt::Display for MidSwapError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDistance(distance) => write!(
                formatter,
                "distance must be odd and at least 3, got {distance}"
            ),
            Self::InvalidRounds => write!(formatter, "rounds must be positive"),
            Self::InvalidProbability { name, value } => {
                write!(
                    formatter,
                    "{name} must be finite and in [0, 1], got {value}"
                )
            }
            Self::CircuitTooLarge => write!(formatter, "requested Mid-SWAP circuit is too large"),
            Self::InternalLayout(message) => {
                write!(formatter, "invalid Mid-SWAP layout: {message}")
            }
        }
    }
}

impl std::error::Error for MidSwapError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CheckBasis {
    X,
    Z,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SiteKind {
    Data,
    Check(CheckBasis),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Site {
    wire_label: u32,
    x: i32,
    y: i32,
    kind: SiteKind,
}

#[derive(Debug)]
enum CircuitItem {
    Instruction(StimInstr),
    Comment(String),
}

pub fn rotated_memory_z_midswap(config: MidSwapConfig) -> Result<String, MidSwapError> {
    validate_config(config)?;
    MidSwapBuilder::new(config)?.build()
}

fn validate_config(config: MidSwapConfig) -> Result<(), MidSwapError> {
    if config.distance < 3 || config.distance.is_multiple_of(2) {
        return Err(MidSwapError::InvalidDistance(config.distance));
    }
    if config.rounds == 0 {
        return Err(MidSwapError::InvalidRounds);
    }
    for (name, value) in [
        (
            "before_round_data_depolarization",
            config.before_round_data_depolarization,
        ),
        (
            "before_round_data_loss_probability",
            config.before_round_data_loss_probability,
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
    ] {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(MidSwapError::InvalidProbability {
                name,
                value: value.to_string(),
            });
        }
    }

    let distance_squared = config
        .distance
        .checked_mul(config.distance)
        .ok_or(MidSwapError::CircuitTooLarge)?;
    let check_measurements = config
        .rounds
        .checked_mul(distance_squared - 1)
        .ok_or(MidSwapError::CircuitTooLarge)?;
    let measurement_bits = check_measurements
        .checked_add(distance_squared)
        .and_then(|measurements| measurements.checked_mul(2))
        .ok_or(MidSwapError::CircuitTooLarge)?;
    if measurement_bits > i32::MAX as usize {
        return Err(MidSwapError::CircuitTooLarge);
    }
    let diameter = config
        .distance
        .checked_mul(2)
        .ok_or(MidSwapError::CircuitTooLarge)?;
    let stride = diameter
        .checked_add(1)
        .ok_or(MidSwapError::CircuitTooLarge)?;
    let max_wire = config
        .distance
        .checked_mul(stride)
        .and_then(|value| value.checked_add(diameter))
        .ok_or(MidSwapError::CircuitTooLarge)?;
    if max_wire > u32::MAX as usize || diameter > i32::MAX as usize {
        return Err(MidSwapError::CircuitTooLarge);
    }
    Ok(())
}

struct MidSwapBuilder {
    config: MidSwapConfig,
    data: Vec<Site>,
    checks: Vec<Site>,
    data_at: HashMap<(i32, i32), Site>,
    physical_wire: HashMap<u32, u32>,
    items: Vec<CircuitItem>,
    measurement_count: i32,
    round_values: HashMap<(usize, u32), i32>,
}

impl MidSwapBuilder {
    fn new(config: MidSwapConfig) -> Result<Self, MidSwapError> {
        let (data, checks) = layout(config.distance)?;
        let data_at = data.iter().map(|site| ((site.x, site.y), *site)).collect();
        let physical_wire = data
            .iter()
            .chain(&checks)
            .map(|site| (site.wire_label, site.wire_label))
            .collect();
        Ok(Self {
            config,
            data,
            checks,
            data_at,
            physical_wire,
            items: Vec::new(),
            measurement_count: 0,
            round_values: HashMap::new(),
        })
    }

    fn build(mut self) -> Result<String, MidSwapError> {
        self.comment("Generated natively by RStim's Mid-SWAP builder.");
        self.comment("Loss-visible measurement records are ordered loss_flag,value_bit.");
        self.comment(&format!("Logical CNOT schedule: {CNOT_SCHEDULE}."));
        self.emit_initialization();
        for round in 0..self.config.rounds {
            self.emit_round(round)?;
        }
        self.emit_final_measurement()?;
        Ok(render_items(&self.items))
    }

    fn emit_initialization(&mut self) {
        let mut sites: Vec<Site> = self.data.iter().chain(&self.checks).copied().collect();
        sites.sort_by_key(|site| site.wire_label);
        for site in sites {
            self.emit(
                "QUBIT_COORDS",
                vec![site.x as f64, site.y as f64],
                vec![StimTarget::Qubit(site.wire_label)],
            );
        }
        let data = self.mapped_sites(&self.data.clone());
        let checks = self.mapped_sites(&self.checks.clone());
        self.emit("R", vec![], qubit_targets(&data));
        self.items.push(CircuitItem::Instruction(
            crate::decoder_dataset::logical_flip_marker_instruction(),
        ));
        self.emit_noise("X_ERROR", self.config.after_reset_flip_probability, &data);
        self.emit_noise("LOSS", self.config.operation_loss_probability, &data);
        self.emit("R", vec![], qubit_targets(&checks));
        self.emit_noise("X_ERROR", self.config.after_reset_flip_probability, &checks);
        self.emit_noise("LOSS", self.config.operation_loss_probability, &checks);
        self.emit("TICK", vec![], vec![]);
    }

    fn emit_round(&mut self, round: usize) -> Result<(), MidSwapError> {
        let data_wires = self.mapped_sites(&self.data.clone());
        self.emit_noise(
            "DEPOLARIZE1",
            self.config.before_round_data_depolarization,
            &data_wires,
        );
        self.emit_noise(
            "LOSS",
            self.config.before_round_data_loss_probability,
            &data_wires,
        );

        let x_checks: Vec<Site> = self
            .checks
            .iter()
            .copied()
            .filter(|site| site.kind == SiteKind::Check(CheckBasis::X))
            .collect();
        let mut x_wires = self.mapped_sites(&x_checks);
        self.emit("H", vec![], qubit_targets(&x_wires));
        self.emit_noise(
            "DEPOLARIZE1",
            self.config.after_clifford_depolarization,
            &x_wires,
        );
        self.emit_noise("LOSS", self.config.operation_loss_probability, &x_wires);
        self.emit("TICK", vec![], vec![]);

        for physical_layer in 0..4 {
            let pairs = self.layer_pairs(round, physical_layer);
            let mut cnot_wires = Vec::with_capacity(2 * pairs.len());
            for (check, data) in &pairs {
                let (control, target) = match check.kind {
                    SiteKind::Check(CheckBasis::X) => (*check, *data),
                    SiteKind::Check(CheckBasis::Z) => (*data, *check),
                    SiteKind::Data => {
                        return Err(MidSwapError::InternalLayout(
                            "CNOT pair used a data site as a check".to_string(),
                        ));
                    }
                };
                cnot_wires.push(self.mapped(control));
                cnot_wires.push(self.mapped(target));
            }
            self.emit("CX", vec![], qubit_targets(&cnot_wires));
            self.emit_noise(
                "DEPOLARIZE2",
                self.config.after_clifford_depolarization,
                &cnot_wires,
            );
            self.emit_noise(
                "LOSS",
                self.config.operation_loss_probability / 2.0,
                &cnot_wires,
            );
            if physical_layer == 0 {
                self.compile_shuttle(round, &pairs);
            }
            self.emit("TICK", vec![], vec![]);
        }

        x_wires = self.mapped_sites(&x_checks);
        self.emit("H", vec![], qubit_targets(&x_wires));
        self.emit_noise(
            "DEPOLARIZE1",
            self.config.after_clifford_depolarization,
            &x_wires,
        );
        self.emit_noise("LOSS", self.config.operation_loss_probability, &x_wires);
        self.emit("TICK", vec![], vec![]);

        let checks = self.checks.clone();
        let values = self.emit_loss_visible_measurement("MRL", &checks, true);
        for (check, value_record) in checks.iter().zip(values) {
            self.round_values
                .insert((round, check.wire_label), value_record);
        }
        for check in checks {
            if round == 0 && check.kind == SiteKind::Check(CheckBasis::X) {
                continue;
            }
            let mut records = vec![self.round_values[&(round, check.wire_label)]];
            if round > 0 {
                records.push(self.round_values[&(round - 1, check.wire_label)]);
            }
            self.emit_detector(check, round as f64, &records);
        }
        if round + 1 < self.config.rounds {
            self.emit("TICK", vec![], vec![]);
        }
        Ok(())
    }

    fn emit_final_measurement(&mut self) -> Result<(), MidSwapError> {
        let data = self.data.clone();
        let values = self.emit_loss_visible_measurement("ML", &data, false);
        let value_by_site: HashMap<u32, i32> = data
            .iter()
            .zip(values)
            .map(|(site, value)| (site.wire_label, value))
            .collect();
        let checks = self.checks.clone();
        for check in checks
            .into_iter()
            .filter(|site| site.kind == SiteKind::Check(CheckBasis::Z))
        {
            let mut records = Vec::new();
            for (dx, dy) in [(-1, 1), (1, 1), (-1, -1), (1, -1)] {
                if let Some(data_site) = self.data_at.get(&(check.x + dx, check.y + dy)) {
                    records.push(value_by_site[&data_site.wire_label]);
                }
            }
            records.push(self.round_values[&(self.config.rounds - 1, check.wire_label)]);
            self.emit_detector(check, self.config.rounds as f64, &records);
        }

        let logical_records: Vec<i32> = data
            .iter()
            .filter(|site| site.y == 1)
            .map(|site| value_by_site[&site.wire_label])
            .collect();
        self.emit(
            "OBSERVABLE_INCLUDE",
            vec![0.0],
            logical_records
                .iter()
                .map(|&record| StimTarget::Rec(self.rec_offset(record)))
                .collect(),
        );
        Ok(())
    }

    fn layer_pairs(&self, round: usize, physical_layer: usize) -> Vec<(Site, Site)> {
        let logical_layer = if round.is_multiple_of(2) {
            physical_layer
        } else {
            3 - physical_layer
        };
        let mut pairs = Vec::new();
        for check in &self.checks {
            let directions = match check.kind {
                SiteKind::Check(CheckBasis::X) => [(-1, 1), (1, 1), (-1, -1), (1, -1)],
                SiteKind::Check(CheckBasis::Z) => [(-1, 1), (-1, -1), (1, 1), (1, -1)],
                SiteKind::Data => continue,
            };
            let (dx, dy) = directions[logical_layer];
            if let Some(data) = self.data_at.get(&(check.x + dx, check.y + dy)) {
                pairs.push((*check, *data));
            }
        }
        pairs
    }

    fn compile_shuttle(&mut self, round: usize, pairs: &[(Site, Site)]) {
        let mut labels = Vec::with_capacity(2 * pairs.len());
        for (check, data) in pairs {
            labels.push(check.wire_label);
            labels.push(data.wire_label);
            let check_wire = self.physical_wire[&check.wire_label];
            let data_wire = self.physical_wire[&data.wire_label];
            self.physical_wire.insert(check.wire_label, data_wire);
            self.physical_wire.insert(data.wire_label, check_wire);
        }
        let labels = labels
            .iter()
            .map(u32::to_string)
            .collect::<Vec<_>>()
            .join(" ");
        self.comment(&format!(
            "MIDSWAP_SHUTTLE round={round} logical_pairs={labels}"
        ));
    }

    fn emit_loss_visible_measurement(
        &mut self,
        instruction: &str,
        sites: &[Site],
        resets: bool,
    ) -> Vec<i32> {
        let wires = self.mapped_sites(sites);
        self.emit_noise("LOSS", self.config.measurement_loss_probability, &wires);
        self.emit_noise(
            "X_ERROR",
            self.config.before_measure_flip_probability,
            &wires,
        );
        self.emit(instruction, vec![], qubit_targets(&wires));
        let values: Vec<i32> = (0..sites.len())
            .map(|index| self.measurement_count + 2 * index as i32 + 1)
            .collect();
        self.measurement_count += 2 * sites.len() as i32;
        if resets {
            self.emit_noise("X_ERROR", self.config.after_reset_flip_probability, &wires);
            self.emit_noise("LOSS", self.config.operation_loss_probability, &wires);
        }
        values
    }

    fn emit_detector(&mut self, check: Site, time: f64, records: &[i32]) {
        self.emit(
            "DETECTOR",
            vec![check.x as f64, check.y as f64, time],
            records
                .iter()
                .map(|&record| StimTarget::Rec(self.rec_offset(record)))
                .collect(),
        );
    }

    fn rec_offset(&self, absolute_record: i32) -> i32 {
        absolute_record - self.measurement_count
    }

    fn mapped(&self, site: Site) -> u32 {
        self.physical_wire[&site.wire_label]
    }

    fn mapped_sites(&self, sites: &[Site]) -> Vec<u32> {
        sites.iter().map(|&site| self.mapped(site)).collect()
    }

    fn emit_noise(&mut self, name: &str, probability: f64, wires: &[u32]) {
        if probability > 0.0 && !wires.is_empty() {
            self.emit(name, vec![probability], qubit_targets(wires));
        }
    }

    fn emit(&mut self, name: &str, args: Vec<f64>, targets: Vec<StimTarget>) {
        self.items.push(CircuitItem::Instruction(StimInstr::new(
            name, args, targets,
        )));
    }

    fn comment(&mut self, text: &str) {
        self.items.push(CircuitItem::Comment(text.to_string()));
    }
}

fn layout(distance: usize) -> Result<(Vec<Site>, Vec<Site>), MidSwapError> {
    let limit = (2 * distance) as i32;
    let stride = (2 * distance + 1) as u32;
    let wire_label = |x: i32, y: i32| x as u32 + (y as u32 / 2) * stride;
    let mut data = Vec::new();
    for y in (1..limit).step_by(2) {
        for x in (1..limit).step_by(2) {
            data.push(Site {
                wire_label: wire_label(x, y),
                x,
                y,
                kind: SiteKind::Data,
            });
        }
    }
    let mut checks = Vec::new();
    for y in (0..=limit).step_by(2) {
        for x in (0..=limit).step_by(2) {
            if !is_check_coordinate(limit, x, y) {
                continue;
            }
            let basis = if (x + y).rem_euclid(4) == 2 {
                CheckBasis::X
            } else {
                CheckBasis::Z
            };
            checks.push(Site {
                wire_label: wire_label(x, y),
                x,
                y,
                kind: SiteKind::Check(basis),
            });
        }
    }
    data.sort_by_key(|site| site.wire_label);
    checks.sort_by_key(|site| site.wire_label);
    if data.len() != distance * distance || checks.len() + 1 != distance * distance {
        return Err(MidSwapError::InternalLayout(format!(
            "expected {} data and {} checks, found {} and {}",
            distance * distance,
            distance * distance - 1,
            data.len(),
            checks.len()
        )));
    }
    Ok((data, checks))
}

fn is_check_coordinate(limit: i32, x: i32, y: i32) -> bool {
    if x.rem_euclid(2) != 0 || y.rem_euclid(2) != 0 {
        return false;
    }
    if x > 0 && x < limit && y > 0 && y < limit {
        return true;
    }
    if (y == 0 || y == limit) && x > 0 && x < limit {
        return (x + y).rem_euclid(4) == 2;
    }
    if (x == 0 || x == limit) && y > 0 && y < limit {
        return (x + y).rem_euclid(4) == 0;
    }
    false
}

fn qubit_targets(wires: &[u32]) -> Vec<StimTarget> {
    wires.iter().copied().map(StimTarget::Qubit).collect()
}

fn render_items(items: &[CircuitItem]) -> String {
    let mut output = String::new();
    for item in items {
        match item {
            CircuitItem::Instruction(instruction) => {
                output.push_str(&circuit_to_string(std::slice::from_ref(instruction)));
            }
            CircuitItem::Comment(comment) => {
                output.push_str("# ");
                output.push_str(comment);
                output.push('\n');
            }
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_lines;
    use crate::stats;

    fn noiseless_config(rounds: usize) -> MidSwapConfig {
        MidSwapConfig {
            distance: 3,
            rounds,
            before_round_data_depolarization: 0.0,
            before_round_data_loss_probability: 0.0,
            after_clifford_depolarization: 0.0,
            before_measure_flip_probability: 0.0,
            after_reset_flip_probability: 0.0,
            operation_loss_probability: 0.0,
            measurement_loss_probability: 0.0,
        }
    }

    #[test]
    fn distance_three_round_two_has_the_expected_shape() {
        let text = rotated_memory_z_midswap(noiseless_config(2)).unwrap();
        let circuit = parse_lines(&text).unwrap();
        assert_eq!(stats::num_qubits(&circuit), 26);
        assert_eq!(stats::num_measurements(&circuit), 50);
        assert_eq!(stats::num_detectors(&circuit), 16);
        assert_eq!(stats::num_observables(&circuit), 1);
        assert_eq!(
            text.lines().filter(|line| line.starts_with("MRL ")).count(),
            2
        );
        assert_eq!(
            text.lines().filter(|line| line.starts_with("ML ")).count(),
            1
        );
        assert_eq!(text.matches("# MIDSWAP_SHUTTLE").count(), 2);

        let mut measurement_count = 0_i64;
        for instruction in &circuit {
            let name = instruction.name().unwrap();
            let targets = instruction.targets().unwrap();
            if matches!(name, "DETECTOR" | "OBSERVABLE_INCLUDE") {
                for target in targets {
                    let StimTarget::Rec(offset) = target else {
                        panic!("{name} contained a non-record target: {target:?}");
                    };
                    let referenced = measurement_count + i64::from(*offset);
                    assert!(
                        *offset < 0 && (0..measurement_count).contains(&referenced),
                        "{name} used invalid rec[{offset}] after {measurement_count} records"
                    );
                }
            }
            if matches!(name, "MRL" | "ML") {
                measurement_count += 2 * targets.len() as i64;
            }
        }
        assert_eq!(measurement_count, 50);
    }

    #[test]
    fn initialization_places_one_flip_marker_before_first_noise() {
        let mut config = noiseless_config(1);
        config.after_reset_flip_probability = 0.01;
        config.operation_loss_probability = 0.02;
        let text = rotated_memory_z_midswap(config).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        let data_reset = lines
            .iter()
            .position(|line| line.starts_with("R "))
            .unwrap();

        assert_eq!(text.matches(LOGICAL_FLIP_MARKER).count(), 1);
        assert_eq!(lines[data_reset + 1], LOGICAL_FLIP_MARKER);
        assert!(lines[data_reset + 2].starts_with("X_ERROR(0.01) "));
        assert!(lines[data_reset + 3].starts_with("LOSS(0.02) "));
    }

    #[test]
    fn noise_channels_are_independent_and_follow_logical_data_mapping() {
        let mut config = noiseless_config(2);
        config.before_round_data_depolarization = 0.01;
        config.after_clifford_depolarization = 0.02;
        config.before_measure_flip_probability = 0.03;
        config.after_reset_flip_probability = 0.04;
        config.before_round_data_loss_probability = 0.05;
        let text = rotated_memory_z_midswap(config).unwrap();
        let circuit = parse_lines(&text).unwrap();

        for (index, instruction) in circuit.iter().enumerate() {
            match instruction.name().unwrap() {
                "R" => {
                    let next = &circuit[index + 1];
                    let noise_index = if matches!(
                        next,
                        StimInstr::Op { name, tag, .. }
                            if name == "TICK"
                                && tag.as_deref()
                                    == Some(crate::decoder_dataset::LOGICAL_FLIP_MARKER_TAG)
                    ) {
                        index + 2
                    } else {
                        index + 1
                    };
                    assert_eq!(circuit[noise_index].name(), Some("X_ERROR"));
                    assert_eq!(circuit[noise_index].args(), Some(&[0.04][..]));
                }
                "MRL" => {
                    assert_eq!(circuit[index - 1].name(), Some("X_ERROR"));
                    assert_eq!(circuit[index - 1].args(), Some(&[0.03][..]));
                    assert_eq!(circuit[index + 1].name(), Some("X_ERROR"));
                    assert_eq!(circuit[index + 1].args(), Some(&[0.04][..]));
                }
                "ML" => {
                    assert_eq!(circuit[index - 1].name(), Some("X_ERROR"));
                    assert_eq!(circuit[index - 1].args(), Some(&[0.03][..]));
                }
                "H" => {
                    assert_eq!(circuit[index + 1].name(), Some("DEPOLARIZE1"));
                    assert_eq!(circuit[index + 1].args(), Some(&[0.02][..]));
                }
                "CX" => {
                    assert_eq!(circuit[index + 1].name(), Some("DEPOLARIZE2"));
                    assert_eq!(circuit[index + 1].args(), Some(&[0.02][..]));
                }
                _ => {}
            }
        }

        let before_round_targets: Vec<Vec<u32>> = circuit
            .iter()
            .filter(|instruction| {
                instruction.name() == Some("DEPOLARIZE1") && instruction.args() == Some(&[0.01][..])
            })
            .map(|instruction| {
                instruction
                    .targets()
                    .unwrap()
                    .iter()
                    .map(|target| match target {
                        StimTarget::Qubit(wire) => *wire,
                        other => panic!("expected qubit target, got {other:?}"),
                    })
                    .collect()
            })
            .collect();
        assert_eq!(
            before_round_targets,
            vec![
                vec![1, 3, 5, 8, 10, 12, 15, 17, 19],
                vec![2, 3, 5, 9, 11, 13, 16, 18, 19],
            ]
        );

        let before_round_loss_targets: Vec<Vec<u32>> = circuit
            .iter()
            .filter(|instruction| {
                instruction.name() == Some("LOSS") && instruction.args() == Some(&[0.05][..])
            })
            .map(|instruction| {
                instruction
                    .targets()
                    .unwrap()
                    .iter()
                    .map(|target| match target {
                        StimTarget::Qubit(wire) => *wire,
                        other => panic!("expected qubit target, got {other:?}"),
                    })
                    .collect()
            })
            .collect();
        assert_eq!(before_round_loss_targets, before_round_targets);
    }

    #[test]
    fn first_round_schedule_and_persistent_mapping_are_golden() {
        let text = rotated_memory_z_midswap(noiseless_config(2)).unwrap();
        let quantum_cx: Vec<&str> = text
            .lines()
            .filter(|line| line.starts_with("CX "))
            .collect();
        assert_eq!(
            &quantum_cx[..4],
            &[
                "CX 2 1 8 9 11 10 12 13 16 15 17 18",
                "CX 1 3 2 8 10 13 5 12 15 18 11 17",
                "CX 11 8 10 3 16 14 15 9 19 17 25 18",
                "CX 3 8 10 5 9 14 15 11 13 17 25 19",
            ]
        );
        assert_eq!(
            &quantum_cx[4..8],
            &[
                "CX 3 8 10 5 9 14 15 11 13 17 25 19",
                "CX 15 3 5 8 16 9 11 14 25 13 19 18",
                "CX 1 8 2 3 5 17 10 12 11 18 15 13",
                "CX 1 2 14 3 5 15 17 12 11 16 18 13",
            ]
        );
    }

    #[test]
    fn invalid_user_inputs_return_errors() {
        assert_eq!(
            MidSwapError::InvalidRounds.to_string(),
            "rounds must be positive"
        );
        assert_eq!(
            MidSwapError::CircuitTooLarge.to_string(),
            "requested Mid-SWAP circuit is too large"
        );
        assert_eq!(
            MidSwapError::InternalLayout("broken mapping".to_string()).to_string(),
            "invalid Mid-SWAP layout: broken mapping"
        );

        let mut config = noiseless_config(1);
        config.distance = 4;
        assert!(matches!(
            rotated_memory_z_midswap(config),
            Err(MidSwapError::InvalidDistance(4))
        ));
        config.distance = 3;
        config.rounds = 0;
        assert_eq!(
            rotated_memory_z_midswap(config),
            Err(MidSwapError::InvalidRounds)
        );
        config.rounds = 1;
        config.before_round_data_depolarization = -0.01;
        assert!(matches!(
            rotated_memory_z_midswap(config),
            Err(MidSwapError::InvalidProbability { .. })
        ));
        config.before_round_data_depolarization = 0.0;
        config.before_round_data_loss_probability = 1.01;
        assert!(matches!(
            rotated_memory_z_midswap(config),
            Err(MidSwapError::InvalidProbability { .. })
        ));
        config.before_round_data_loss_probability = 0.0;
        config.operation_loss_probability = f64::NAN;
        assert!(matches!(
            rotated_memory_z_midswap(config),
            Err(MidSwapError::InvalidProbability { .. })
        ));
        config.operation_loss_probability = 0.0;
        config.measurement_loss_probability = 1.01;
        assert!(matches!(
            rotated_memory_z_midswap(config),
            Err(MidSwapError::InvalidProbability { .. })
        ));
    }
}
