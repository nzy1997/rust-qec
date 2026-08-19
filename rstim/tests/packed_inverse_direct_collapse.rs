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

fn legacy_measure_z_many(state: &mut StabilizerState, targets: &[(usize, bool)]) -> Vec<bool> {
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

#[derive(Clone, Debug)]
struct PauliRow {
    x: Vec<bool>,
    z: Vec<bool>,
    phase: u8,
}

impl PauliRow {
    fn from_snapshot(snapshot: &CanonicalTableauSnapshot, row: usize) -> Self {
        Self {
            x: snapshot.x[row].clone(),
            z: snapshot.z[row].clone(),
            phase: snapshot.phase[row],
        }
    }

    fn bit(&self, bit: usize) -> bool {
        let n = self.x.len();
        if bit < n {
            self.x[bit]
        } else {
            self.z[bit - n]
        }
    }

    fn leading_bit(&self) -> Option<usize> {
        (0..2 * self.x.len()).find(|&bit| self.bit(bit))
    }

    fn is_identity(&self) -> bool {
        !self.x.iter().any(|&bit| bit) && !self.z.iter().any(|&bit| bit)
    }

    fn multiply_assign(&mut self, rhs: &PauliRow) {
        let mut phase_delta = 0;
        for q in 0..self.x.len() {
            let (x, z, phase) = multiply_pauli(self.x[q], self.z[q], rhs.x[q], rhs.z[q]);
            self.x[q] = x;
            self.z[q] = z;
            phase_delta = (phase_delta + phase) % 4;
        }
        self.phase = (self.phase + rhs.phase + phase_delta) % 4;
    }
}

fn multiply_pauli(x1: bool, z1: bool, x2: bool, z2: bool) -> (bool, bool, u8) {
    match ((x1, z1), (x2, z2)) {
        ((false, false), _) => (x2, z2, 0),
        (_, (false, false)) => (x1, z1, 0),
        ((true, false), (true, false)) => (false, false, 0),
        ((false, true), (false, true)) => (false, false, 0),
        ((true, true), (true, true)) => (false, false, 0),
        ((true, false), (false, true)) => (true, true, 3),
        ((false, true), (true, false)) => (true, true, 1),
        ((true, false), (true, true)) => (false, true, 1),
        ((true, true), (true, false)) => (false, true, 3),
        ((false, true), (true, true)) => (true, false, 3),
        ((true, true), (false, true)) => (true, false, 1),
    }
}

fn reduce_row(row: &mut PauliRow, basis: &[Option<PauliRow>]) {
    for pivot in 0..basis.len() {
        if row.bit(pivot) {
            if let Some(basis_row) = &basis[pivot] {
                row.multiply_assign(basis_row);
            }
        }
    }
}

fn stabilizer_basis(snapshot: &CanonicalTableauSnapshot) -> Vec<Option<PauliRow>> {
    let n = snapshot.num_qubits;
    let mut basis = vec![None; 2 * n];
    for row in n..2 * n {
        let mut candidate = PauliRow::from_snapshot(snapshot, row);
        reduce_row(&mut candidate, &basis);
        if let Some(pivot) = candidate.leading_bit() {
            basis[pivot] = Some(candidate);
        } else {
            assert_eq!(
                candidate.phase % 4,
                0,
                "dependent stabilizer row reduced to non-identity phase",
            );
        }
    }
    basis
}

fn row_in_span(row: PauliRow, basis: &[Option<PauliRow>]) -> bool {
    let mut candidate = row;
    reduce_row(&mut candidate, basis);
    candidate.is_identity() && candidate.phase % 4 == 0
}

fn assert_stabilizer_groups_match(
    packed: &PackedInverseTableau,
    legacy: &StabilizerState,
    label: &str,
) {
    let packed_snapshot = packed.canonical_snapshot();
    let legacy_snapshot = legacy.canonical_snapshot();
    let n = packed_snapshot.num_qubits;
    assert_eq!(legacy_snapshot.num_qubits, n, "{label}");

    let legacy_basis = stabilizer_basis(&legacy_snapshot);
    for row in n..2 * n {
        assert!(
            row_in_span(
                PauliRow::from_snapshot(&packed_snapshot, row),
                &legacy_basis
            ),
            "{label}: packed stabilizer row {row} is not in the legacy stabilizer group",
        );
    }

    let packed_basis = stabilizer_basis(&packed_snapshot);
    for row in n..2 * n {
        assert!(
            row_in_span(
                PauliRow::from_snapshot(&legacy_snapshot, row),
                &packed_basis
            ),
            "{label}: legacy stabilizer row {row} is not in the packed stabilizer group",
        );
    }
}

fn assert_raw_z_row_has_y_pivot(tableau: &PackedInverseTableau, q: usize) {
    let row = tableau.num_qubits() + q;
    assert!(tableau.x(row, q), "raw Z row {q} must have X support");
    assert!(tableau.z(row, q), "raw Z row {q} must have Z support");
}

fn raw_z_x_support(tableau: &PackedInverseTableau, q: usize) -> Vec<usize> {
    let row = tableau.num_qubits() + q;
    (0..tableau.num_qubits())
        .filter(|&qubit| tableau.x(row, qubit))
        .collect()
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

    let (bits, counters) = direct_collapse(
        &mut tableau,
        &[(0, false), (1, false), (2, false), (3, true)],
    );

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
fn direct_collapse_eliminates_multi_x_support_without_padding() {
    let num_qubits = 32;
    let mut packed = PackedInverseTableau::identity(num_qubits);
    let mut legacy = StabilizerState::new(num_qubits);

    packed.h(0);
    legacy.h(0);
    packed.h(1);
    legacy.h(1);
    packed.cx(0, 1);
    legacy.cx(0, 1);

    let target = (0..num_qubits)
        .find(|&q| {
            let support = raw_z_x_support(&packed, q);
            support.len() >= 2 && support.iter().skip(1).any(|&later| later > support[0])
        })
        .expect("test setup must create a raw Z row with later X support");
    assert_eq!(raw_z_x_support(&packed, target), vec![0, 1]);

    let targets = [(target, false)];
    let (packed_bits, counters) = direct_collapse(&mut packed, &targets);
    let legacy_bits = legacy_measure_z_many(&mut legacy, &targets);

    assert_eq!(packed_bits, legacy_bits);
    assert_eq!(counters.direct_inverse_batches, 1);
    assert_eq!(counters.transposed_collapse_batches, 1);
    assert_eq!(counters.collapse_pivots, 1);
    assert_snapshot_matches_legacy(&packed, &legacy, "multi-X no-padding collapse snapshot");
}

#[test]
fn direct_collapse_public_width_fast_path_matches_legacy() {
    let num_qubits = 241;
    let mut packed = PackedInverseTableau::identity(num_qubits);
    let mut legacy = StabilizerState::new(num_qubits);

    for q in [0, 1] {
        packed.h(q);
        legacy.h(q);
    }
    packed.cx(0, 1);
    legacy.cx(0, 1);

    let target = (0..num_qubits)
        .find(|&q| raw_z_x_support(&packed, q) == [0, 1])
        .expect("test setup must create a two-column X support row");
    let targets = [(target, false)];
    let (packed_bits, counters) = direct_collapse(&mut packed, &targets);
    let legacy_bits = legacy_measure_z_many(&mut legacy, &targets);

    assert_eq!(packed_bits, legacy_bits);
    assert_eq!(counters.collapse_pivots, 1);
    assert_snapshot_matches_legacy(&packed, &legacy, "241-qubit collapse snapshot");
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

    let mut y_packed = PackedInverseTableau::identity(num_qubits);
    let mut y_legacy = StabilizerState::new(num_qubits);
    for q in [63, 127] {
        y_packed.s(q);
        y_packed.h(q);
        y_legacy.s(q);
        y_legacy.h(q);
        assert_raw_z_row_has_y_pivot(&y_packed, q);
    }
    for q in [64, 128] {
        y_packed.s_dag(q);
        y_packed.h(q);
        y_legacy.s_dag(q);
        y_legacy.h(q);
        assert_raw_z_row_has_y_pivot(&y_packed, q);
    }

    let y_targets = [(63, false), (64, true), (127, false), (128, true)];
    let (y_bits, y_counters) = direct_collapse(&mut y_packed, &y_targets);
    let y_legacy_bits = legacy_measure_z_many(&mut y_legacy, &y_targets);

    assert_eq!(y_bits, y_legacy_bits);
    assert_eq!(y_counters.direct_inverse_batches, 1);
    assert_eq!(y_counters.transposed_collapse_batches, 1);
    assert_eq!(y_counters.collapse_pivots, 4);
    assert_eq!(y_counters.canonical_materializations, 0);
    assert_eq!(y_counters.canonical_writebacks, 0);
    assert_eq!(
        y_packed.measure_z_many_biased(&y_targets),
        legacy_measure_z_many(&mut y_legacy, &y_targets),
        "Y-pivot boundary collapse must leave target signs deterministic",
    );
    assert_stabilizer_groups_match(
        &y_packed,
        &y_legacy,
        "boundary Y-pivot collapse stabilizers",
    );
}
