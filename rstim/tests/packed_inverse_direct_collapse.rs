use rstim::data_path::ReferenceBuildPhaseCounters;
use rstim::sim::packed_inverse_tableau::{CanonicalTableauSnapshot, PackedInverseTableau};
use rstim::sim::tableau::StabilizerState;

fn direct_collapse(
    tableau: &mut PackedInverseTableau,
    targets: &[(usize, bool)],
) -> (Vec<bool>, ReferenceBuildPhaseCounters) {
    let mut counters = ReferenceBuildPhaseCounters::default();
    let bits = tableau.collapse_z_many_biased(targets, &mut counters);
    (bits, counters)
}

fn legacy_measure_z_many(
    state: &mut StabilizerState,
    targets: &[(usize, bool)],
) -> Vec<bool> {
    targets
        .iter()
        .map(|&(q, inverted)| (state.measure_z_biased(q) == 1) ^ inverted)
        .collect()
}

fn assert_snapshot_matches_legacy(
    packed: &PackedInverseTableau,
    legacy: &StabilizerState,
    label: &str,
) {
    let packed_snapshot: CanonicalTableauSnapshot = packed.canonical_snapshot();
    let legacy_snapshot = legacy.canonical_snapshot();
    assert_eq!(packed_snapshot, legacy_snapshot, "{label}");
}

#[test]
fn deterministic_z_batch_avoids_transpose() {
    let mut tableau = PackedInverseTableau::identity(3);
    tableau.x_gate(1);
    tableau.z_gate(2);

    let (bits, counters) = direct_collapse(&mut tableau, &[(0, false), (1, false), (2, true)]);

    assert_eq!(bits, vec![false, true, true]);
    assert_eq!(counters.direct_inverse_batches, 1);
    assert_eq!(counters.transposed_collapse_batches, 0);
    assert_eq!(counters.canonical_materializations, 0);
    assert_eq!(counters.canonical_writebacks, 0);
    assert_eq!(counters.collapse_pivots, 0);
}

#[test]
fn random_z_collapse_matches_legacy_tableau() {
    let mut packed = PackedInverseTableau::identity(4);
    let mut legacy = StabilizerState::new(4);
    for q in [0, 2] {
        packed.h(q);
        legacy.h(q);
    }
    packed.cx(0, 1);
    legacy.cx(0, 1);
    packed.z_gate(2);
    legacy.z_gate(2);

    let targets = [(0, false), (1, false), (2, false), (3, true)];
    let (packed_bits, counters) = direct_collapse(&mut packed, &targets);
    let legacy_bits = legacy_measure_z_many(&mut legacy, &targets);

    assert_eq!(packed_bits, legacy_bits);
    assert_eq!(counters.direct_inverse_batches, 1);
    assert_eq!(counters.transposed_collapse_batches, 1);
    assert!(counters.collapse_pivots >= 1);
    assert_snapshot_matches_legacy(&packed, &legacy, "random collapse snapshot");
}

#[test]
fn mixed_z_batch_reuses_one_transposed_view() {
    let mut tableau = PackedInverseTableau::identity(5);
    tableau.x_gate(0);
    tableau.h(1);
    tableau.h(3);

    let (bits, counters) =
        direct_collapse(&mut tableau, &[(0, false), (1, false), (2, false), (3, true)]);

    assert_eq!(bits, vec![true, false, false, true]);
    assert_eq!(counters.direct_inverse_batches, 1);
    assert_eq!(counters.transposed_collapse_batches, 1);
    assert_eq!(counters.canonical_materializations, 0);
    assert_eq!(counters.canonical_writebacks, 0);
    assert_eq!(counters.collapse_pivots, 2);
}

#[test]
fn direct_collapse_preserves_deterministic_one() {
    let mut tableau = PackedInverseTableau::identity(1);
    tableau.x_gate(0);

    let (bits, counters) = direct_collapse(&mut tableau, &[(0, false)]);

    assert_eq!(bits, vec![true]);
    assert_eq!(counters.transposed_collapse_batches, 0);
    assert_eq!(counters.collapse_pivots, 0);
}

#[test]
fn direct_collapse_crosses_64_and_128_qubit_boundaries() {
    let num_qubits = 130;
    let mut packed = PackedInverseTableau::identity(num_qubits);
    let mut legacy = StabilizerState::new(num_qubits);

    for q in [0, 63, 64, 65, 127, 128, 129] {
        packed.h(q);
        legacy.h(q);
    }
    for q in [63, 64, 127, 128] {
        packed.s_dag(q);
        legacy.s_dag(q);
    }
    for (control, target) in [(63, 64), (64, 65), (127, 128), (128, 129)] {
        packed.cx(control, target);
        legacy.cx(control, target);
    }
    packed.x_gate(129);
    legacy.x_gate(129);

    let targets = [
        (0, false),
        (63, false),
        (64, true),
        (65, false),
        (127, false),
        (128, true),
        (129, false),
    ];
    let (packed_bits, counters) = direct_collapse(&mut packed, &targets);
    let legacy_bits = legacy_measure_z_many(&mut legacy, &targets);

    assert_eq!(packed_bits, legacy_bits);
    assert_eq!(counters.direct_inverse_batches, 1);
    assert_eq!(counters.transposed_collapse_batches, 1);
    assert!(counters.collapse_pivots >= 3);

    let verification_targets: Vec<_> = (0..num_qubits).map(|q| (q, false)).collect();
    let mut packed_x = packed.clone();
    let mut legacy_x = legacy.clone();
    for q in 0..num_qubits {
        packed_x.h(q);
        legacy_x.h(q);
    }
    assert_eq!(
        packed_x.measure_z_many_biased(&verification_targets),
        legacy_measure_z_many(&mut legacy_x, &verification_targets),
        "boundary collapse X-basis state",
    );

    let mut packed_y = packed.clone();
    let mut legacy_y = legacy.clone();
    for q in 0..num_qubits {
        packed_y.s_dag(q);
        packed_y.h(q);
        legacy_y.s_dag(q);
        legacy_y.h(q);
    }
    assert_eq!(
        packed_y.measure_z_many_biased(&verification_targets),
        legacy_measure_z_many(&mut legacy_y, &verification_targets),
        "boundary collapse Y-basis state",
    );
}
