use rmagna::catalogue::{FrozenPhasePlan, PhaseSelectionOptions, PhaseSelectionRoute};
use rmagna::gf2::BitVector;
use rmagna::phase::ParityPhase;
use rmagna::quadratic::QuadraticForm;
use rmagna::scalar::{ExactScalar, ScalarArithmetic};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arithmetic = ScalarArithmetic::default();
    let phase = ParityPhase::new(
        3,
        0,
        (0..3)
            .map(|j| Ok::<_, Box<dyn std::error::Error>>((BitVector::from_indices(3, [j])?, 1)))
            .collect::<Result<Vec<_>, _>>()?,
    )?;
    let plan = FrozenPhasePlan::new(&phase, None, &PhaseSelectionOptions::default(), &arithmetic)?;
    let boundary = QuadraticForm::new(0, vec![0; 3], vec![BitVector::zeros(3)?; 3])?;
    let value = plan.evaluate(&phase, &boundary, &ExactScalar::one(), &arithmetic)?;
    let one_plus_omega = arithmetic.add(&ExactScalar::one(), &arithmetic.omega(1, 0)?)?;
    let expected = arithmetic.multiply(
        &arithmetic.multiply(&one_plus_omega, &one_plus_omega)?,
        &one_plus_omega,
    )?;
    assert_eq!(value, expected);
    assert_eq!(plan.receipt().route, PhaseSelectionRoute::Walsh);
    println!(
        "Selected {}: rank {}, table {} logical bytes, {} terms",
        plan.receipt().route.name(),
        plan.quotient_rank(),
        plan.receipt().logical_core_table_bytes,
        plan.num_terms()
    );
    Ok(())
}
