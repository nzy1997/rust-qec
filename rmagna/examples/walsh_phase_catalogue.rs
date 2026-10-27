use rmagna::catalogue::{CatalogueLimits, PhaseCatalogue};
use rmagna::scalar::ScalarArithmetic;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let catalogue = PhaseCatalogue::from_walsh_table(
        1,
        &[0, 1],
        2,
        &CatalogueLimits::default(),
        &ScalarArithmetic::default(),
    )?;
    let usage = catalogue.usage();
    assert_eq!(usage.terms, 2);
    println!(
        "Certified {} exact Walsh characters; reconstruction work {}",
        usage.terms, usage.verification_work
    );
    Ok(())
}
