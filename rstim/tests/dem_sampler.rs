use rand::SeedableRng;
use rand::rngs::StdRng;
use rstim::dem::{DemTarget, DetectorErrorModel};

#[test]
fn dem_sample_deterministic_error() {
    let mut dem = DetectorErrorModel::new();
    dem.add_error(1.0, vec![DemTarget::Detector(0), DemTarget::Observable(0)]);
    let mut rng = StdRng::seed_from_u64(42);
    let (dets, obs) = dem.sample(&mut rng);
    assert_eq!(dets, vec![true]);
    assert_eq!(obs, vec![true]);
}

#[test]
fn dem_sample_no_error() {
    let mut dem = DetectorErrorModel::new();
    dem.add_error(0.0, vec![DemTarget::Detector(0)]);
    dem.add_detector(0, vec![]);
    let mut rng = StdRng::seed_from_u64(42);
    let (dets, obs) = dem.sample(&mut rng);
    assert_eq!(dets, vec![false]);
    assert_eq!(obs, vec![]);
}

#[test]
fn dem_sample_statistical() {
    let mut dem = DetectorErrorModel::new();
    dem.add_error(0.5, vec![DemTarget::Detector(0)]);
    let mut rng = StdRng::seed_from_u64(42);
    let mut count = 0;
    for _ in 0..10000 {
        let (dets, _) = dem.sample(&mut rng);
        if dets[0] { count += 1; }
    }
    assert!((count as f64 / 10000.0 - 0.5).abs() < 0.05);
}

#[test]
fn dem_sample_two_errors_xor() {
    let mut dem = DetectorErrorModel::new();
    dem.add_error(1.0, vec![DemTarget::Detector(0)]);
    dem.add_error(1.0, vec![DemTarget::Detector(0)]);
    let mut rng = StdRng::seed_from_u64(42);
    let (dets, _) = dem.sample(&mut rng);
    assert_eq!(dets, vec![false]);
}

#[test]
fn dem_sample_with_repeat() {
    let mut body = DetectorErrorModel::new();
    body.add_error(1.0, vec![DemTarget::Detector(0)]);
    body.add_shift_detectors(1, vec![]);
    let mut dem = DetectorErrorModel::new();
    dem.add_repeat(3, body);
    let mut rng = StdRng::seed_from_u64(42);
    let (dets, _) = dem.sample(&mut rng);
    assert_eq!(dets, vec![true, true, true]);
}

#[test]
fn dem_sample_batch() {
    let mut dem = DetectorErrorModel::new();
    dem.add_error(0.5, vec![DemTarget::Detector(0)]);
    let mut rng = StdRng::seed_from_u64(42);
    let results = dem.sample_batch(1000, &mut rng);
    assert_eq!(results.detections.num_major(), 1);
    assert_eq!(results.detections.num_minor(), 1000);
    let count: usize = (0..1000).filter(|&s| results.detections.get(0, s)).count();
    assert!((count as f64 / 1000.0 - 0.5).abs() < 0.05);
}

#[test]
fn dem_sample_observable_only() {
    let mut dem = DetectorErrorModel::new();
    dem.add_error(1.0, vec![DemTarget::Observable(0)]);
    let mut rng = StdRng::seed_from_u64(42);
    let (dets, obs) = dem.sample(&mut rng);
    assert_eq!(dets, vec![]);
    assert_eq!(obs, vec![true]);
}

#[test]
fn dem_sample_multiple_detectors() {
    let mut dem = DetectorErrorModel::new();
    dem.add_error(1.0, vec![DemTarget::Detector(0), DemTarget::Detector(2)]);
    dem.add_detector(1, vec![]);
    let mut rng = StdRng::seed_from_u64(42);
    let (dets, _) = dem.sample(&mut rng);
    assert_eq!(dets, vec![true, false, true]);
}

#[test]
fn dem_sample_batch_observable_flips() {
    let mut dem = DetectorErrorModel::new();
    dem.add_error(1.0, vec![DemTarget::Observable(0)]);
    let mut rng = StdRng::seed_from_u64(42);
    let results = dem.sample_batch(100, &mut rng);
    assert_eq!(results.observable_flips.num_major(), 1);
    let all_true = (0..100).all(|s| results.observable_flips.get(0, s));
    assert!(all_true);
}

#[test]
fn dem_sample_batch_with_repeat() {
    let mut body = DetectorErrorModel::new();
    body.add_error(1.0, vec![DemTarget::Detector(0)]);
    body.add_shift_detectors(1, vec![]);
    let mut dem = DetectorErrorModel::new();
    dem.add_repeat(3, body);
    let mut rng = StdRng::seed_from_u64(42);
    let results = dem.sample_batch(10, &mut rng);
    assert_eq!(results.detections.num_major(), 3);
    for det in 0..3 {
        for shot in 0..10 {
            assert!(results.detections.get(det, shot));
        }
    }
}

#[test]
fn dem_sample_separator_ignored() {
    let mut dem = DetectorErrorModel::new();
    dem.add_error(1.0, vec![DemTarget::Detector(0), DemTarget::Separator, DemTarget::Detector(1)]);
    let mut rng = StdRng::seed_from_u64(42);
    let (dets, _) = dem.sample(&mut rng);
    assert_eq!(dets, vec![true, true]);
}
