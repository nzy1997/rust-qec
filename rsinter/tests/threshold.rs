use rsinter::collect::CollectOptions;
use rstim::dem::{DemInstruction, DemTarget, DetectorErrorModel};

fn assert_all_graphlike_dem(dem: &DetectorErrorModel) {
    fn recurse(instrs: &[DemInstruction]) {
        for instr in instrs {
            match instr {
                DemInstruction::Error { targets, .. } => {
                    let mut det_count = 0usize;
                    for target in targets {
                        match target {
                            DemTarget::Detector(_) => det_count += 1,
                            DemTarget::Separator => det_count = 0,
                            DemTarget::Observable(_) => {}
                        }
                        assert!(det_count <= 2, "non-graphlike component: {targets:?}");
                    }
                }
                DemInstruction::Repeat { body, .. } => recurse(body.instructions()),
                DemInstruction::Detector { .. }
                | DemInstruction::LogicalObservable { .. }
                | DemInstruction::ShiftDetectors { .. } => {}
            }
        }
    }

    recurse(dem.instructions());
}

#[test]
fn make_rotated_surface_code_threshold_tasks_produces_mwpm_graphlike_tasks() {
    let tasks = rsinter::threshold::make_rotated_surface_code_threshold_tasks(&[3], &[0.001], 128)
        .unwrap();
    assert_eq!(tasks.len(), 1);

    let task = &tasks[0];
    assert_eq!(task.decoder, "mwpm");
    assert_eq!(task.collection_options.max_shots, Some(128));
    assert_eq!(task.metadata["d"].as_u64(), Some(3));
    assert_eq!(task.metadata["r"].as_u64(), Some(9));
    assert_eq!(task.metadata["p"].as_f64(), Some(0.001));
    assert!(!task.circuit.is_empty());
    assert_all_graphlike_dem(&task.dem);
}

#[test]
fn collect_rotated_surface_code_threshold_runs_real_mwpm_sampling_sweep() {
    let options = CollectOptions {
        num_workers: 1,
        max_shots: None,
        max_errors: None,
        max_batch_size: Some(64),
        start_batch_size: 32,
        save_resume_filepath: None,
        print_progress: false,
    };

    let stats = rsinter::threshold::collect_rotated_surface_code_threshold(
        &[3],
        &[0.001, 0.002],
        64,
        &options,
    )
    .unwrap();

    assert_eq!(stats.len(), 2);
    for stat in &stats {
        assert_eq!(stat.decoder, "mwpm");
        assert_eq!(stat.metadata["d"].as_u64(), Some(3));
        assert!(matches!(stat.metadata["p"].as_f64(), Some(0.001 | 0.002)));
        assert!(stat.shots >= 64);
        assert!(stat.errors <= stat.shots);
    }
}

#[test]
fn collect_rotated_surface_code_threshold_d7_high_noise_does_not_panic() {
    let options = CollectOptions {
        num_workers: 1,
        max_shots: None,
        max_errors: None,
        max_batch_size: Some(128),
        start_batch_size: 64,
        save_resume_filepath: None,
        print_progress: false,
    };

    let stats = rsinter::threshold::collect_rotated_surface_code_threshold(
        &[7],
        &[0.012],
        2_048,
        &options,
    )
    .unwrap();

    assert_eq!(stats.len(), 1);
    assert!(stats[0].shots >= 2_048);
}

#[test]
fn stim_notebook_collection_budget_matches_stim_reference() {
    let options = rsinter::threshold::stim_surface_code_threshold_collect_options(4);
    assert_eq!(rsinter::threshold::STIM_SURFACE_CODE_THRESHOLD_MAX_SHOTS, 1_000_000);
    assert_eq!(rsinter::threshold::STIM_SURFACE_CODE_THRESHOLD_MAX_ERRORS, 5_000);
    assert_eq!(options.num_workers, 4);
    assert_eq!(options.max_shots, Some(1_000_000));
    assert_eq!(options.max_errors, Some(5_000));
}
