use rand::SeedableRng;
use rand::rngs::StdRng;
use rstim::interactive_shot::{EditableShot, ExpansionLimits};
use rstim::sample_trace::MeasurementComponent;
use rstim::sim::bit_table::BitTable;
use rstim::{executor::Executor, parser::parse_lines, sampler::sample_batch};

fn shot_bits(table: &BitTable, shot: usize, start: usize, end: usize) -> Vec<bool> {
    (start..end).map(|index| table.get(index, shot)).collect()
}

#[test]
fn incorrect_flags_do_not_change_which_qubits_can_take_part_in_gates() {
    let circuit = parse_lines(
        "R 0 1 2 3\n\
         X 0\n\
         LOSS(1) 1\n\
         ML(1) 0 1\n\
         DETECTOR rec[-2]\n\
         DETECTOR rec[-1]\n\
         X 1\n\
         CX 0 2 1 3\n\
         ML 0 1\n\
         M 2 3\n",
    )
    .unwrap();
    let shots = 4096;
    let sampled = sample_batch(&circuit, shots, &mut StdRng::seed_from_u64(42)).unwrap();
    let mut missed_ones = 0;
    for shot in 0..shots {
        let m = &sampled.measurements;
        assert_eq!(shot_bits(m, shot, 0, 3), [true, true, false]);
        let missed_value = m.get(3, shot);
        missed_ones += usize::from(missed_value);
        assert_eq!(
            shot_bits(m, shot, 4, 10),
            [false, true, true, true, true, false]
        );
        assert!(!sampled.detections.get(0, shot));
        assert_eq!(sampled.detections.get(1, shot), missed_value);
    }
    assert!((1900..=2200).contains(&missed_ones), "{missed_ones}");
}

#[test]
fn each_target_misclassifies_at_the_declared_rate_for_either_true_state() {
    let circuit = parse_lines("R 0 1\nLOSS(1) 1\nML(0.2) 0 1\n").unwrap();
    let shots = 16_384;
    let sampled = sample_batch(&circuit, shots, &mut StdRng::seed_from_u64(42)).unwrap();
    let mut false_positives = 0;
    let mut false_negatives = 0;
    let mut missed_ones = 0;
    let mut both = 0;
    for shot in 0..shots {
        let false_positive = sampled.measurements.get(0, shot);
        let false_negative = !sampled.measurements.get(2, shot);
        false_positives += usize::from(false_positive);
        false_negatives += usize::from(false_negative);
        both += usize::from(false_positive && false_negative);
        assert!(!sampled.measurements.get(1, shot));
        let lost_value = sampled.measurements.get(3, shot);
        if false_negative {
            missed_ones += usize::from(lost_value);
        } else {
            assert!(lost_value);
        }
    }
    let rate = |count| count as f64 / shots as f64;
    assert!((rate(false_positives) - 0.2).abs() < 0.015);
    assert!((rate(false_negatives) - 0.2).abs() < 0.015);
    assert!((rate(missed_ones) - 0.1).abs() < 0.012);
    assert!((rate(both) - 0.04).abs() < 0.01);
}

#[test]
fn loss_visible_measurements_preserve_inversion_and_reset_semantics() {
    for (readout, prepare, check, resets) in [
        ("ML", "R", "ML", false),
        ("MZL", "R", "ML", false),
        ("MXL", "RX", "MXL", false),
        ("MYL", "RY", "MYL", false),
        ("MRL", "R", "ML", true),
        ("MRZL", "R", "ML", true),
        ("MRXL", "RX", "MXL", true),
        ("MRYL", "RY", "MYL", true),
    ] {
        let circuit = format!("{prepare} 0 1\nLOSS(1) 1\n{readout}(1) !0 !1\n{check} 0 1\n");
        let mut executor = Executor::from_instrs(parse_lines(&circuit).unwrap()).unwrap();
        let shot = executor.run(&mut StdRng::seed_from_u64(42)).unwrap();
        assert_eq!(shot.measurements[..3], [true, true, false], "{readout}");
        assert_eq!(
            shot.measurements[4..],
            [false, false, !resets, !resets],
            "{readout}",
        );
    }
}

#[test]
fn shot_trace_distinguishes_the_reported_flag_from_physical_loss() {
    let circuit = "R 0 1\nLOSS(1) 1\nML(1) 0 1\n";
    let mut shot = EditableShot::open(circuit, ExpansionLimits::default(), 42).unwrap();
    assert!(shot.current_trace().noise_events.is_empty());
    shot.sample(42).unwrap();

    let trace = shot.current_trace();
    let classifications: Vec<_> = trace
        .noise_events
        .iter()
        .filter(|event| event.instr_name == "ML")
        .collect();
    assert_eq!(classifications.len(), 2);
    assert!(
        classifications
            .iter()
            .all(|event| event.branch_label.as_deref() == Some("flip"))
    );
    assert_eq!(
        trace.measurement_events[0].component,
        MeasurementComponent::LossFlag
    );
    assert!(trace.measurement_events[0].bit);
    assert!(!trace.measurement_events[1].loss_cause);
    assert!(!trace.measurement_events[2].bit);
    assert!(trace.measurement_events[3].loss_cause);

    let snapshot = shot.view_snapshot().unwrap();
    assert_eq!(
        snapshot
            .svg
            .matches("class=\"noise-site measurement\"")
            .count(),
        2
    );
    for ids in ["m1 m2", "m3 m4"] {
        assert!(
            snapshot.svg.lines().any(|line| {
                line.contains("class=\"noise-site measurement\"")
                    && line.contains("data-noise-event-id=")
                    && line.contains(&format!("data-measurement-ids=\"{ids}\""))
            }),
            "missing read-only classification hotspot for {ids}",
        );
    }
}

#[test]
fn classification_hotspots_use_qubit_slots_in_mixed_target_lists() {
    let circuit = "R 0 1\nML(1) sweep[0] 0 sweep[1] !1\n";
    let mut shot = EditableShot::open(circuit, ExpansionLimits::default(), 42).unwrap();
    shot.sample(42).unwrap();
    let svg = shot.view_snapshot().unwrap().svg;
    let events = shot.session().catalog().events();
    assert_eq!(events.len(), 2);

    for ((slot, record_ids), event) in [(1, "m1 m2"), (3, "m3 m4")].into_iter().zip(events) {
        assert_eq!(event.id.site.target_slots, [slot]);
        let event_id = event.id.encode();
        assert!(
            svg.lines().any(|line| {
                line.contains(&format!("data-noise-event-id=\"{event_id}\""))
                    && line.contains(&format!("data-measurement-ids=\"{record_ids}\""))
            }),
            "incorrect event on {record_ids}",
        );
    }
    assert!(svg.contains(">L=1 | M=0</text>"));
    assert!(svg.contains(">L=1 | M=1</text>"));
}

#[test]
fn zero_probability_preserves_seeded_sampling_and_invalid_arguments_are_rejected() {
    let run = |circuit| {
        let instructions = parse_lines(circuit).unwrap();
        let mut executor = Executor::from_instrs(instructions).unwrap();
        let mut rng = StdRng::seed_from_u64(42);
        (0..64)
            .map(|_| executor.run(&mut rng).unwrap().measurements)
            .collect::<Vec<_>>()
    };
    let ideal_flags = run("R 0 1\nLOSS(0.2) 1\nH 0\nML 0 1\n");
    assert_eq!(ideal_flags, run("R 0 1\nLOSS(0.2) 1\nH 0\nML(0) 0 1\n"));
    let ideal_values: Vec<_> = ideal_flags.iter().map(|row| vec![row[1], row[3]]).collect();
    assert_eq!(ideal_values, run("R 0 1\nLOSS(0.2) 1\nH 0\nM 0 1\n"));
    for circuit in ["ML(-0.1) 0", "ML(1.1) 0", "MRL(0.2,0.3) 0", "MXL(NaN) 0"] {
        assert!(
            rstim::validation::parse_and_validate(circuit).is_err(),
            "{circuit}"
        );
    }
}
