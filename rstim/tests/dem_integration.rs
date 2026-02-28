use rand::SeedableRng;
use rand::rngs::StdRng;
use rstim::parser::parse_lines;
use rstim::error_analyzer::ErrorAnalyzer;
use rstim::sampler::sample_batch;

#[test]
fn circuit_vs_dem_detection_rates() {
    let circuit = "\
R 0 1
X_ERROR(0.05) 0
M 0 1
DETECTOR rec[-2]
DETECTOR rec[-1]
";
    let instrs = parse_lines(circuit).unwrap();

    let mut rng = StdRng::seed_from_u64(42);
    let circuit_out = sample_batch(&instrs, 10000, &mut rng).unwrap();
    let circuit_d0: usize = (0..10000).filter(|&s| circuit_out.detections.get(0, s)).count();
    let circuit_d1: usize = (0..10000).filter(|&s| circuit_out.detections.get(1, s)).count();

    let dem = ErrorAnalyzer::circuit_to_dem(&instrs).unwrap();
    let mut rng2 = StdRng::seed_from_u64(99);
    let dem_out = dem.sample_batch(10000, &mut rng2);
    let dem_d0: usize = (0..10000).filter(|&s| dem_out.detections.get(0, s)).count();
    let dem_d1: usize = (0..10000).filter(|&s| dem_out.detections.get(1, s)).count();

    let circuit_rate_0 = circuit_d0 as f64 / 10000.0;
    let dem_rate_0 = dem_d0 as f64 / 10000.0;
    assert!((circuit_rate_0 - dem_rate_0).abs() < 0.03,
        "circuit={circuit_rate_0}, dem={dem_rate_0}");
    assert!((circuit_d1 as f64 / 10000.0) < 0.01);
    assert!((dem_d1 as f64 / 10000.0) < 0.01);
}

#[test]
fn circuit_to_dem_to_string_round_trip() {
    let circuit = "\
X_ERROR(0.1) 0
M 0
DETECTOR rec[-1]
OBSERVABLE_INCLUDE(0) rec[-1]
";
    let instrs = parse_lines(circuit).unwrap();
    let dem = ErrorAnalyzer::circuit_to_dem(&instrs).unwrap();
    let dem_str = dem.to_string();
    assert!(dem_str.contains("error"));
    assert!(dem_str.contains("D0"));
    assert!(dem_str.contains("L0"));

    let dem2 = rstim::dem::DetectorErrorModel::parse(&dem_str).unwrap();
    assert_eq!(dem.instructions().len(), dem2.instructions().len());
}

#[test]
fn repetition_code_circuit_to_dem() {
    let circuit = "\
R 0 1 2 3
TICK
CNOT 0 1
CNOT 2 1
CNOT 2 3
TICK
M 1 3
DETECTOR rec[-2]
DETECTOR rec[-1]
REPEAT 2 {
    R 1 3
    TICK
    X_ERROR(0.01) 0 2
    CNOT 0 1
    CNOT 2 1
    CNOT 2 3
    TICK
    M 1 3
    DETECTOR rec[-2] rec[-4]
    DETECTOR rec[-1] rec[-3]
}
M 0 2
OBSERVABLE_INCLUDE(0) rec[-1] rec[-2]
";
    let instrs = parse_lines(circuit).unwrap();
    let dem = ErrorAnalyzer::circuit_to_dem(&instrs).unwrap();
    assert!(!dem.instructions().is_empty());
    let dem_str = dem.to_string();
    assert!(dem_str.contains("error"));
}

#[test]
fn cx_propagation_cross_validate() {
    let circuit = "\
R 0 1
X_ERROR(0.1) 0
CX 0 1
M 0 1
DETECTOR rec[-2]
DETECTOR rec[-1]
";
    let instrs = parse_lines(circuit).unwrap();
    let dem = ErrorAnalyzer::circuit_to_dem(&instrs).unwrap();

    let dem_str = dem.to_string();
    assert!(dem_str.contains("D0"));
    assert!(dem_str.contains("D1"));

    let mut rng = StdRng::seed_from_u64(42);
    let circuit_out = sample_batch(&instrs, 10000, &mut rng).unwrap();
    let c_both: usize = (0..10000).filter(|&s|
        circuit_out.detections.get(0, s) && circuit_out.detections.get(1, s)
    ).count();
    let c_rate = c_both as f64 / 10000.0;

    let mut rng2 = StdRng::seed_from_u64(99);
    let dem_out = dem.sample_batch(10000, &mut rng2);
    let d_both: usize = (0..10000).filter(|&s|
        dem_out.detections.get(0, s) && dem_out.detections.get(1, s)
    ).count();
    let d_rate = d_both as f64 / 10000.0;

    assert!((c_rate - 0.1).abs() < 0.03, "circuit rate={c_rate}");
    assert!((d_rate - 0.1).abs() < 0.03, "dem rate={d_rate}");
}

#[test]
fn observable_flip_rate_cross_validate() {
    let circuit = "\
R 0
X_ERROR(0.05) 0
M 0
OBSERVABLE_INCLUDE(0) rec[-1]
";
    let instrs = parse_lines(circuit).unwrap();

    let mut rng = StdRng::seed_from_u64(42);
    let circuit_out = sample_batch(&instrs, 10000, &mut rng).unwrap();
    let c_flips: usize = (0..10000).filter(|&s| circuit_out.observable_flips.get(0, s)).count();

    let dem = ErrorAnalyzer::circuit_to_dem(&instrs).unwrap();
    let mut rng2 = StdRng::seed_from_u64(99);
    let dem_out = dem.sample_batch(10000, &mut rng2);
    let d_flips: usize = (0..10000).filter(|&s| dem_out.observable_flips.get(0, s)).count();

    let c_rate = c_flips as f64 / 10000.0;
    let d_rate = d_flips as f64 / 10000.0;
    assert!((c_rate - 0.05).abs() < 0.03, "circuit={c_rate}");
    assert!((d_rate - 0.05).abs() < 0.03, "dem={d_rate}");
}

#[test]
fn dem_empty_circuit_produces_empty_dem() {
    let instrs = parse_lines("R 0\nM 0").unwrap();
    let dem = ErrorAnalyzer::circuit_to_dem(&instrs).unwrap();
    assert!(dem.instructions().is_empty());
}
