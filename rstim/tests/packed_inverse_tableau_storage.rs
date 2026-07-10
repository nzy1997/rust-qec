use std::panic::{catch_unwind, AssertUnwindSafe};

use rstim::sim::packed_inverse_tableau::PackedInverseTableau;

fn words_for_bits(bits: usize) -> usize {
    bits.div_ceil(64)
}

fn expected_plane_len(num_qubits: usize) -> usize {
    2 * num_qubits * words_for_bits(num_qubits)
}

fn assert_identity(num_qubits: usize) {
    let tableau = PackedInverseTableau::identity(num_qubits);
    let words_per_row = words_for_bits(num_qubits);
    let num_rows = 2 * num_qubits;

    assert_eq!(tableau.num_qubits(), num_qubits);
    assert_eq!(tableau.num_rows(), num_rows);
    assert_eq!(tableau.words_per_row(), words_per_row);
    assert_eq!(tableau.x_plane_words().len(), expected_plane_len(num_qubits));
    assert_eq!(tableau.z_plane_words().len(), expected_plane_len(num_qubits));
    assert_eq!(tableau.sign_words().len(), words_for_bits(num_rows));

    if num_qubits == 0 {
        assert!(tableau.x_plane_words().is_empty());
        assert!(tableau.z_plane_words().is_empty());
        assert!(tableau.sign_words().is_empty());
    }

    for row in 0..num_rows {
        assert!(!tableau.sign_bit(row), "identity sign bit row {row}");
        assert_eq!(tableau.canonical_phase(row), 0, "identity phase row {row}");

        for qubit in 0..num_qubits {
            let expected_x = row < num_qubits && row == qubit;
            let expected_z = row >= num_qubits && row - num_qubits == qubit;
            assert_eq!(tableau.x(row, qubit), expected_x, "x({row}, {qubit})");
            assert_eq!(tableau.z(row, qubit), expected_z, "z({row}, {qubit})");
        }
    }
}

fn row_words(plane: &[u64], row: usize, words_per_row: usize) -> &[u64] {
    let start = row * words_per_row;
    &plane[start..start + words_per_row]
}

fn assert_padding_zero(tableau: &PackedInverseTableau) {
    assert_eq!(tableau.num_qubits(), 130);
    let words_per_row = tableau.words_per_row();
    let valid_mask = (1u64 << (130 % 64)) - 1;
    let padding_mask = !valid_mask;

    for row in 0..tableau.num_rows() {
        let last_word = row * words_per_row + words_per_row - 1;
        assert_eq!(
            tableau.x_plane_words()[last_word] & padding_mask,
            0,
            "x padding row {row}",
        );
        assert_eq!(
            tableau.z_plane_words()[last_word] & padding_mask,
            0,
            "z padding row {row}",
        );
    }
}

#[test]
fn identity_and_lengths_are_exact_for_0_1_64_65_130() {
    for num_qubits in [0, 1, 64, 65, 130] {
        assert_identity(num_qubits);
    }
}

#[test]
fn boundary_bits_63_64_129_map_to_expected_words() {
    let tableau = PackedInverseTableau::identity(130);
    let w = tableau.words_per_row();

    assert_eq!(w, 3);
    assert_eq!(row_words(tableau.x_plane_words(), 63, w), &[1u64 << 63, 0, 0]);
    assert_eq!(row_words(tableau.x_plane_words(), 64, w), &[0, 1, 0]);
    assert_eq!(row_words(tableau.x_plane_words(), 129, w), &[0, 0, 1u64 << 1]);

    assert_eq!(
        row_words(tableau.z_plane_words(), 130 + 63, w),
        &[1u64 << 63, 0, 0],
    );
    assert_eq!(row_words(tableau.z_plane_words(), 130 + 64, w), &[0, 1, 0]);
    assert_eq!(
        row_words(tableau.z_plane_words(), 130 + 129, w),
        &[0, 0, 1u64 << 1],
    );

    assert!(!tableau.x(129, 130 - 2));
    assert!(tableau.x(129, 129));
    assert!(tableau.z(259, 129));

    assert!(catch_unwind(|| tableau.x(0, 130)).is_err());
    assert!(catch_unwind(|| tableau.z(0, 130)).is_err());
    assert!(catch_unwind(|| tableau.sign_bit(260)).is_err());
}

#[test]
fn packed_signs_round_trip_positive_and_negative() {
    let mut tableau = PackedInverseTableau::identity(130);

    assert_eq!(tableau.sign_words().len(), 5);
    assert!(tableau.sign_words().iter().all(|word| *word == 0));

    tableau.set_sign_bit(0, true);
    assert!(tableau.sign_bit(0));
    assert_eq!(tableau.canonical_phase(0), 2);

    tableau.set_canonical_phase(0, 0);
    assert!(!tableau.sign_bit(0));
    assert_eq!(tableau.canonical_phase(0), 0);

    tableau.set_canonical_phase(64, 2);
    tableau.set_sign_bit(129, true);

    assert!(catch_unwind(AssertUnwindSafe(|| tableau.set_sign_bit(260, false))).is_err());

    assert_eq!(tableau.canonical_phase(64), 2);
    assert_eq!(tableau.canonical_phase(129), 2);
    assert_eq!(tableau.sign_words()[1] & 1, 1);
    assert_eq!((tableau.sign_words()[2] >> 1) & 1, 1);
    assert_eq!(tableau.sign_words()[0] & 1, 0);
}

#[test]
fn row_copy_and_plane_xor_obey_contract() {
    let mut tableau = PackedInverseTableau::identity(130);

    tableau.set_canonical_phase(1, 2);
    tableau.copy_row(1, 131);
    assert!(tableau.x(131, 1));
    assert!(!tableau.z(131, 1));
    assert_eq!(tableau.canonical_phase(131), 2);

    tableau.set_canonical_phase(0, 2);
    tableau.set_canonical_phase(64, 0);
    tableau.xor_pauli_planes(0, 64);
    assert!(tableau.x(64, 0));
    assert!(tableau.x(64, 64));
    assert!(!tableau.z(64, 0));
    assert!(!tableau.sign_bit(64));
    assert_eq!(tableau.canonical_phase(64), 0);

    tableau.set_canonical_phase(194, 2);
    tableau.xor_pauli_planes(130, 194);
    assert!(tableau.z(194, 0));
    assert!(tableau.z(194, 64));
    assert!(!tableau.x(194, 0));
    assert_eq!(tableau.canonical_phase(194), 2);

    assert!(catch_unwind(AssertUnwindSafe(|| tableau.copy_row(260, 0))).is_err());
    assert!(catch_unwind(AssertUnwindSafe(|| tableau.xor_pauli_planes(0, 260))).is_err());
}

#[test]
fn unused_padding_bits_stay_zero() {
    let mut tableau = PackedInverseTableau::identity(130);

    assert_padding_zero(&tableau);
    tableau.copy_row(129, 0);
    assert_padding_zero(&tableau);
    tableau.xor_pauli_planes(128, 0);
    assert_padding_zero(&tableau);
    tableau.copy_row(259, 1);
    assert_padding_zero(&tableau);
    tableau.xor_pauli_planes(258, 1);
    assert_padding_zero(&tableau);

    println!("PASS packed inverse-tableau storage");
}
