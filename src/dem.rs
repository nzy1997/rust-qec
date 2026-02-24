#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum DemTarget {
    Detector(usize),
    Observable(usize),
    Separator,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DemInstruction {
    Error {
        probability: f64,
        targets: Vec<DemTarget>,
    },
    Detector {
        index: usize,
        coords: Vec<f64>,
    },
    LogicalObservable {
        index: usize,
    },
    ShiftDetectors {
        detector_offset: usize,
        coord_offsets: Vec<f64>,
    },
    Repeat {
        count: u64,
        body: DetectorErrorModel,
    },
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct DetectorErrorModel {
    instrs: Vec<DemInstruction>,
    num_detectors: usize,
    num_observables: usize,
}

impl DetectorErrorModel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn instructions(&self) -> &[DemInstruction] {
        &self.instrs
    }

    pub fn num_detectors(&self) -> usize {
        self.num_detectors
    }

    pub fn num_observables(&self) -> usize {
        self.num_observables
    }

    pub fn add_error(&mut self, probability: f64, targets: Vec<DemTarget>) {
        self.push(DemInstruction::Error {
            probability,
            targets,
        });
    }

    pub fn add_detector(&mut self, index: usize, coords: Vec<f64>) {
        self.push(DemInstruction::Detector { index, coords });
    }

    pub fn add_observable(&mut self, index: usize) {
        self.push(DemInstruction::LogicalObservable { index });
    }

    pub fn add_shift_detectors(&mut self, detector_offset: usize, coord_offsets: Vec<f64>) {
        self.push(DemInstruction::ShiftDetectors {
            detector_offset,
            coord_offsets,
        });
    }

    pub fn add_repeat(&mut self, count: u64, body: DetectorErrorModel) {
        self.push(DemInstruction::Repeat { count, body });
    }

    pub fn push(&mut self, instr: DemInstruction) {
        match &instr {
            DemInstruction::Error { targets, .. } => self.update_counts_from_targets(targets),
            DemInstruction::Detector { index, .. } => {
                self.num_detectors = self.num_detectors.max(index + 1);
            }
            DemInstruction::LogicalObservable { index } => {
                self.num_observables = self.num_observables.max(index + 1);
            }
            DemInstruction::ShiftDetectors { .. } => {}
            DemInstruction::Repeat { body, .. } => {
                self.num_detectors = self.num_detectors.max(body.num_detectors);
                self.num_observables = self.num_observables.max(body.num_observables);
            }
        }
        self.instrs.push(instr);
    }

    fn update_counts_from_targets(&mut self, targets: &[DemTarget]) {
        for target in targets {
            match target {
                DemTarget::Detector(i) => {
                    self.num_detectors = self.num_detectors.max(i + 1);
                }
                DemTarget::Observable(i) => {
                    self.num_observables = self.num_observables.max(i + 1);
                }
                DemTarget::Separator => {}
            }
        }
    }
}
