use rstim::dem::{DemInstruction, DemTarget, DetectorErrorModel};

#[test]
fn dem_empty() {
    let dem = DetectorErrorModel::new();
    assert_eq!(dem.instructions().len(), 0);
    assert_eq!(dem.num_detectors(), 0);
    assert_eq!(dem.num_observables(), 0);
}

#[test]
fn dem_error_instruction() {
    let mut dem = DetectorErrorModel::new();
    dem.add_error(0.1, vec![DemTarget::Detector(0), DemTarget::Detector(1)]);
    assert_eq!(dem.instructions().len(), 1);
    assert_eq!(dem.num_detectors(), 2);
    match &dem.instructions()[0] {
        DemInstruction::Error {
            probability,
            targets,
            ..
        } => {
            assert_eq!(*probability, 0.1);
            assert_eq!(targets.len(), 2);
        }
        _ => panic!("expected error instruction"),
    }
}

#[test]
fn dem_detector_instruction() {
    let mut dem = DetectorErrorModel::new();
    dem.add_detector(5, vec![1.0, 2.5]);
    assert_eq!(dem.num_detectors(), 6);
    match &dem.instructions()[0] {
        DemInstruction::Detector { index, coords } => {
            assert_eq!(*index, 5);
            assert_eq!(coords, &vec![1.0, 2.5]);
        }
        _ => panic!("expected detector instruction"),
    }
}

#[test]
fn dem_observable_instruction() {
    let mut dem = DetectorErrorModel::new();
    dem.add_observable(2);
    assert_eq!(dem.num_observables(), 3);
}

#[test]
fn dem_error_with_observable() {
    let mut dem = DetectorErrorModel::new();
    dem.add_error(
        0.01,
        vec![DemTarget::Detector(0), DemTarget::Observable(0)],
    );
    assert_eq!(dem.num_detectors(), 1);
    assert_eq!(dem.num_observables(), 1);
}

#[test]
fn dem_repeat_block() {
    let mut body = DetectorErrorModel::new();
    body.add_error(0.01, vec![DemTarget::Detector(0), DemTarget::Detector(1)]);
    body.add_shift_detectors(1, vec![0.0, 1.0]);
    let mut dem = DetectorErrorModel::new();
    dem.add_repeat(10, body);
    assert_eq!(dem.instructions().len(), 1);
}

#[test]
fn dem_push_direct() {
    let mut dem = DetectorErrorModel::new();
    dem.push(DemInstruction::Detector {
        index: 2,
        coords: vec![0.0, 1.0],
    });
    dem.push(DemInstruction::LogicalObservable { index: 1 });
    dem.push(DemInstruction::Error {
        probability: 0.05,
        targets: vec![DemTarget::Detector(0), DemTarget::Observable(0)],
    });
    assert_eq!(dem.instructions().len(), 3);
    assert_eq!(dem.num_detectors(), 3);
    assert_eq!(dem.num_observables(), 2);
}

#[test]
fn dem_push_repeat_folds_body_counts() {
    let mut body = DetectorErrorModel::new();
    body.add_error(0.01, vec![DemTarget::Detector(4), DemTarget::Observable(3)]);
    let mut dem = DetectorErrorModel::new();
    dem.push(DemInstruction::Repeat { count: 5, body });
    assert_eq!(dem.num_detectors(), 5);
    assert_eq!(dem.num_observables(), 4);
}

#[test]
fn dem_shift_detectors() {
    let mut dem = DetectorErrorModel::new();
    dem.add_shift_detectors(3, vec![0.0, 0.5]);
    match &dem.instructions()[0] {
        DemInstruction::ShiftDetectors {
            detector_offset,
            coord_offsets,
        } => {
            assert_eq!(*detector_offset, 3);
            assert_eq!(coord_offsets, &vec![0.0, 0.5]);
        }
        _ => panic!("expected shift_detectors"),
    }
}
