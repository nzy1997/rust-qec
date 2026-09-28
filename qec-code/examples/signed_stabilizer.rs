//! Run with `cargo run -p qec-code --example signed_stabilizer`.
use qec_code::Pauli;
use qec_code::phased_pauli::{Phase, PhasedPauli, SignedStabilizerGroup};

fn signed(x: &[u8], z: &[u8], phase: Phase) -> PhasedPauli {
    PhasedPauli::new(Pauli::from_xz_bits(x.to_vec(), z.to_vec()).unwrap(), phase)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let zz = signed(&[0, 0], &[1, 1], Phase::PlusOne);
    let group = SignedStabilizerGroup::new(2, vec![zz])?;
    let x_logical = signed(&[1, 1], &[0, 0], Phase::PlusOne);
    let z_logical = signed(&[0, 0], &[1, 0], Phase::PlusOne);
    let basis = group.validate_logical_basis(vec![x_logical], vec![z_logical])?;
    assert_eq!(basis.k(), 1);

    let witness = group
        .contains_with_witness(&signed(&[0, 0], &[1, 1], Phase::PlusOne))?
        .expect("+ZZ is a stabilizer");
    assert_eq!(
        group.product_from_indices(witness.generator_indices())?,
        group.generators()[0]
    );
    assert!(
        group
            .contains_with_witness(&signed(&[0, 0], &[1, 1], Phase::MinusOne))?
            .is_none()
    );
    Ok(())
}
