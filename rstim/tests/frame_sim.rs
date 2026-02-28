use rand::SeedableRng;
use rand::rngs::StdRng;
use rstim::sim::measure_record_batch::MeasureRecordBatch;
use rstim::sim::bit_table::BitTable;
use rstim::sim::frame::FrameSimulator;
use rstim::executor::reference_sample;
use rstim::parser::parse_lines;
use rstim::sampler::sample_batch;

#[test]
fn measure_record_batch_push_and_lookback() {
    let mut mrb = MeasureRecordBatch::new(64);
    let mut row = BitTable::new(1, 64);
    row.set(0, 0, true);
    mrb.push_row(row.row_words(0));
    assert_eq!(mrb.lookback(1, 0), true);
    assert_eq!(mrb.lookback(1, 1), false);
}

#[test]
fn measure_record_batch_multiple_rows() {
    let mut mrb = MeasureRecordBatch::new(64);
    let mut r1 = BitTable::new(1, 64);
    r1.set(0, 0, true);
    mrb.push_row(r1.row_words(0));
    let mut r2 = BitTable::new(1, 64);
    r2.set(0, 1, true);
    mrb.push_row(r2.row_words(0));
    assert_eq!(mrb.lookback(1, 0), false);  // r2, shot 0
    assert_eq!(mrb.lookback(1, 1), true);   // r2, shot 1
    assert_eq!(mrb.lookback(2, 0), true);   // r1, shot 0
    assert_eq!(mrb.lookback(2, 1), false);  // r1, shot 1
}

#[test]
fn reference_sample_deterministic() {
    let instrs = parse_lines("X 0\nM 0\n").unwrap();
    let ref_sample = reference_sample(&instrs).unwrap();
    assert_eq!(ref_sample, vec![true]);
}

#[test]
fn reference_sample_no_noise() {
    let instrs = parse_lines("X_ERROR(1) 0\nM 0\n").unwrap();
    let ref_sample = reference_sample(&instrs).unwrap();
    assert_eq!(ref_sample, vec![false]);
}

#[test]
fn reference_sample_bell() {
    let instrs = parse_lines("H 0\nCNOT 0 1\nM 0 1\n").unwrap();
    let ref_sample = reference_sample(&instrs).unwrap();
    assert_eq!(ref_sample.len(), 2);
    assert_eq!(ref_sample[0], ref_sample[1]); // correlated
    assert_eq!(ref_sample[0], false); // biased toward 0
}

#[test]
fn reference_sample_heralded_no_herald() {
    let instrs = parse_lines("HERALDED_ERASE(1) 0\nM 0\n").unwrap();
    let ref_sample = reference_sample(&instrs).unwrap();
    assert_eq!(ref_sample, vec![false, false]);
}

#[test]
fn reference_sample_mpad() {
    let instrs = parse_lines("MPAD 0 1\n").unwrap();
    let ref_sample = reference_sample(&instrs).unwrap();
    assert_eq!(ref_sample, vec![false, true]);
}

#[test]
fn reference_sample_mpp() {
    let instrs = parse_lines("MPP Z0\n").unwrap();
    let ref_sample = reference_sample(&instrs).unwrap();
    assert_eq!(ref_sample, vec![false]);
}

#[test]
fn reference_sample_reset() {
    let instrs = parse_lines("X 0\nR 0\nM 0\n").unwrap();
    let ref_sample = reference_sample(&instrs).unwrap();
    assert_eq!(ref_sample, vec![false]);
}

#[test]
fn frame_sim_no_noise_x_m() {
    let instrs = parse_lines("X 0\nM 0\n").unwrap();
    let ref_sample = reference_sample(&instrs).unwrap();
    assert_eq!(ref_sample, vec![true]);
    let mut rng = StdRng::seed_from_u64(42);
    let mut frame = FrameSimulator::new(1, 64);
    frame.run(&instrs, &ref_sample, &mut rng).unwrap();
    let m = frame.measurements(&ref_sample);
    for shot in 0..64 {
        assert_eq!(m.get(0, shot), true, "shot {shot}");
    }
}

#[test]
fn frame_sim_h_cnot_bell() {
    let instrs = parse_lines("H 0\nCNOT 0 1\nM 0 1\n").unwrap();
    let ref_sample = reference_sample(&instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    let mut frame = FrameSimulator::new(2, 256);
    frame.run(&instrs, &ref_sample, &mut rng).unwrap();
    let m = frame.measurements(&ref_sample);
    for shot in 0..256 {
        assert_eq!(m.get(0, shot), m.get(1, shot), "shot {shot}");
    }
}

#[test]
fn frame_sim_identity() {
    let instrs = parse_lines("M 0\n").unwrap();
    let ref_sample = reference_sample(&instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    let mut frame = FrameSimulator::new(1, 128);
    frame.run(&instrs, &ref_sample, &mut rng).unwrap();
    let m = frame.measurements(&ref_sample);
    for shot in 0..128 {
        assert_eq!(m.get(0, shot), false, "shot {shot}");
    }
}

#[test]
fn frame_sim_reset_then_measure() {
    let instrs = parse_lines("X 0\nR 0\nM 0\n").unwrap();
    let ref_sample = reference_sample(&instrs).unwrap();
    assert_eq!(ref_sample, vec![false]);
    let mut rng = StdRng::seed_from_u64(42);
    let mut frame = FrameSimulator::new(1, 64);
    frame.run(&instrs, &ref_sample, &mut rng).unwrap();
    let m = frame.measurements(&ref_sample);
    for shot in 0..64 {
        assert_eq!(m.get(0, shot), false, "shot {shot}");
    }
}

#[test]
fn frame_sim_measure_reset_measure() {
    let instrs = parse_lines("MR 0\nM 0\n").unwrap();
    let ref_sample = reference_sample(&instrs).unwrap();
    assert_eq!(ref_sample, vec![false, false]);
    let mut rng = StdRng::seed_from_u64(42);
    let mut frame = FrameSimulator::new(1, 64);
    frame.run(&instrs, &ref_sample, &mut rng).unwrap();
    let m = frame.measurements(&ref_sample);
    for shot in 0..64 {
        assert_eq!(m.get(0, shot), false, "shot {shot}");
        assert_eq!(m.get(1, shot), false, "shot {shot}");
    }
}

#[test]
fn frame_sim_x_error_all_flip() {
    let instrs = parse_lines("X_ERROR(1) 0\nM 0\n").unwrap();
    let ref_sample = reference_sample(&instrs).unwrap();
    assert_eq!(ref_sample, vec![false]);
    let mut rng = StdRng::seed_from_u64(42);
    let mut frame = FrameSimulator::new(1, 64);
    frame.run(&instrs, &ref_sample, &mut rng).unwrap();
    let m = frame.measurements(&ref_sample);
    for shot in 0..64 { assert_eq!(m.get(0, shot), true); }
}

#[test]
fn frame_sim_z_error_no_flip_z_measurement() {
    let instrs = parse_lines("Z_ERROR(1) 0\nM 0\n").unwrap();
    let ref_sample = reference_sample(&instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    let mut frame = FrameSimulator::new(1, 64);
    frame.run(&instrs, &ref_sample, &mut rng).unwrap();
    let m = frame.measurements(&ref_sample);
    for shot in 0..64 { assert_eq!(m.get(0, shot), false); }
}

#[test]
fn frame_sim_correlated_error() {
    let instrs = parse_lines("CORRELATED_ERROR(1) X0 X1\nM 0 1\n").unwrap();
    let ref_sample = reference_sample(&instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    let mut frame = FrameSimulator::new(2, 64);
    frame.run(&instrs, &ref_sample, &mut rng).unwrap();
    let m = frame.measurements(&ref_sample);
    for shot in 0..64 {
        assert_eq!(m.get(0, shot), true);
        assert_eq!(m.get(1, shot), true);
    }
}

#[test]
fn frame_sim_else_correlated_error() {
    let instrs = parse_lines("CORRELATED_ERROR(1) X0\nELSE_CORRELATED_ERROR(1) X1\nM 0 1\n").unwrap();
    let ref_sample = reference_sample(&instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    let mut frame = FrameSimulator::new(2, 64);
    frame.run(&instrs, &ref_sample, &mut rng).unwrap();
    let m = frame.measurements(&ref_sample);
    for shot in 0..64 {
        assert_eq!(m.get(0, shot), true);
        assert_eq!(m.get(1, shot), false);
    }
}

#[test]
fn frame_sim_depolarize1_statistical() {
    let instrs = parse_lines("DEPOLARIZE1(0.75) 0\nM 0\n").unwrap();
    let ref_sample = reference_sample(&instrs).unwrap();
    let n = 10000;
    let mut rng = StdRng::seed_from_u64(42);
    let mut frame = FrameSimulator::new(1, n);
    frame.run(&instrs, &ref_sample, &mut rng).unwrap();
    let m = frame.measurements(&ref_sample);
    let count: usize = (0..n).filter(|&s| m.get(0, s)).count();
    // X or Y flip Z measurement: 2/3 of 75% = 50%
    assert!((count as f64 / n as f64 - 0.5).abs() < 0.05, "count={count}");
}

#[test]
fn frame_sim_mpp_zz_bell() {
    let instrs = parse_lines("H 0\nCNOT 0 1\nMPP Z0*Z1\n").unwrap();
    let ref_sample = reference_sample(&instrs).unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    let mut frame = FrameSimulator::new(2, 128);
    frame.run(&instrs, &ref_sample, &mut rng).unwrap();
    let m = frame.measurements(&ref_sample);
    for shot in 0..128 {
        assert_eq!(m.get(0, shot), ref_sample[0]);
    }
}

#[test]
fn frame_sim_heralded_erase() {
    let instrs = parse_lines("HERALDED_ERASE(1) 0\nM 0\n").unwrap();
    let ref_sample = reference_sample(&instrs).unwrap();
    assert_eq!(ref_sample, vec![false, false]);
    let mut rng = StdRng::seed_from_u64(42);
    let mut frame = FrameSimulator::new(1, 64);
    frame.run(&instrs, &ref_sample, &mut rng).unwrap();
    let m = frame.measurements(&ref_sample);
    for shot in 0..64 { assert_eq!(m.get(0, shot), true); }
}

#[test]
fn sample_batch_deterministic() {
    let instrs = parse_lines("X 0\nM 0\n").unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    let out = sample_batch(&instrs, 100, &mut rng).unwrap();
    for shot in 0..100 {
        assert_eq!(out.measurements.get(0, shot), true);
    }
}

#[test]
fn sample_batch_detector_noiseless() {
    let instrs = parse_lines("M 0\nR 0\nM 0\nDETECTOR rec[-1] rec[-2]\n").unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    let out = sample_batch(&instrs, 64, &mut rng).unwrap();
    for shot in 0..64 {
        assert_eq!(out.detections.get(0, shot), false);
    }
}

#[test]
fn sample_batch_detector_with_noise() {
    let instrs = parse_lines("M 0\nR 0\nX_ERROR(1) 0\nM 0\nDETECTOR rec[-1] rec[-2]\n").unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    let out = sample_batch(&instrs, 64, &mut rng).unwrap();
    for shot in 0..64 {
        assert_eq!(out.detections.get(0, shot), true);
    }
}

#[test]
fn sample_batch_observable() {
    let instrs = parse_lines("X 0\nM 0\nOBSERVABLE_INCLUDE(0) rec[-1]\n").unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    let out = sample_batch(&instrs, 64, &mut rng).unwrap();
    for shot in 0..64 {
        assert_eq!(out.observable_flips.get(0, shot), true);
    }
}

#[test]
fn sample_batch_bell_correlated() {
    let instrs = parse_lines("H 0\nCNOT 0 1\nM 0 1\n").unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    let out = sample_batch(&instrs, 1000, &mut rng).unwrap();
    for shot in 0..1000 {
        assert_eq!(out.measurements.get(0, shot), out.measurements.get(1, shot));
    }
}

#[test]
fn sample_batch_repeat() {
    let instrs = parse_lines("REPEAT 3 {\nX 0\nM 0\nR 0\n}\n").unwrap();
    let mut rng = StdRng::seed_from_u64(42);
    let out = sample_batch(&instrs, 64, &mut rng).unwrap();
    for shot in 0..64 {
        for m_idx in 0..3 {
            assert_eq!(out.measurements.get(m_idx, shot), true);
        }
    }
}
