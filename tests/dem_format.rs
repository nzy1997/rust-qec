use rstim::dem::{DemTarget, DetectorErrorModel};

#[test]
fn dem_write_empty() {
    let dem = DetectorErrorModel::new();
    assert_eq!(dem.to_string(), "");
}

#[test]
fn dem_write_simple_error() {
    let mut dem = DetectorErrorModel::new();
    dem.add_error(0.1, vec![DemTarget::Detector(0), DemTarget::Detector(1)]);
    assert_eq!(dem.to_string(), "error(0.1) D0 D1\n");
}

#[test]
fn dem_write_error_with_observable() {
    let mut dem = DetectorErrorModel::new();
    dem.add_error(0.01, vec![
        DemTarget::Detector(2), DemTarget::Detector(3), DemTarget::Observable(0)
    ]);
    assert_eq!(dem.to_string(), "error(0.01) D2 D3 L0\n");
}

#[test]
fn dem_write_error_with_separator() {
    let mut dem = DetectorErrorModel::new();
    dem.add_error(0.02, vec![
        DemTarget::Detector(2), DemTarget::Observable(0),
        DemTarget::Separator,
        DemTarget::Detector(5), DemTarget::Detector(6),
    ]);
    assert_eq!(dem.to_string(), "error(0.02) D2 L0 ^ D5 D6\n");
}

#[test]
fn dem_write_detector_with_coords() {
    let mut dem = DetectorErrorModel::new();
    dem.add_detector(4, vec![2.5, 3.5, 6.0]);
    assert_eq!(dem.to_string(), "detector(2.5, 3.5, 6) D4\n");
}

#[test]
fn dem_write_shift_detectors() {
    let mut dem = DetectorErrorModel::new();
    dem.add_shift_detectors(2, vec![0.0, 0.5]);
    assert_eq!(dem.to_string(), "shift_detectors(0, 0.5) 2\n");
}

#[test]
fn dem_write_shift_detectors_no_coords() {
    let mut dem = DetectorErrorModel::new();
    dem.add_shift_detectors(3, vec![]);
    assert_eq!(dem.to_string(), "shift_detectors 3\n");
}

#[test]
fn dem_write_repeat() {
    let mut body = DetectorErrorModel::new();
    body.add_error(0.01, vec![DemTarget::Detector(0), DemTarget::Detector(1)]);
    body.add_shift_detectors(1, vec![]);
    let mut dem = DetectorErrorModel::new();
    dem.add_repeat(10, body);
    let expected = "repeat 10 {\n    error(0.01) D0 D1\n    shift_detectors 1\n}\n";
    assert_eq!(dem.to_string(), expected);
}

#[test]
fn dem_parse_simple() {
    let input = "error(0.1) D0 D1\n";
    let dem = DetectorErrorModel::parse(input).unwrap();
    assert_eq!(dem.instructions().len(), 1);
    assert_eq!(dem.num_detectors(), 2);
}

#[test]
fn dem_parse_full() {
    let input = "\
error(0.1) D0 D1
error(0.01) D2 D3 L0
detector(1, 2) D0
logical_observable L1
shift_detectors(0, 0.5) 2
repeat 5 {
    error(0.05) D0 D1
    shift_detectors 1
}
";
    let dem = DetectorErrorModel::parse(input).unwrap();
    assert_eq!(dem.instructions().len(), 6);
}

#[test]
fn dem_parse_round_trip() {
    let input = "\
error(0.1) D0 D1
error(0.01) D2 D3 L0
detector(1, 2) D0
shift_detectors(0, 0.5) 2
repeat 5 {
    error(0.05) D0 D1
    shift_detectors 1
}
";
    let dem = DetectorErrorModel::parse(input).unwrap();
    let output = dem.to_string();
    let dem2 = DetectorErrorModel::parse(&output).unwrap();
    assert_eq!(dem, dem2);
}

#[test]
fn dem_parse_comments_and_blank_lines() {
    let input = "# This is a comment\n\nerror(0.1) D0\n# Another comment\n";
    let dem = DetectorErrorModel::parse(input).unwrap();
    assert_eq!(dem.instructions().len(), 1);
}

#[test]
fn dem_parse_separator() {
    let input = "error(0.02) D2 L0 ^ D5 D6\n";
    let dem = DetectorErrorModel::parse(input).unwrap();
    match &dem.instructions()[0] {
        rstim::dem::DemInstruction::Error { targets, .. } => {
            assert_eq!(targets.len(), 5);
            assert_eq!(targets[2], DemTarget::Separator);
        }
        _ => panic!("expected error"),
    }
}
