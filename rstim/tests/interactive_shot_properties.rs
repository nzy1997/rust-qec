use std::collections::HashSet;

use proptest::prelude::*;
use rstim::interactive_shot::{EditableShot, ExpansionLimits, NoiseEventId, NoiseOutcome};

fn bounded_circuit(repeat: u8) -> String {
    format!(
        "R 0 1\nREPEAT {repeat} {{\n X_ERROR(0.25) 0\n DEPOLARIZE1(0.4) 1\n M 0 1\n DETECTOR rec[-2] rec[-1]\n OBSERVABLE_INCLUDE(0) rec[-1]\n R 0 1\n}}\n"
    )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]

    #[test]
    fn stable_ids_are_unique_deterministic_and_round_trip(repeat in 1_u8..8, seed in any::<u64>()) {
        let source = bounded_circuit(repeat);
        let left = EditableShot::open(&source, ExpansionLimits::default(), seed).unwrap();
        let right = EditableShot::open(&source, ExpansionLimits::default(), seed).unwrap();
        let left_ids = left.session().catalog().events().iter().map(|event| event.id.encode()).collect::<Vec<_>>();
        let right_ids = right.session().catalog().events().iter().map(|event| event.id.encode()).collect::<Vec<_>>();
        prop_assert_eq!(&left_ids, &right_ids);
        prop_assert_eq!(left_ids.iter().collect::<HashSet<_>>().len(), left_ids.len());
        for encoded in left_ids {
            prop_assert_eq!(NoiseEventId::decode(&encoded).unwrap().encode(), encoded);
        }
    }

    #[test]
    fn set_restore_and_undo_redo_preserve_semantic_snapshots(repeat in 1_u8..6, seed in any::<u64>(), event_index in 0_usize..10) {
        let source = bounded_circuit(repeat);
        let mut shot = EditableShot::open(&source, ExpansionLimits::default(), seed).unwrap();
        let events = shot.session().catalog().events();
        let event_id = events[event_index % events.len()].id.clone();
        let outcome = NoiseOutcome::X;
        let initial = shot.summary();

        shot.set_noise(&event_id, outcome).unwrap();
        let edited = shot.summary();
        prop_assert_ne!(&edited.result, &initial.result);

        prop_assert!(shot.undo().unwrap());
        prop_assert_eq!(&shot.summary().result, &initial.result);
        prop_assert!(shot.redo().unwrap());
        prop_assert_eq!(&shot.summary().result, &edited.result);

        shot.restore_noise(&event_id).unwrap();
        prop_assert_eq!(&shot.summary().result, &initial.result);
    }

    #[test]
    fn no_error_base_has_no_declared_noise_without_overrides(repeat in 1_u8..8, seed in any::<u64>()) {
        let source = bounded_circuit(repeat);
        let shot = EditableShot::open(&source, ExpansionLimits::default(), seed).unwrap();
        for event in shot.summary().result.noise_events {
            prop_assert_eq!(event.effective_outcome, NoiseOutcome::Identity);
            prop_assert!(event.override_outcome.is_none());
        }
    }

    #[test]
    fn byte_equivalent_snapshots_follow_identical_commands(seed in any::<u64>(), sample_seed in any::<u64>()) {
        let source = bounded_circuit(3);
        let mut left = EditableShot::open(&source, ExpansionLimits::default(), seed).unwrap();
        let mut right = EditableShot::open(&source, ExpansionLimits::default(), seed).unwrap();
        left.sample(sample_seed).unwrap();
        right.sample(sample_seed).unwrap();
        let event_id = left.session().catalog().events()[3].id.clone();
        left.set_noise(&event_id, NoiseOutcome::X).unwrap();
        right.set_noise(&event_id, NoiseOutcome::X).unwrap();
        let left_json = serde_json::to_vec(&left.view_snapshot().unwrap()).unwrap();
        let right_json = serde_json::to_vec(&right.view_snapshot().unwrap()).unwrap();
        prop_assert_eq!(left_json, right_json);
    }
}

#[test]
fn thousand_edit_history_sequence_round_trips_without_crossing_shot_boundary() {
    let source = bounded_circuit(4);
    let mut shot = EditableShot::open(&source, ExpansionLimits::default(), 19).unwrap();
    let ids = shot
        .session()
        .catalog()
        .events()
        .iter()
        .map(|event| event.id.clone())
        .collect::<Vec<_>>();
    let initial = shot.summary().result;
    for step in 0..1_000 {
        let outcome = if (step / ids.len()) % 2 == 0 {
            NoiseOutcome::X
        } else {
            NoiseOutcome::Identity
        };
        shot.set_noise(&ids[step % ids.len()], outcome).unwrap();
    }
    let final_result = shot.summary().result;
    let mut undo_count = 0;
    while shot.undo().unwrap() {
        undo_count += 1;
    }
    assert_eq!(undo_count, 1_000);
    assert_eq!(shot.summary().result, initial);
    let mut redo_count = 0;
    while shot.redo().unwrap() {
        redo_count += 1;
    }
    assert_eq!(redo_count, 1_000);
    assert_eq!(shot.summary().result, final_result);

    shot.sample(23).unwrap();
    assert!(!shot.undo().unwrap());
    assert!(!shot.redo().unwrap());
}
