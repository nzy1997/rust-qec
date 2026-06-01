use rilpqec::{lower_dem_to_problem, IlpDecodeError};
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

#[test]
fn observable_only_terms_are_dropped_after_baseline_normalization_rules() {
    let high_dem = DetectorErrorModel::parse("error(0.75) L0\n").unwrap();
    let high_problem = lower_dem_to_problem(&high_dem).unwrap();
    assert!(high_problem.columns.is_empty());
    assert_eq!(high_problem.baseline_observables, vec![true]);

    let low_dem = DetectorErrorModel::parse("error(0.25) L0\n").unwrap();
    let low_problem = lower_dem_to_problem(&low_dem).unwrap();
    assert!(low_problem.columns.is_empty());
    assert_eq!(low_problem.baseline_observables, vec![false]);
}

#[test]
fn observables_from_correction_rejects_width_mismatch() {
    let dem = DetectorErrorModel::parse("error(0.25) D0 L0\n").unwrap();
    let problem = lower_dem_to_problem(&dem).unwrap();

    assert!(matches!(
        problem.observables_from_correction(&[]),
        Err(IlpDecodeError::CorrectionWidthMismatch {
            expected: 1,
            actual: 0,
        })
    ));
    assert!(matches!(
        problem.observables_from_correction(&[true, false]),
        Err(IlpDecodeError::CorrectionWidthMismatch {
            expected: 1,
            actual: 2,
        })
    ));
}

#[test]
fn observables_from_correction_applies_columns_when_width_matches() {
    let dem = DetectorErrorModel::parse("error(0.25) D0 L0\nerror(0.25) D1\n").unwrap();
    let problem = lower_dem_to_problem(&dem).unwrap();

    assert_eq!(
        problem.observables_from_correction(&[true, false]).unwrap(),
        vec![true]
    );
}
