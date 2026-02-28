use rand::SeedableRng;
use rand::rngs::StdRng;
use rstim::dem::{DemTarget, DetectorErrorModel};

#[test]
fn effective_num_detectors_with_shift() {
    let mut dem = DetectorErrorModel::new();
    dem.add_error(1.0, vec![DemTarget::Detector(0)]);
    dem.add_shift_detectors(2, vec![]);
    dem.add_error(1.0, vec![DemTarget::Detector(0)]);
    assert_eq!(dem.effective_num_detectors(), 3);
}

#[test]
fn effective_num_detectors_with_detector_instr() {
    let mut dem = DetectorErrorModel::new();
    dem.add_detector(2, vec![1.0, 2.0]);
    assert_eq!(dem.effective_num_detectors(), 3);
}

#[test]
fn effective_num_detectors_nested_repeat_with_shift() {
    let mut inner_body = DetectorErrorModel::new();
    inner_body.add_error(1.0, vec![DemTarget::Detector(0)]);
    inner_body.add_shift_detectors(1, vec![]);

    let mut body = DetectorErrorModel::new();
    body.add_repeat(2, inner_body);

    let mut dem = DetectorErrorModel::new();
    dem.add_repeat(3, body);
    assert_eq!(dem.effective_num_detectors(), 6);
}

#[test]
fn total_shift_nested_repeat() {
    let mut inner = DetectorErrorModel::new();
    inner.add_shift_detectors(1, vec![]);
    let mut outer = DetectorErrorModel::new();
    outer.add_repeat(3, inner);
    let mut dem = DetectorErrorModel::new();
    dem.add_repeat(2, outer);
    assert_eq!(dem.effective_num_detectors(), 0);
}

#[test]
fn sample_with_shift_detectors() {
    let mut dem = DetectorErrorModel::new();
    dem.add_error(1.0, vec![DemTarget::Detector(0)]);
    dem.add_shift_detectors(1, vec![]);
    dem.add_error(1.0, vec![DemTarget::Detector(0)]);
    let mut rng = StdRng::seed_from_u64(42);
    let (dets, _) = dem.sample(&mut rng);
    assert_eq!(dets, vec![true, true]);
}

#[test]
fn sample_batch_with_shift() {
    let mut dem = DetectorErrorModel::new();
    dem.add_error(1.0, vec![DemTarget::Detector(0)]);
    dem.add_shift_detectors(1, vec![]);
    dem.add_error(1.0, vec![DemTarget::Detector(0)]);
    let mut rng = StdRng::seed_from_u64(42);
    let out = dem.sample_batch(10, &mut rng);
    assert_eq!(out.detections.num_major(), 2);
    for shot in 0..10 {
        assert!(out.detections.get(0, shot));
        assert!(out.detections.get(1, shot));
    }
}

#[test]
fn sample_logical_observable_ignored() {
    let mut dem = DetectorErrorModel::new();
    dem.add_observable(0);
    dem.add_error(1.0, vec![DemTarget::Observable(0)]);
    let mut rng = StdRng::seed_from_u64(42);
    let (_, obs) = dem.sample(&mut rng);
    assert_eq!(obs, vec![true]);
}

#[test]
fn sample_detector_instr_ignored() {
    let mut dem = DetectorErrorModel::new();
    dem.add_detector(0, vec![1.0, 2.0]);
    dem.add_error(1.0, vec![DemTarget::Detector(0)]);
    let mut rng = StdRng::seed_from_u64(42);
    let (dets, _) = dem.sample(&mut rng);
    assert_eq!(dets[0], true);
}

#[test]
fn random_bits_with_prob_zero() {
    let mut dem = DetectorErrorModel::new();
    dem.add_error(0.0, vec![DemTarget::Detector(0)]);
    let mut rng = StdRng::seed_from_u64(42);
    let out = dem.sample_batch(100, &mut rng);
    let count: usize = (0..100).filter(|&s| out.detections.get(0, s)).count();
    assert_eq!(count, 0);
}

#[test]
fn random_bits_with_prob_one() {
    let mut dem = DetectorErrorModel::new();
    dem.add_error(1.0, vec![DemTarget::Detector(0)]);
    let mut rng = StdRng::seed_from_u64(42);
    let out = dem.sample_batch(100, &mut rng);
    let count: usize = (0..100).filter(|&s| out.detections.get(0, s)).count();
    assert_eq!(count, 100);
}

#[test]
fn sample_batch_observable_with_shift() {
    let mut body = DetectorErrorModel::new();
    body.add_error(1.0, vec![DemTarget::Detector(0), DemTarget::Observable(0)]);
    body.add_shift_detectors(1, vec![]);
    let mut dem = DetectorErrorModel::new();
    dem.add_repeat(2, body);
    let mut rng = StdRng::seed_from_u64(42);
    let out = dem.sample_batch(10, &mut rng);
    assert_eq!(out.detections.num_major(), 2);
    for shot in 0..10 {
        assert!(!out.observable_flips.get(0, shot));
    }
}

#[test]
fn set_min_counts() {
    let mut dem = DetectorErrorModel::new();
    dem.set_min_counts(5, 3);
    assert_eq!(dem.num_detectors(), 5);
    assert_eq!(dem.num_observables(), 3);
    dem.set_min_counts(2, 1);
    assert_eq!(dem.num_detectors(), 5);
    assert_eq!(dem.num_observables(), 3);
}

#[test]
fn parse_error_missing_paren() {
    let result = DetectorErrorModel::parse("error 0.1 D0");
    assert!(result.is_err());
}

#[test]
fn parse_error_bad_probability() {
    let result = DetectorErrorModel::parse("error(abc) D0");
    assert!(result.is_err());
}

#[test]
fn parse_unknown_instruction() {
    let result = DetectorErrorModel::parse("foobar(0.1) D0");
    assert!(result.is_err());
}

#[test]
fn parse_unexpected_close_brace() {
    let result = DetectorErrorModel::parse("}");
    assert!(result.is_err());
}

#[test]
fn parse_unclosed_repeat() {
    let result = DetectorErrorModel::parse("repeat 3 {\nerror(0.1) D0");
    assert!(result.is_err());
}
