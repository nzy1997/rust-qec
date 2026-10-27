use rmagna::{
    catalogue::{
        FrozenPhasePlan, PhaseInjectionOptions, PhaseSelectionOptions, builtin_phase_catalogue,
    },
    gf2::BitVector,
    phase::ParityPhase,
    scalar::ScalarArithmetic,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Pinned native correctness witness: twelve pair groups share only three
    // independent constraints. A Cartesian product would have 4,096 branches.
    let terms = [
        (2, 3),
        (3, 3),
        (11, 3),
        (15, 7),
        (18, 7),
        (19, 7),
        (21, 5),
        (33, 5),
        (50, 1),
        (52, 1),
        (56, 7),
        (57, 7),
        (64, 2),
    ]
    .into_iter()
    .map(|(mask, c)| {
        Ok((
            BitVector::from_indices(7, (0..7).filter(|j| mask >> j & 1 != 0))?,
            c,
        ))
    })
    .collect::<Result<Vec<_>, rmagna::gf2::Gf2Error>>()?;
    let phase = ParityPhase::new(7, 1, terms)?;
    let mut options = PhaseSelectionOptions::default();
    // Allow bounded baseline construction, then admit only the reduced result.
    options.construction.max_terms = 64;
    options.preparation.max_terms = 8;
    let injection = PhaseInjectionOptions {
        max_terms: 8,
        ..Default::default()
    };
    let plan = FrozenPhasePlan::new_with_injection(
        &phase,
        Some(builtin_phase_catalogue()?),
        &options,
        &injection,
        &ScalarArithmetic::default(),
    )?;
    let receipt = plan
        .injection_receipt()
        .expect("explicit injection requested");
    assert_eq!(receipt.original_terms, 64);
    assert_eq!(receipt.groups.len(), 12);
    assert_eq!(receipt.constraint_rank, 3);
    assert_eq!(plan.num_terms(), 8);
    assert!(receipt.selected);
    println!(
        "Selected T-pair injection: {} -> {} terms, {} groups, constraint rank {}",
        receipt.original_terms,
        plan.num_terms(),
        receipt.groups.len(),
        receipt.constraint_rank
    );
    Ok(())
}
