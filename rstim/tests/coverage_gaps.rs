//! Targeted tests for remaining coverage gaps.
use rand::rngs::StdRng;
use rand::SeedableRng;
use rstim::executor::Executor;
use rstim::parser::parse_lines;
use rstim::sampler::sample_batch;

// --- executor.rs: CZ (lines 72-73) ---
#[test]
fn executor_cz_gate() {
    let instrs = parse_lines("H 0\nCZ 0 1\nH 0\nM 0 1").unwrap();
    let mut exec = Executor::from_instrs(instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(0);
    let out = exec.run(&mut rng).unwrap();
    // CZ on |+0> = |+0>, so M0 is random, M1 = 0
    assert!(!out.measurements[1]);
}

// --- executor.rs: REPEAT (lines 418-426) ---
#[test]
fn executor_repeat_block() {
    let instrs = parse_lines("REPEAT 3 {\n  M 0\n}").unwrap();
    let mut exec = Executor::from_instrs(instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(0);
    let out = exec.run(&mut rng).unwrap();
    assert_eq!(out.measurements.len(), 3);
}

// --- executor.rs: QUBIT_COORDS error (line 254) ---
#[test]
fn executor_qubit_coords_bad_target() {
    // QUBIT_COORDS with rec target should error
    let result = parse_lines("QUBIT_COORDS(0) rec[-1]");
    // parser may reject this; if not, executor should
    assert!(
        result.is_err() || {
            let instrs = result.unwrap();
            let mut exec = Executor::from_instrs(instrs).unwrap();
            let mut rng = StdRng::seed_from_u64(0);
            exec.run(&mut rng).is_err()
        }
    );
}

// --- executor.rs: sweep in qubits() (line 780-781) ---
#[test]
fn executor_sweep_in_single_qubit_gate_errors() {
    // H with sweep target — executor rejects sweep in non-CX context
    let instrs = parse_lines("H sweep[0]\nM 0").unwrap();
    let mut exec = Executor::from_instrs(instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(0);
    assert!(exec.run(&mut rng).is_err());
}

// --- executor.rs: QubitInv in qubits_with_inversion (line 793-794) ---
#[test]
fn executor_measure_with_inversion() {
    let instrs = parse_lines("X 0\nM !0").unwrap();
    let mut exec = Executor::from_instrs(instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(0);
    let out = exec.run(&mut rng).unwrap();
    // X flips to |1>, M !0 inverts → false
    assert!(!out.measurements[0]);
}

// --- executor.rs: sweep in qubits_with_inversion (line 793) ---
#[test]
fn executor_sweep_in_measurement() {
    let instrs = parse_lines("M sweep[0]").unwrap();
    let mut exec = Executor::from_instrs(instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(0);
    let out = exec.run(&mut rng).unwrap();
    // sweep target skipped, no measurements
    assert!(out.measurements.is_empty());
}

// --- executor.rs: MPP empty product (line 705, 944) ---
#[test]
fn executor_mpp_empty_product_inverted() {
    // MPP with !I (inverted identity) should return true
    let instrs = parse_lines("MPP !Z0*Z0\nM 0").unwrap();
    let mut exec = Executor::from_instrs(instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(0);
    let _out = exec.run(&mut rng).unwrap();
}

// --- executor.rs: CORRELATED_ERROR (line 415 — unsupported instruction error) ---
// This is the catch-all error; hard to hit without an actually unsupported instruction.

// --- recorder.rs: rec with positive offset (line 17), out of range (line 21) ---
#[test]
fn recorder_edge_cases() {
    use rstim::recorder::Recorder;
    let mut r = Recorder::default();
    r.push(true);
    assert_eq!(r.rec(1), None); // positive offset
    assert_eq!(r.rec(-5), None); // out of range
    assert_eq!(r.rec(-1), Some(true));
}

// --- ir.rs: name()/targets() on Repeat (lines 59, 66) ---
#[test]
fn ir_repeat_accessors() {
    let instrs = parse_lines("REPEAT 2 {\n  M 0\n}").unwrap();
    assert!(instrs[0].name().is_none());
    assert!(instrs[0].targets().is_none());
}

// --- ir.rs: Annotation constructors (lines 165-170) ---
#[test]
fn ir_annotation_constructors() {
    use rstim::ir::Annotation;
    let det = Annotation::detector(vec![1.0, 2.0], vec![-1]);
    assert!(det.observable_index.is_none());
    let obs = Annotation::observable_include(0, vec![-1]);
    assert_eq!(obs.observable_index, Some(0));
}

// --- frame sim: sweep in CX pair (lines 819-820, 831-832, 840, 851, 855) ---
#[test]
fn frame_sim_sweep_in_cx_pair() {
    let instrs = parse_lines("CX sweep[0] 1\nM 1").unwrap();
    let mut rng = StdRng::seed_from_u64(0);
    let result = sample_batch(&instrs, 4, &mut rng).unwrap();
    for shot in 0..4 {
        assert!(!result.measurements.get(0, shot));
    }
}

// --- frame sim: MPP multi-product (lines 970-974, 977, 985) ---
#[test]
fn frame_sim_mpp_multi_product() {
    // MPP Z0 Z1 — two separate products
    let instrs = parse_lines("MPP Z0 Z1").unwrap();
    let mut rng = StdRng::seed_from_u64(0);
    let result = sample_batch(&instrs, 4, &mut rng).unwrap();
    assert_eq!(result.measurements.num_major(), 2);
}

// --- frame sim: MPP empty product inverted (lines 624-627, 629, 631) ---
#[test]
fn frame_sim_mpp_inverted() {
    let instrs = parse_lines("MPP !Z0\nM 0").unwrap();
    let mut rng = StdRng::seed_from_u64(0);
    let result = sample_batch(&instrs, 4, &mut rng).unwrap();
    // !Z0 on |0> = !(+1) = true
    for shot in 0..4 {
        assert!(result.measurements.get(0, shot));
    }
}

// --- frame sim: SPP empty product (line 673) ---
#[test]
fn frame_sim_spp_basic() {
    let instrs = parse_lines("SPP Z0\nM 0").unwrap();
    let mut rng = StdRng::seed_from_u64(0);
    let result = sample_batch(&instrs, 2, &mut rng).unwrap();
    assert_eq!(result.measurements.num_major(), 1);
}

// --- frame sim: DETECTOR with ref_parity (lines 578, 583) ---
#[test]
fn frame_sim_detector_with_ref_parity() {
    // X 0 makes reference M=1; actual also M=1, so detector should not fire
    // But frame sim XORs with reference, so if all shots measure 1, det=0
    let circuit = "X 0\nM 0\nDETECTOR rec[-1]";
    let instrs = parse_lines(circuit).unwrap();
    let mut rng = StdRng::seed_from_u64(0);
    let result = sample_batch(&instrs, 2, &mut rng).unwrap();
    // Just verify it runs without error — ref_parity path is exercised
    assert_eq!(result.detections.num_major(), 1);
}

// --- explain_errors.rs: REPEAT path (lines 64-69) via DEM with repeat ---
#[test]
fn explain_errors_dem_with_repeat() {
    use rstim::error_analyzer::ErrorAnalyzer;
    use rstim::explain_errors::explain;
    // Circuit with REPEAT produces DEM with REPEAT block
    let circuit = "REPEAT 2 {\n  X_ERROR(0.1) 0\n  M 0\n  DETECTOR rec[-1]\n}";
    let instrs = parse_lines(circuit).unwrap();
    let dem = ErrorAnalyzer::circuit_to_dem(&instrs).unwrap();
    // Fire detector 1 (second round)
    let explanations = explain(&dem, &[1]);
    assert!(!explanations.is_empty());
    assert!(explanations[0].detectors.contains(&1));
}

// --- explain_errors.rs: empty DEM (line 28 — None branch) ---
#[test]
fn explain_errors_empty_dem() {
    use rstim::dem::DetectorErrorModel;
    use rstim::explain_errors::explain;
    let dem = DetectorErrorModel::new();
    let explanations = explain(&dem, &[0]);
    assert!(explanations.is_empty());
}

// --- cli.rs: run_explain_errors with dem_text (line 501) ---
#[test]
fn explain_errors_cli_with_dem_text() {
    use rstim::cli::run_explain_errors;
    let dem_text = "error(0.1) D0\n";
    let dets_input = b"shot D0\n";
    let mut out = Vec::new();
    run_explain_errors("", Some(dem_text), dets_input, "dets", &mut out).unwrap();
    let text = String::from_utf8(out).unwrap();
    assert!(text.contains("error("));
}

// --- cli.rs: run_convert with circuit instead of bits (lines 421-422) ---
#[test]
fn convert_with_circuit_for_bits() {
    use rstim::cli::run_convert;
    let circuit = "M 0 1";
    let input = b"10\n";
    let mut out = Vec::new();
    run_convert(input, "01", "b8", None, Some(circuit), None, &mut out).unwrap();
    assert!(!out.is_empty());
}

// --- cli.rs: run_m2d with ptb64 input (lines 468-469) ---
#[test]
fn m2d_ptb64_input() {
    use rstim::cli::run_m2d;
    use rstim::output::write_shots_ptb64;
    use rstim::sim::bit_table::BitTable;
    let circuit = "R 0\nM 0\nDETECTOR rec[-1]";
    // 1 measurement, 1 shot, bit=1
    let mut t = BitTable::new(1, 1);
    t.set(0, 0, true);
    let mut ptb_data = Vec::new();
    write_shots_ptb64(&t, &mut ptb_data).unwrap();
    let mut out = Vec::new();
    run_m2d(circuit, &ptb_data, "ptb64", "01", Some(1), false, &mut out).unwrap();
    assert_eq!(out, b"1\n");
}

// --- codegen/surface_code.rs: unrotated with noise (lines 411-412, 433-434) ---
#[test]
fn unrotated_surface_code_with_noise() {
    use rstim::codegen::surface_code::unrotated_memory_x;
    use rstim::ir::StimInstr;
    let instrs = unrotated_memory_x(3, 1, 0.001);
    let has_noise = instrs.iter().any(|i| {
        matches!(i, StimInstr::Op { name, .. } if name == "DEPOLARIZE1" || name == "DEPOLARIZE2")
    });
    assert!(has_noise);
}

// --- codegen/surface_code.rs: unrotated memory_z with noise ---
#[test]
fn unrotated_surface_code_z_with_noise() {
    use rstim::codegen::surface_code::unrotated_memory_z;
    use rstim::ir::StimInstr;
    let instrs = unrotated_memory_z(3, 1, 0.001);
    let has_noise = instrs.iter().any(|i| {
        matches!(i, StimInstr::Op { name, .. } if name == "DEPOLARIZE1" || name == "DEPOLARIZE2")
    });
    assert!(has_noise);
}

#[test]
fn loss_visible_variants_cover_executor_paths() {
    let circuit = "LOSS(1) 0\n\
                   MXL 0\n\
                   LOSS(1) 0\n\
                   MYL 0\n\
                   LOSS(1) 0\n\
                   MRXL 0\n\
                   LOSS(1) 0\n\
                   MRYL 0\n\
                   LOSS(1) 0 1\n\
                   MPP X0*X1\n\
                   MXX 0 1\n\
                   MYY 0 1\n\
                   MZZ 0 1\n";
    let instrs = parse_lines(circuit).unwrap();

    let mut exec = Executor::from_instrs(instrs.clone()).unwrap();
    let mut rng = StdRng::seed_from_u64(0);
    let out = exec.run(&mut rng).unwrap();
    assert_eq!(out.measurements.len(), 12);
    assert!(out.measurements.iter().all(|&b| b));
}

#[test]
fn loss_visible_variants_cover_reference_sample_paths() {
    use rstim::executor::reference_sample;

    let circuit = "H 0\n\
                   MXL 0\n\
                   H 0\n\
                   S 0\n\
                   MYL 0\n\
                   H 0\n\
                   MRXL 0\n\
                   MRXL 0\n\
                   H 0\n\
                   S 0\n\
                   MRYL 0\n\
                   MRYL 0\n\
                   H 0\n\
                   H 1\n\
                   MPP X0*X1\n\
                   H 0\n\
                   H 1\n\
                   MXX 0 1\n\
                   H 0\n\
                   S 0\n\
                   H 1\n\
                   S 1\n\
                   MYY 0 1\n\
                   MZZ 0 1\n";
    let instrs = parse_lines(circuit).unwrap();
    let ref_sample = reference_sample(&instrs).unwrap();
    assert_eq!(ref_sample.len(), 16);
    assert!(ref_sample.iter().all(|&b| !b));
}

#[test]
fn sample_batch_recurses_into_loss_visible_repeat() {
    let instrs = parse_lines("REPEAT 2 {\n  LOSS(1) 0\n  ML 0\n}\n").unwrap();
    let mut rng = StdRng::seed_from_u64(0);
    let out = sample_batch(&instrs, 1, &mut rng).unwrap();
    assert_eq!(out.measurements.num_major(), 4);
    for m in 0..4 {
        assert!(out.measurements.get(m, 0));
    }
}

#[test]
fn parser_accepts_empty_arg_lists_with_and_without_tags() {
    let plain = parse_lines("X_ERROR() 0\n").unwrap();
    match &plain[0] {
        rstim::ir::StimInstr::Op {
            name,
            args,
            targets,
            ..
        } => {
            assert_eq!(name, "X_ERROR");
            assert!(args.is_empty());
            assert_eq!(targets, &vec![rstim::ir::StimTarget::Qubit(0)]);
        }
        _ => panic!("expected Op"),
    }

    let tagged = parse_lines("X_ERROR[LEAK]() 0\n").unwrap();
    match &tagged[0] {
        rstim::ir::StimInstr::Op {
            name,
            tag,
            args,
            targets,
            ..
        } => {
            assert_eq!(name, "X_ERROR");
            assert_eq!(tag.as_deref(), Some("LEAK"));
            assert!(args.is_empty());
            assert_eq!(targets, &vec![rstim::ir::StimTarget::Qubit(0)]);
        }
        _ => panic!("expected Op"),
    }
}

#[test]
fn m2d_detector_rejects_non_rec_targets() {
    use rstim::m2d::measurements_to_detections;
    use rstim::sim::bit_table::BitTable;

    let instrs = parse_lines("R 0\nM 0\nDETECTOR rec[-1] 0\n").unwrap();
    let mut meas = BitTable::new(1, 1);
    meas.set(0, 0, false);
    match measurements_to_detections(&instrs, &meas) {
        Ok(_) => panic!("expected non-rec detector target to be rejected"),
        Err(err) => {
            assert!(err.contains("DETECTOR and OBSERVABLE_INCLUDE targets must be rec[-k]"));
        }
    }
}

// --- error_analyzer.rs: empty MPP product (lines 635-636) ---
#[test]
fn error_analyzer_mpp_with_observable() {
    use rstim::error_analyzer::ErrorAnalyzer;
    // MPP with observable — exercises undo_mpp path
    let circuit = "MPP Z0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]";
    let instrs = parse_lines(circuit).unwrap();
    let dem = ErrorAnalyzer::circuit_to_dem(&instrs).unwrap();
    assert!(dem.num_detectors() >= 1);
}

// --- error_analyzer.rs: DEPOLARIZE1 Z-only error (line 512) ---
#[test]
fn error_analyzer_depolarize1() {
    use rstim::error_analyzer::ErrorAnalyzer;
    let circuit = "DEPOLARIZE1(0.01) 0\nM 0\nDETECTOR rec[-1]";
    let instrs = parse_lines(circuit).unwrap();
    let dem = ErrorAnalyzer::circuit_to_dem(&instrs).unwrap();
    // Should produce error mechanisms
    assert!(!dem.instructions().is_empty());
}

// --- dem.rs: Separator in sampler (line 274) ---
#[test]
fn dem_sample_with_separator() {
    use rstim::dem::DetectorErrorModel;
    // DEM with separator: error(0.1) D0 ^ D1
    let dem_text = "error(0.1) D0 ^ D1";
    let dem = DetectorErrorModel::parse(dem_text).unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    let _result = dem.sample_batch(10, &mut rng);
}

// --- dem.rs: LogicalObservable/Detector formatting (lines 342-343) ---
#[test]
fn dem_format_logical_observable() {
    use rstim::dem::DetectorErrorModel;
    let dem_text = "error(0.1) D0 L0\ndetector(0, 0) D0\nlogical_observable L0";
    let dem = DetectorErrorModel::parse(dem_text).unwrap();
    let formatted = dem.to_string();
    assert!(formatted.contains("logical_observable L0"));
    assert!(formatted.contains("detector("));
}

// --- parser.rs: various uncovered error paths ---
#[test]
fn parser_error_paths() {
    // Bad float arg
    assert!(parse_lines("X_ERROR(abc) 0").is_err());
    // Unclosed paren
    assert!(parse_lines("X_ERROR(0.1 0").is_err());
}
