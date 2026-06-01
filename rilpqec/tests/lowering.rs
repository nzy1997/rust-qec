use rilpqec::lower_dem_to_problem;
use rstim::dem::DetectorErrorModel;

#[test]
fn separator_targets_are_merged_into_one_column() {
    let dem = DetectorErrorModel::parse("error(0.25) D0 ^ D1 L0\n").unwrap();

    let problem = lower_dem_to_problem(&dem).unwrap();

    assert_eq!(problem.num_detectors, 2);
    assert_eq!(problem.num_observables, 1);
    assert_eq!(problem.columns.len(), 1);
    assert_eq!(problem.columns[0].detectors, vec![0, 1]);
    assert_eq!(problem.columns[0].observables, vec![0]);
}

#[test]
fn repeat_shift_and_probability_extremes_become_forced_state_or_drop() {
    let dem = DetectorErrorModel::parse(
        "error(1) D0 L0\nerror(0) D0\nrepeat 2 {\n    error(0.75) D0 D1 L0\n    shift_detectors 3\n}\nerror(0.25) L0\n",
    )
    .unwrap();

    let problem = lower_dem_to_problem(&dem).unwrap();

    assert_eq!(problem.num_detectors, 5);
    assert_eq!(problem.num_observables, 1);
    assert_eq!(problem.columns.len(), 2);
    assert_eq!(
        problem.forced_syndrome,
        vec![false, true, false, true, true]
    );
    assert_eq!(problem.baseline_observables, vec![true]);
    assert_eq!(problem.columns[0].detectors, vec![0, 1]);
    assert_eq!(problem.columns[1].detectors, vec![3, 4]);
    assert!(problem.columns.iter().all(|term| term.weight > 0.0));
}
