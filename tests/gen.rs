use rstim::circuit_gen;
use rstim::stats;
use rstim::sampler::sample_batch;
use rstim::error_analyzer::ErrorAnalyzer;
use rand::SeedableRng;
use rand::rngs::StdRng;

#[test]
fn rep_code_basic_structure() {
    let instrs = circuit_gen::repetition_code_memory(3, 2, 0.001);
    assert!(stats::num_qubits(&instrs) > 0);
    assert!(stats::num_measurements(&instrs) > 0);
    assert!(stats::num_detectors(&instrs) > 0);
    assert_eq!(stats::num_observables(&instrs), 1);
}

#[test]
fn rep_code_noiseless_no_detections() {
    let instrs = circuit_gen::repetition_code_memory(3, 5, 0.0);
    let mut rng = StdRng::seed_from_u64(42);
    let result = sample_batch(&instrs, 100, &mut rng).unwrap();
    for shot in 0..100 {
        for det in 0..result.detections.num_major() {
            assert!(!result.detections.get(det, shot), "unexpected detection at d={det} shot={shot}");
        }
    }
}

#[test]
fn rep_code_produces_valid_dem() {
    let instrs = circuit_gen::repetition_code_memory(3, 3, 0.001);
    let dem = ErrorAnalyzer::circuit_to_dem(&instrs).unwrap();
    let dem_str = dem.to_string();
    assert!(dem_str.contains("error"));
    assert!(dem_str.contains("D"));
}

#[test]
fn rep_code_distance_5() {
    let instrs = circuit_gen::repetition_code_memory(5, 3, 0.001);
    assert_eq!(stats::num_qubits(&instrs), 9);
}

#[test]
fn rep_code_single_round() {
    let instrs = circuit_gen::repetition_code_memory(3, 1, 0.001);
    assert!(stats::num_detectors(&instrs) > 0);
}
