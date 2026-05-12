#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SampleTrace {
    pub noise_events: Vec<NoiseEvent>,
    pub measurement_events: Vec<MeasurementEvent>,
    pub detector_events: Vec<DetectorEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoiseEvent {
    pub op_path: Vec<usize>,
    pub repeat_iterations: Vec<u64>,
    pub instr_name: String,
    pub target_slots: Vec<usize>,
    pub target_qubits: Vec<u32>,
    pub occurred: bool,
    pub branch_label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeasurementEvent {
    pub op_path: Vec<usize>,
    pub repeat_iterations: Vec<u64>,
    pub target_slot: usize,
    pub target_qubit: u32,
    pub instr_name: String,
    pub measurement_index: usize,
    pub bit: bool,
    pub loss_cause: bool,
    pub component: MeasurementComponent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectorEvent {
    pub op_path: Vec<usize>,
    pub repeat_iterations: Vec<u64>,
    pub detector_index: usize,
    pub flipped: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MeasurementComponent {
    Value,
    LossFlag,
}
